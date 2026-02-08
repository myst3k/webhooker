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

/// Extract the CORS allowed origin from endpoint settings, defaulting to "*".
fn get_cors_origin(settings: &Option<serde_json::Value>) -> String {
    settings
        .as_ref()
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
        .unwrap_or_else(|| "*".to_string())
}

/// Wrap a response with CORS headers.
fn with_cors(response: Response, origin: &str) -> Response {
    let mut response = response;
    let headers = response.headers_mut();
    headers.insert("Access-Control-Allow-Origin", origin.parse().unwrap_or_else(|_| "*".parse().unwrap()));
    headers.insert("Access-Control-Allow-Methods", "POST, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type".parse().unwrap());
    response
}

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

    let cors_origin = get_cors_origin(&endpoint.settings);

    // Parse body
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok());

    let raw_data = if content_type.is_some_and(|ct| ct.contains("multipart/form-data")) {
        parser::parse_multipart(&headers, body)
            .await
            .map_err(|e| {
                with_cors(
                    (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
                    &cors_origin,
                )
            })?
    } else {
        parser::parse_body(content_type, &body).map_err(|e| {
            with_cors(
                (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
                &cors_origin,
            )
        })?
    };

    let peer_ip: Option<IpAddr> = Some(addr.ip());

    let result = pipeline::run(&state, &endpoint, &headers, peer_ip, raw_data)
        .await
        .map_err(|e| {
            let status = if e.contains("Rate limited") {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            with_cors(
                (status, Json(json!({"error": e}))).into_response(),
                &cors_origin,
            )
        })?;

    // If redirect configured and it's a form submission, redirect
    if let Some(ref url) = result.redirect_url {
        if content_type.is_some_and(|ct| ct.contains("form")) {
            return Ok(with_cors(Redirect::to(url).into_response(), &cors_origin));
        }
    }

    if result.spam {
        // Silent 200 for spam
        return Ok(with_cors(
            (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            &cors_origin,
        ));
    }

    Ok(with_cors(
        (
            StatusCode::CREATED,
            Json(json!({
                "status": "created",
                "submission_id": result.submission_id,
            })),
        )
            .into_response(),
        &cors_origin,
    ))
}

pub async fn ingest_options(
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
) -> Response {
    let endpoint = db::endpoints::find_by_id(&state.pool, endpoint_id).await;

    let allowed_origins = endpoint
        .ok()
        .flatten()
        .map(|e| get_cors_origin(&e.settings))
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
