use std::net::IpAddr;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::state::SharedState;
use crate::submission::{parser, pipeline};

pub async fn ingest(
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let endpoint = db::endpoints::find_by_id(&state.pool, endpoint_id)
        .await
        .map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response()
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(json!({"error": "Endpoint not found"}))).into_response()
        })?;

    // Parse body
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok());

    let raw_data = if content_type.is_some_and(|ct| ct.contains("multipart/form-data")) {
        parser::parse_multipart(&headers, body)
            .await
            .map_err(|e| {
                (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response()
            })?
    } else {
        parser::parse_body(content_type, &body).map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response()
        })?
    };

    let peer_ip: Option<IpAddr> = Some(addr.ip());

    let result = pipeline::run(&state, &endpoint, &headers, peer_ip, raw_data)
        .await
        .map_err(|e| {
            if e.contains("Rate limited") {
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error": e}))).into_response()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response()
            }
        })?;

    // If redirect configured and it's a form submission, redirect
    if let Some(ref url) = result.redirect_url {
        if content_type.is_some_and(|ct| ct.contains("form")) {
            return Ok(Redirect::to(url).into_response());
        }
    }

    if result.spam {
        // Silent 200 for spam
        return Ok((StatusCode::OK, Json(json!({"status": "ok"}))).into_response());
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "submission_id": result.submission_id,
        })),
    )
        .into_response())
}

pub async fn ingest_options(
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
) -> Response {
    let endpoint = db::endpoints::find_by_id(&state.pool, endpoint_id).await;

    let allowed_origins = endpoint
        .ok()
        .flatten()
        .and_then(|e| e.settings)
        .and_then(|s| {
            s.get("cors_origins")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
        })
        .unwrap_or_else(|| "*".to_string());

    (
        [
            ("Access-Control-Allow-Origin", allowed_origins),
            ("Access-Control-Allow-Methods", "POST, OPTIONS".to_string()),
            (
                "Access-Control-Allow-Headers",
                "Content-Type".to_string(),
            ),
            ("Access-Control-Max-Age", "86400".to_string()),
        ],
        StatusCode::NO_CONTENT,
    )
        .into_response()
}
