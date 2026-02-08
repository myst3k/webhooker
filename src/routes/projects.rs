use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::Project;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProject {
    pub name: String,
    pub slug: Option<String>,
}

pub async fn list(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<Vec<Project>>, AppError> {
    let projects = db::projects::list(&state.pool, auth.tenant_id()).await?;
    Ok(Json(projects))
}

pub async fn create(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    let slug = req.slug.unwrap_or_else(|| slugify(&req.name));
    validate_slug(&slug)?;

    let project = db::projects::create(&state.pool, auth.tenant_id(), &req.name, &slug)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                AppError::Conflict("A project with this slug already exists".to_string())
            }
            _ => AppError::Database(e),
        })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "project.created",
        "project",
        Some(project.id),
        None,
    )
    .await;

    Ok(Json(project))
}

pub async fn get(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, AppError> {
    let project = db::projects::find_by_id(&state.pool, id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;
    Ok(Json(project))
}

pub async fn update(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProject>,
) -> Result<Json<Project>, AppError> {
    let slug = req.slug.unwrap_or_else(|| slugify(&req.name));
    validate_slug(&slug)?;

    let project = db::projects::update(&state.pool, id, auth.tenant_id(), &req.name, &slug)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound("Project not found".to_string()),
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                AppError::Conflict("A project with this slug already exists".to_string())
            }
            _ => AppError::Database(e),
        })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "project.updated",
        "project",
        Some(project.id),
        None,
    )
    .await;

    Ok(Json(project))
}

pub async fn delete(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::projects::delete(&state.pool, id, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "project.deleted",
        "project",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
