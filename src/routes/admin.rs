use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::auth::password;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::{Tenant, User};
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct CreateTenant {
    pub name: String,
    pub slug: String,
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub tenant_id: Uuid,
    pub email: String,
    pub password: String,
    pub name: String,
    pub role: String,
}

pub async fn list_tenants(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<Vec<Tenant>>, AppError> {
    auth.require_system_admin()?;
    let tenants = db::tenants::list(&state.pool).await?;
    Ok(Json(tenants))
}

pub async fn create_tenant(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateTenant>,
) -> Result<Json<Tenant>, AppError> {
    auth.require_system_admin()?;

    let tenant = db::tenants::create(&state.pool, &req.name, &req.slug)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                AppError::Conflict("A tenant with this slug already exists".to_string())
            }
            _ => AppError::Database(e),
        })?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "tenant.created",
        "tenant",
        Some(tenant.id),
        None,
    )
    .await;

    Ok(Json(tenant))
}

pub async fn get_tenant(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_system_admin()?;

    let tenant = db::tenants::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tenant not found".to_string()))?;

    let members = db::users::list_by_tenant(&state.pool, id).await?;

    Ok(Json(serde_json::json!({
        "tenant": tenant,
        "members": members,
    })))
}

pub async fn delete_tenant(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_system_admin()?;

    db::tenants::delete(&state.pool, id).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "tenant.deleted",
        "tenant",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}

pub async fn list_users(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<Vec<User>>, AppError> {
    auth.require_system_admin()?;
    let users = db::users::list_all(&state.pool).await?;
    Ok(Json(users))
}

pub async fn create_user(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<CreateUser>,
) -> Result<Json<User>, AppError> {
    auth.require_system_admin()?;

    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Verify tenant exists
    db::tenants::find_by_id(&state.pool, req.tenant_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Tenant not found".to_string()))?;

    let pw_hash = password::hash(&req.password).map_err(|e| AppError::Internal(e))?;

    let user = db::users::create(
        &state.pool,
        req.tenant_id,
        &req.email,
        &pw_hash,
        &req.name,
        &req.role,
        false,
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("A user with this email already exists".to_string())
        }
        _ => AppError::Database(e),
    })?;

    // Send welcome email
    if let Some(ref mailer) = state.system_mailer {
        let _ = mailer
            .send_welcome(&user.email, &user.name, &state.config.base_url)
            .await;
    }

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "user.created",
        "user",
        Some(user.id),
        None,
    )
    .await;

    Ok(Json(user))
}

pub async fn delete_user(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_system_admin()?;

    db::users::delete(&state.pool, id).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "user.deleted",
        "user",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Deleted" })))
}
