use std::net::IpAddr;

use axum::http::HeaderMap;
use serde_json::json;
use uuid::Uuid;

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

    let raw = raw_data.clone();
    let (data, extras) = fields::sort_fields(&raw_data, endpoint.fields.as_ref());

    let warnings = fields::validate_fields(&data, endpoint.fields.as_ref());
    if !warnings.is_empty() {
        tracing::debug!("Validation warnings for endpoint {}: {:?}", endpoint.id, warnings);
    }

    let meta = metadata::extract(headers, peer_addr, &state.config.trusted_proxies);

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

    let actions = db::actions::list_enabled_ordered(&state.pool, endpoint.id)
        .await
        .unwrap_or_default();

    for action in &actions {
        if let Err(e) = db::action_queue::enqueue(&state.pool, submission.id, action.id).await {
            tracing::error!("Failed to enqueue action {}: {e}", action.id);
        }
    }

    let redirect_url = settings["redirect_url"]
        .as_str()
        .map(|s| s.to_string());

    Ok(PipelineResult {
        submission_id: Some(submission.id),
        redirect_url,
        spam: false,
    })
}
