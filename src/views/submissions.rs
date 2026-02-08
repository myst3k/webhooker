use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::state::SharedState;

#[allow(dead_code)]
struct SubmissionRow {
    id: String,
    cells: Vec<String>,
    created_at: String,
    extras: String,
    metadata: String,
    raw: String,
}

#[derive(Template)]
#[template(path = "dashboard/submissions_table.html")]
#[allow(dead_code)]
struct SubmissionsTableTemplate {
    rows: Vec<SubmissionRow>,
    endpoint_id: String,
    total: i64,
    page: i64,
    per_page: i64,
    total_pages: i64,
    sort_by: String,
    sort_order: String,
    search: String,
    field_names: Vec<String>,
}

#[derive(Deserialize)]
pub struct TableParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub search: Option<String>,
}

pub async fn table_partial(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
    Query(params): Query<TableParams>,
) -> Result<impl IntoResponse, AppError> {
    let endpoint = db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100).max(1);
    let offset = (page - 1) * per_page;
    let sort_by = params.sort_by.unwrap_or_else(|| "created_at".to_string());
    let sort_order = params.sort_order.unwrap_or_else(|| "desc".to_string());
    let search = params.search.clone().unwrap_or_default();

    let list_params = db::submissions::ListParams {
        endpoint_id,
        limit: per_page,
        offset,
        sort_by: sort_by.clone(),
        sort_order: sort_order.clone(),
        search: if search.is_empty() {
            None
        } else {
            Some(search.clone())
        },
    };

    let submissions = db::submissions::list(&state.pool, &list_params).await?;
    let total = db::submissions::count(
        &state.pool,
        endpoint_id,
        if search.is_empty() {
            None
        } else {
            Some(&search)
        },
    )
    .await?;

    let field_names: Vec<String> = endpoint
        .fields
        .as_ref()
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| {
            let mut keys = Vec::new();
            for sub in &submissions {
                if let Some(obj) = sub.data.as_object() {
                    for key in obj.keys() {
                        if !keys.contains(key) {
                            keys.push(key.clone());
                        }
                    }
                }
            }
            keys
        });

    // Pre-process submissions into rows
    let rows: Vec<SubmissionRow> = submissions
        .iter()
        .map(|sub| {
            let cells: Vec<String> = field_names
                .iter()
                .map(|col| {
                    sub.data
                        .get(col)
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Null => String::new(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default()
                })
                .collect();

            SubmissionRow {
                id: sub.id.to_string(),
                cells,
                created_at: sub.created_at.format("%Y-%m-%d %H:%M").to_string(),
                extras: serde_json::to_string_pretty(&sub.extras).unwrap_or_default(),
                metadata: serde_json::to_string_pretty(&sub.metadata).unwrap_or_default(),
                raw: serde_json::to_string_pretty(&sub.raw).unwrap_or_default(),
            }
        })
        .collect();

    let total_pages = ((total as f64) / (per_page as f64)).ceil() as i64;

    let template = SubmissionsTableTemplate {
        rows,
        endpoint_id: endpoint_id.to_string(),
        total,
        page,
        per_page,
        total_pages,
        sort_by,
        sort_order,
        search,
        field_names,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
