use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::{Action, ActionLog};
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct CreateAction {
    pub action_type: String,
    pub config: serde_json::Value,
    pub position: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateAction {
    pub action_type: String,
    pub config: serde_json::Value,
    pub position: i32,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct LogParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_by_endpoint(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
) -> Result<Json<Vec<Action>>, AppError> {
    // Verify endpoint belongs to tenant
    db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let actions = db::actions::list_by_endpoint(&state.pool, endpoint_id).await?;
    Ok(Json(actions))
}

pub async fn create(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(endpoint_id): Path<Uuid>,
    Json(req): Json<CreateAction>,
) -> Result<Json<Action>, AppError> {
    // Verify endpoint belongs to tenant
    db::endpoints::find_by_id_scoped(&state.pool, endpoint_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    // Validate config against module
    if let Some(module) = state.modules.get(&req.action_type) {
        module
            .validate_config(&req.config)
            .map_err(|e| AppError::BadRequest(e.message))?;
    } else {
        return Err(AppError::BadRequest(format!(
            "Unknown action type: {}",
            req.action_type
        )));
    }

    let action = db::actions::create(
        &state.pool,
        endpoint_id,
        &req.action_type,
        &req.config,
        req.position.unwrap_or(0),
    )
    .await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "action.created",
        "action",
        Some(action.id),
        None,
    )
    .await;

    Ok(Json(action))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAction>,
) -> Result<Json<Action>, AppError> {
    // Validate config
    if let Some(module) = state.modules.get(&req.action_type) {
        module
            .validate_config(&req.config)
            .map_err(|e| AppError::BadRequest(e.message))?;
    }

    let action = db::actions::update(
        &state.pool,
        id,
        auth.tenant_id(),
        &req.action_type,
        &req.config,
        req.position,
        req.enabled,
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Action not found".to_string()),
        _ => AppError::Database(e),
    })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "action.updated",
        "action",
        Some(action.id),
        None,
    )
    .await;

    Ok(Json(action))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::actions::delete(&state.pool, id, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "action.deleted",
        "action",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}

pub async fn log(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(params): Query<LogParams>,
) -> Result<Json<Vec<ActionLog>>, AppError> {
    // Verify action belongs to tenant
    db::actions::find_by_id_scoped(&state.pool, id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Action not found".to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100).max(1);
    let offset = (page - 1) * per_page;

    let logs = db::action_log::list_by_action(&state.pool, id, per_page, offset).await?;
    Ok(Json(logs))
}
