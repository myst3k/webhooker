use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::auth::password;
use crate::crypto;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::models::{Tenant, User};
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct UpdateTenant {
    pub name: String,
    pub slug: String,
}

#[derive(Deserialize)]
pub struct AddMember {
    pub email: String,
    pub password: String,
    pub name: String,
    pub role: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMemberRole {
    pub role: String,
}

#[derive(Deserialize)]
pub struct ResetMemberPassword {
    pub password: String,
}

#[derive(Deserialize)]
pub struct SmtpConfigRequest {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub tls_mode: Option<String>,
}

#[derive(Deserialize)]
pub struct TestSmtpRequest {
    pub to: String,
}

pub async fn get_tenant(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<Tenant>, AppError> {
    let tenant = db::tenants::find_by_id(&state.pool, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Tenant not found".to_string()))?;
    Ok(Json(tenant))
}

pub async fn update_tenant(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<UpdateTenant>,
) -> Result<Json<Tenant>, AppError> {
    auth.require_owner_or_admin()?;

    let tenant = db::tenants::update(&state.pool, auth.tenant_id(), &req.name, &req.slug)
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
        "tenant.updated",
        "tenant",
        Some(tenant.id),
        None,
    )
    .await;

    Ok(Json(tenant))
}

pub async fn list_members(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<Vec<User>>, AppError> {
    let members = db::users::list_by_tenant(&state.pool, auth.tenant_id()).await?;
    Ok(Json(members))
}

pub async fn add_member(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<AddMember>,
) -> Result<Json<User>, AppError> {
    auth.require_owner_or_admin()?;

    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let pw_hash = password::hash(&req.password).map_err(|e| AppError::Internal(e))?;
    let role = req.role.as_deref().unwrap_or("member");

    let user = db::users::create(
        &state.pool,
        auth.tenant_id(),
        &req.email,
        &pw_hash,
        &req.name,
        role,
        false,
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("A user with this email already exists".to_string())
        }
        _ => AppError::Database(e),
    })?;

    // Send notifications
    if let Some(ref mailer) = state.system_mailer {
        let tenant = db::tenants::find_by_id(&state.pool, auth.tenant_id())
            .await?
            .unwrap();
        let _ = mailer
            .send_member_added(&user.email, &user.name, &tenant.name, &state.config.base_url)
            .await;
    }

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "member.added",
        "user",
        Some(user.id),
        None,
    )
    .await;

    Ok(Json(user))
}

pub async fn update_member_role(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMemberRole>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    // Verify member belongs to tenant
    let user = db::users::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if user.tenant_id != auth.tenant_id() {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    db::users::update_role(&state.pool, id, &req.role).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "member.role_updated",
        "user",
        Some(id),
        Some(serde_json::json!({ "new_role": req.role })),
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Role updated" })))
}

pub async fn remove_member(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    // Verify member belongs to tenant
    let user = db::users::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if user.tenant_id != auth.tenant_id() {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // Don't allow removing yourself
    if id == auth.user_id {
        return Err(AppError::BadRequest("Cannot remove yourself".to_string()));
    }

    db::users::delete(&state.pool, id).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "member.removed",
        "user",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Member removed" })))
}

pub async fn reset_member_password(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResetMemberPassword>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Verify member belongs to tenant
    let user = db::users::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if user.tenant_id != auth.tenant_id() {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    let pw_hash = password::hash(&req.password).map_err(|e| AppError::Internal(e))?;
    db::users::update_password(&state.pool, id, &pw_hash).await?;

    // Nuke their refresh tokens
    db::refresh_tokens::delete_all_for_user(&state.pool, id).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "member.password_reset",
        "user",
        Some(id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "Password reset" })))
}

// SMTP config routes
pub async fn get_smtp(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    let config = db::tenant_smtp::find_by_tenant(&state.pool, auth.tenant_id()).await?;

    match config {
        Some(c) => Ok(Json(serde_json::json!({
            "configured": true,
            "host": c.host,
            "port": c.port,
            "from_address": c.from_address,
            "from_name": c.from_name,
            "tls_mode": c.tls_mode,
        }))),
        None => Ok(Json(serde_json::json!({ "configured": false }))),
    }
}

pub async fn update_smtp(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<SmtpConfigRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    let username_enc =
        crypto::encrypt(&req.username, &state.config.encryption_key)
            .map_err(|e| AppError::Internal(e))?;
    let password_enc =
        crypto::encrypt(&req.password, &state.config.encryption_key)
            .map_err(|e| AppError::Internal(e))?;

    db::tenant_smtp::upsert(
        &state.pool,
        auth.tenant_id(),
        &req.host,
        req.port,
        &username_enc,
        &password_enc,
        &req.from_address,
        req.from_name.as_deref(),
        req.tls_mode.as_deref().unwrap_or("starttls"),
    )
    .await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "smtp.updated",
        "tenant",
        Some(auth.tenant_id()),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "SMTP configured" })))
}

pub async fn delete_smtp(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    db::tenant_smtp::delete(&state.pool, auth.tenant_id()).await?;

    audit::log_event(
        &state.pool,
        auth.tenant_id(),
        Some(auth.user_id),
        "smtp.deleted",
        "tenant",
        Some(auth.tenant_id()),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "message": "SMTP config removed" })))
}

pub async fn test_smtp(
    auth: AuthUser,
    State(state): State<SharedState>,
    Json(req): Json<TestSmtpRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth.require_owner_or_admin()?;

    let config = db::tenant_smtp::find_by_tenant(&state.pool, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::BadRequest("SMTP not configured".to_string()))?;

    // Decrypt credentials
    let username = crypto::decrypt(&config.username_enc, &state.config.encryption_key)
        .map_err(|e| AppError::Internal(e))?;
    let password_str = crypto::decrypt(&config.password_enc, &state.config.encryption_key)
        .map_err(|e| AppError::Internal(e))?;

    let smtp = crate::actions::email::TenantSmtp {
        host: config.host,
        port: config.port as u16,
        username,
        password: password_str,
        from_address: config.from_address,
        from_name: config.from_name,
        tls_mode: config.tls_mode,
    };

    let transport = crate::actions::email::build_smtp_transport(&smtp)
        .map_err(|e| AppError::Internal(e))?;

    use lettre::message::header::ContentType;
    use lettre::{AsyncTransport, Message};

    let from = if let Some(name) = &smtp.from_name {
        format!("{} <{}>", name, smtp.from_address)
    } else {
        smtp.from_address.clone()
    };

    let message = Message::builder()
        .from(from.parse().map_err(|e| AppError::Internal(format!("Invalid from: {e}")))?)
        .to(req.to.parse().map_err(|e| AppError::BadRequest(format!("Invalid to address: {e}")))?)
        .subject("Webhooker SMTP Test")
        .header(ContentType::TEXT_PLAIN)
        .body("This is a test email from Webhooker. Your SMTP configuration is working!".to_string())
        .map_err(|e| AppError::Internal(format!("Failed to build email: {e}")))?;

    transport
        .send(message)
        .await
        .map_err(|e| AppError::BadRequest(format!("SMTP test failed: {e}")))?;

    Ok(Json(serde_json::json!({ "message": "Test email sent successfully" })))
}
