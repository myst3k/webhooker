use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::Endpoint;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct CreateEndpoint {
    pub name: String,
    pub slug: String,
    pub fields: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateEndpoint {
    pub name: String,
    pub slug: String,
    pub fields: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
}

pub async fn list_by_project(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Endpoint>>, AppError> {
    // Verify project belongs to tenant
    db::projects::find_by_id(&state.pool, project_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    let endpoints = db::endpoints::list_by_project(&state.pool, project_id).await?;
    Ok(Json(endpoints))
}

pub async fn create(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateEndpoint>,
) -> Result<Json<Endpoint>, AppError> {
    // Verify project belongs to tenant
    db::projects::find_by_id(&state.pool, project_id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    validate_slug(&req.slug)?;

    let endpoint = db::endpoints::create(
        &state.pool,
        project_id,
        &req.name,
        &req.slug,
        req.fields.as_ref(),
        req.settings.as_ref(),
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("An endpoint with this slug already exists in this project".to_string())
        }
        _ => AppError::Database(e),
    })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "endpoint.created",
        "endpoint",
        Some(endpoint.id),
        None,
    )
    .await;

    Ok(Json(endpoint))
}

pub async fn get(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Endpoint>, AppError> {
    let endpoint = db::endpoints::find_by_id_scoped(&state.pool, id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;
    Ok(Json(endpoint))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEndpoint>,
) -> Result<Json<Endpoint>, AppError> {
    validate_slug(&req.slug)?;

    let endpoint = db::endpoints::update(
        &state.pool,
        id,
        auth.tenant_id(),
        &req.name,
        &req.slug,
        req.fields.as_ref(),
        req.settings.as_ref(),
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::NotFound("Endpoint not found".to_string()),
        _ => AppError::Database(e),
    })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "endpoint.updated",
        "endpoint",
        Some(endpoint.id),
        None,
    )
    .await;

    Ok(Json(endpoint))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::endpoints::delete(&state.pool, id, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "endpoint.deleted",
        "endpoint",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty() || slug.len() > 100 {
        return Err(AppError::BadRequest(
            "Slug must be between 1 and 100 characters".to_string(),
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::BadRequest(
            "Slug must contain only lowercase letters, numbers, and hyphens".to_string(),
        ));
    }
    Ok(())
}
