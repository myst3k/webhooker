use std::net::IpAddr;

use axum::http::HeaderMap;
use serde_json::json;
use uuid::Uuid;

use crate::actions::context::ActionContext;
use crate::actions::ActionStatus;
use crate::db;
use crate::models::Endpoint;
use crate::state::SharedState;

use super::fields;
use super::honeypot;
use super::metadata;

pub struct PipelineResult {
    pub submission_id: Option<Uuid>,
    pub redirect_url: Option<String>,
    pub spam: bool,
}

/// Run the 11-step submission processing pipeline.
pub async fn run(
    state: &SharedState,
    endpoint: &Endpoint,
    headers: &HeaderMap,
    peer_addr: Option<IpAddr>,
    raw_data: serde_json::Value,
) -> Result<PipelineResult, String> {
    let settings = endpoint
        .settings
        .as_ref()
        .cloned()
        .unwrap_or(json!({}));

    // Step 1: Rate limit check
    let rate_limit = settings["rate_limit"].as_u64().unwrap_or(10) as u32;
    let rate_window = settings["rate_limit_window_secs"].as_u64().unwrap_or(60);
    let ip = peer_addr.unwrap_or(IpAddr::from([127, 0, 0, 1]));

    if let Err(retry_after) =
        state
            .submission_limiter
            .check(endpoint.id, ip, rate_limit, rate_window)
    {
        return Err(format!("Rate limited. Retry after {retry_after}s"));
    }

    // Step 2: Body is already parsed (passed as raw_data)

    // Step 3: Honeypot check
    let honeypot_field = settings["honeypot_field"].as_str();
    if honeypot::is_spam(&raw_data, honeypot_field) {
        return Ok(PipelineResult {
            submission_id: None,
            redirect_url: settings["redirect_url"]
                .as_str()
                .map(|s| s.to_string()),
            spam: true,
        });
    }

    // Step 4: Store raw (will be stored as part of submission)
    let raw = raw_data.clone();

    // Step 5: Sort fields
    let (data, extras) = fields::sort_fields(&raw_data, endpoint.fields.as_ref());

    // Step 6: Validate (warnings only, don't reject)
    let warnings = fields::validate_fields(&data, endpoint.fields.as_ref());
    if !warnings.is_empty() {
        tracing::debug!("Validation warnings for endpoint {}: {:?}", endpoint.id, warnings);
    }

    // Step 7: Capture metadata
    let meta = metadata::extract(headers, peer_addr, &state.config.trusted_proxies);

    // Step 8: Store submission
    let submission = db::submissions::create(
        &state.pool,
        endpoint.id,
        &data,
        &extras,
        &raw,
        &meta,
    )
    .await
    .map_err(|e| format!("Failed to store submission: {e}"))?;

    // Step 9: Run action pipeline
    let actions = db::actions::list_enabled_ordered(&state.pool, endpoint.id)
        .await
        .unwrap_or_default();

    if !actions.is_empty() {
        // Load project and tenant for action context
        let project = db::projects::find_by_id_unscoped(&state.pool, endpoint.project_id)
            .await
            .ok()
            .flatten();

        let tenant = if let Some(ref proj) = project {
            db::tenants::find_by_id(&state.pool, proj.tenant_id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        if let (Some(project), Some(tenant)) = (project, tenant) {
            let ctx = ActionContext {
                submission: submission.clone(),
                endpoint: endpoint.clone(),
                project,
                tenant,
            };

            // Step 9-10: Execute actions and log results
            for action in &actions {
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
                        Ok(Err(e)) => (
                            "failed",
                            Some(json!({ "error": e.message })),
                        ),
                        Err(_) => (
                            "failed",
                            Some(json!({ "error": "Action timed out after 30s" })),
                        ),
                    }
                } else {
                    (
                        "skipped",
                        Some(json!({ "error": format!("Unknown module: {}", action.action_type) })),
                    )
                };

                // Step 10: Log action result
                let _ = db::action_log::create(
                    &state.pool,
                    action.id,
                    submission.id,
                    status,
                    response.as_ref(),
                )
                .await;
            }
        }
    }

    // Step 11: Respond
    let redirect_url = settings["redirect_url"]
        .as_str()
        .map(|s| s.to_string());

    Ok(PipelineResult {
        submission_id: Some(submission.id),
        redirect_url,
        spam: false,
    })
}
