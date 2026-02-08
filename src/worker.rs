use serde_json::json;
use tokio::sync::watch;

use crate::actions::context::ActionContext;
use crate::actions::ActionStatus;
use crate::db;
use crate::state::SharedState;

/// Start a worker pool on a dedicated Tokio runtime with its own thread pool.
/// This runs on a separate OS thread and blocks until shutdown is signaled.
pub fn run_pool(
    state: SharedState,
    shutdown: watch::Receiver<bool>,
    worker_count: usize,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("worker-pool".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(worker_count)
                .thread_name("action-worker")
                .enable_all()
                .build()
                .expect("Failed to build worker runtime");

            runtime.block_on(async {
                let mut handles = Vec::with_capacity(worker_count);

                for id in 0..worker_count {
                    handles.push(tokio::spawn(run(id, state.clone(), shutdown.clone())));
                }

                tracing::info!("Action worker pool started ({worker_count} workers)");

                for handle in handles {
                    let _ = handle.await;
                }

                tracing::info!("Action worker pool stopped");
            });
        })
        .expect("Failed to spawn worker pool thread")
}

/// A single worker loop that polls the queue and processes items.
async fn run(id: usize, state: SharedState, mut shutdown: watch::Receiver<bool>) {
    tracing::debug!("Worker {id} started");

    loop {
        if *shutdown.borrow() {
            break;
        }

        match process_next(&state).await {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                tracing::error!("Worker {id} error: {e}");
            }
        }

            tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            _ = shutdown.changed() => {}
        }
    }

    tracing::debug!("Worker {id} stopped");
}

/// Try to claim and process the next queue item. Returns true if an item was processed.
async fn process_next(state: &SharedState) -> Result<bool, String> {
    let item = db::action_queue::claim_next(&state.pool)
        .await
        .map_err(|e| format!("Failed to claim queue item: {e}"))?;

    let item = match item {
        Some(item) => item,
        None => return Ok(false),
    };

    tracing::debug!(
        "Processing queue item {} (action={}, submission={}, attempt={})",
        item.id,
        item.action_id,
        item.submission_id,
        item.attempts
    );

    let action = db::actions::find_by_id(&state.pool, item.action_id)
        .await
        .map_err(|e| format!("Failed to load action: {e}"))?;

    let action = match action {
        Some(a) => a,
        None => {
            let error = format!("Action {} not found", item.action_id);
            let _ = db::action_queue::mark_failed(
                &state.pool,
                item.id,
                item.attempts,
                item.max_attempts,
                &error,
            )
            .await;
            let _ = db::action_log::create(
                &state.pool,
                item.action_id,
                item.submission_id,
                "failed",
                Some(&json!({ "error": &error })),
            )
            .await;
            return Ok(true);
        }
    };

    let submission = db::submissions::find_by_id(&state.pool, item.submission_id)
        .await
        .map_err(|e| format!("Failed to load submission: {e}"))?;

    let submission = match submission {
        Some(s) => s,
        None => {
            let error = format!("Submission {} not found", item.submission_id);
            let _ = db::action_queue::mark_failed(
                &state.pool,
                item.id,
                item.attempts,
                item.max_attempts,
                &error,
            )
            .await;
            return Ok(true);
        }
    };

    let endpoint = db::endpoints::find_by_id(&state.pool, action.endpoint_id)
        .await
        .ok()
        .flatten();

    let project = if let Some(ref ep) = endpoint {
        db::projects::find_by_id_unscoped(&state.pool, ep.project_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let tenant = if let Some(ref proj) = project {
        db::tenants::find_by_id(&state.pool, proj.tenant_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let (endpoint, project, tenant) = match (endpoint, project, tenant) {
        (Some(ep), Some(proj), Some(t)) => (ep, proj, t),
        _ => {
            let error = "Failed to load action context (endpoint/project/tenant missing)";
            let _ = db::action_queue::mark_failed(
                &state.pool,
                item.id,
                item.attempts,
                item.max_attempts,
                error,
            )
            .await;
            let _ = db::action_log::create(
                &state.pool,
                item.action_id,
                item.submission_id,
                "failed",
                Some(&json!({ "error": error })),
            )
            .await;
            return Ok(true);
        }
    };

    let ctx = ActionContext {
        submission,
        endpoint,
        project,
        tenant,
    };

    let module = state.modules.get(&action.action_type);
    let (status, response) = if let Some(module) = module {
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            module.execute(&ctx, &action.config),
        )
        .await
        {
            Ok(Ok(result)) => {
                let status_str = match result.status {
                    ActionStatus::Success => "success",
                    ActionStatus::Failed => "failed",
                };
                (status_str, result.response)
            }
            Ok(Err(e)) => ("failed", Some(json!({ "error": e.message }))),
            Err(_) => ("failed", Some(json!({ "error": "Action timed out after 30s" }))),
        }
    } else {
        (
            "failed",
            Some(json!({ "error": format!("Unknown module: {}", action.action_type) })),
        )
    };

    let _ = db::action_log::create(
        &state.pool,
        item.action_id,
        item.submission_id,
        status,
        response.as_ref(),
    )
    .await;

    if status == "success" {
        let _ = db::action_queue::mark_completed(&state.pool, item.id).await;
    } else {
        let error_msg = response
            .as_ref()
            .and_then(|r| r.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or("Unknown error");
        let _ = db::action_queue::mark_failed(
            &state.pool,
            item.id,
            item.attempts,
            item.max_attempts,
            error_msg,
        )
        .await;
    }

    Ok(true)
}
