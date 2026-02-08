use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::Submission;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct ListParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct ExportParams {
    pub format: Option<String>,
}

pub async fn list(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify endpoint belongs to tenant
    db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100).max(1);
    let offset = (page - 1) * per_page;

    let list_params = db::submissions::ListParams {
        endpoint_id,
        limit: per_page,
        offset,
        sort_by: db::submissions::SortColumn::parse(params.sort_by.as_deref().unwrap_or("created_at")),
        sort_order: db::submissions::SortOrder::parse(params.sort_order.as_deref().unwrap_or("desc")),
        search: params.search.clone(),
    };

    let submissions = db::submissions::list(&state.pool, &list_params).await?;
    let total = db::submissions::count(&state.pool, endpoint_id, params.search.as_deref()).await?;

    Ok(Json(serde_json::json!({
        "submissions": submissions,
        "total": total,
        "page": page,
        "per_page": per_page,
        "total_pages": (total as f64 / per_page as f64).ceil() as i64,
    })))
}

pub async fn get(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Submission>, AppError> {
    let submission = db::submissions::find_by_id_scoped(&state.pool, id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Submission not found".to_string()))?;
    Ok(Json(submission))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::submissions::delete(&state.pool, id, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "submission.deleted",
        "submission",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}

pub async fn bulk_delete(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify endpoint belongs to tenant
    db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let deleted = db::submissions::bulk_delete(&state.pool, endpoint_id, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "submissions.bulk_deleted",
        "endpoint",
        Some(endpoint_id),
        Some(serde_json::json!({ "count": deleted })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

pub async fn export(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
    Query(params): Query<ExportParams>,
) -> Result<impl IntoResponse, AppError> {
    // Verify endpoint belongs to tenant
    db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let submissions =
        db::submissions::list_for_export(&state.pool, endpoint_id, auth.tenant_id()).await?;

    match params.format.as_deref().unwrap_or("json") {
        "csv" => {
            let csv = export_csv(&submissions);
            Ok((
                [
                    (header::CONTENT_TYPE, "text/csv"),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"submissions.csv\"",
                    ),
                ],
                csv,
            )
                .into_response())
        }
        _ => Ok(Json(submissions).into_response()),
    }
}

fn export_csv(submissions: &[Submission]) -> String {
    use std::fmt::Write;
    let mut csv = String::new();

    // Collect all unique keys from data fields
    let mut keys: Vec<String> = Vec::new();
    for sub in submissions {
        if let Some(obj) = sub.data.as_object() {
            for key in obj.keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
    }

    // Header
    let _ = write!(csv, "id,created_at");
    for key in &keys {
        let _ = write!(csv, ",{key}");
    }
    let _ = writeln!(csv);

    // Rows
    for sub in submissions {
        let _ = write!(csv, "{},{}", sub.id, sub.created_at.to_rfc3339());
        for key in &keys {
            let val = sub
                .data
                .get(key)
                .map(|v| match v {
                    serde_json::Value::String(s) => csv_escape(s),
                    other => csv_escape(&other.to_string()),
                })
                .unwrap_or_default();
            let _ = write!(csv, ",{val}");
        }
        let _ = writeln!(csv);
    }

    csv
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
