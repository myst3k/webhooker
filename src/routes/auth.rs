use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::extractor::AuthUser;
use crate::auth::jwt::{Claims, encode_token};
use crate::auth::password;
use crate::db;
use crate::error::AppError;
use crate::middleware::audit;
use crate::state::SharedState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

fn generate_refresh_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub async fn register(
    State(state): State<SharedState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Only allow registration if no users exist (bootstrap)
    let count = db::users::count_all(&state.pool).await?;
    if count > 0 {
        return Err(AppError::Forbidden(
            "Registration is disabled. Contact your system administrator.".to_string(),
        ));
    }

    if req.email.is_empty() || req.password.is_empty() || req.name.is_empty() {
        return Err(AppError::BadRequest("All fields are required".to_string()));
    }

    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let pw_hash =
        password::hash(&req.password).map_err(|e| AppError::Internal(e))?;

    // Create default tenant
    let slug = slugify(&req.name);
    let tenant = db::tenants::create(&state.pool, &format!("{}'s Workspace", req.name), &slug)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create tenant: {e}")))?;

    // Create system admin + owner user
    let user = db::users::create(
        &state.pool,
        tenant.id,
        &req.email,
        &pw_hash,
        &req.name,
        "owner",
        true,
    )
    .await?;

    // Generate tokens
    let claims = Claims::new(user.id, tenant.id, "owner".to_string(), true);
    let access_token = encode_token(&claims, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(e))?;

    let refresh = generate_refresh_token();
    let refresh_hash = hash_token(&refresh);
    db::refresh_tokens::create(
        &state.pool,
        user.id,
        &refresh_hash,
        Utc::now() + Duration::days(7),
    )
    .await?;

    audit::log_event(
        &state.pool,
        tenant.id,
        Some(user.id),
        "user.registered",
        "user",
        Some(user.id),
        None,
    )
    .await;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: refresh,
    }))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Rate limit check
    if let Err(_) = state.login_limiter.check(&req.email) {
        return Err(AppError::RateLimited(
            "Too many login attempts. Please try again later.".to_string(),
        ));
    }

    let user = db::users::find_by_email(&state.pool, &req.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    let valid = password::verify(&req.password, &user.password_hash)
        .map_err(|e| AppError::Internal(e))?;

    if !valid {
        state.login_limiter.record_failure(&req.email);
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    let claims = Claims::new(
        user.id,
        user.tenant_id,
        user.role.clone(),
        user.is_system_admin,
    );
    let access_token = encode_token(&claims, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(e))?;

    let refresh = generate_refresh_token();
    let refresh_hash = hash_token(&refresh);
    db::refresh_tokens::create(
        &state.pool,
        user.id,
        &refresh_hash,
        Utc::now() + Duration::days(7),
    )
    .await?;

    audit::log_event(
        &state.pool,
        user.tenant_id,
        Some(user.id),
        "user.login",
        "user",
        Some(user.id),
        None,
    )
    .await;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: refresh,
    }))
}

pub async fn refresh(
    State(state): State<SharedState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let token_hash = hash_token(&req.refresh_token);

    let stored = db::refresh_tokens::find_by_hash(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid refresh token".to_string()))?;

    // Reuse detection: if token was already used, nuke ALL tokens for user
    if stored.used {
        tracing::warn!(
            "Refresh token reuse detected for user {}. Nuking all sessions.",
            stored.user_id
        );
        db::refresh_tokens::delete_all_for_user(&state.pool, stored.user_id).await?;
        return Err(AppError::Unauthorized(
            "Refresh token reuse detected. All sessions revoked.".to_string(),
        ));
    }

    // Check expiry
    if stored.expires_at < Utc::now() {
        return Err(AppError::Unauthorized("Refresh token expired".to_string()));
    }

    // Mark as used
    db::refresh_tokens::mark_used(&state.pool, stored.id).await?;

    // Load user
    let user = db::users::find_by_id(&state.pool, stored.user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    // Issue new tokens
    let claims = Claims::new(
        user.id,
        user.tenant_id,
        user.role.clone(),
        user.is_system_admin,
    );
    let access_token = encode_token(&claims, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(e))?;

    let new_refresh = generate_refresh_token();
    let new_refresh_hash = hash_token(&new_refresh);
    db::refresh_tokens::create(
        &state.pool,
        user.id,
        &new_refresh_hash,
        Utc::now() + Duration::days(7),
    )
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: new_refresh,
    }))
}

pub async fn logout(
    State(state): State<SharedState>,
    Json(req): Json<LogoutRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    let token_hash = hash_token(&req.refresh_token);
    db::refresh_tokens::delete_by_hash(&state.pool, &token_hash).await?;

    Ok(Json(MessageResponse {
        message: "Logged out successfully".to_string(),
    }))
}

pub async fn forgot_password(
    State(state): State<SharedState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    // Always return 200 to not reveal whether email exists
    let response = Json(MessageResponse {
        message: "If that email is registered, a reset link has been sent.".to_string(),
    });

    // Background: look up user and send email
    let pool = state.pool.clone();
    let mailer = state.system_mailer.clone();
    let base_url = state.config.base_url.clone();

    tokio::spawn(async move {
        if let Ok(Some(user)) = db::users::find_by_email(&pool, &req.email).await {
            let token = generate_refresh_token();
            let token_hash = hash_token(&token);

            if let Ok(_) = db::password_reset_tokens::create(
                &pool,
                user.id,
                &token_hash,
                Utc::now() + Duration::hours(1),
            )
            .await
            {
                if let Some(mailer) = mailer {
                    let reset_url = format!("{base_url}/auth/reset-password?token={token}");
                    if let Err(e) = mailer.send_password_reset(&user.email, &reset_url).await {
                        tracing::error!("Failed to send password reset email: {e}");
                    }
                } else {
                    tracing::warn!("System SMTP not configured. Password reset token: {token}");
                }
            }
        }
    });

    Ok(response)
}

pub async fn reset_password(
    State(state): State<SharedState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let token_hash = hash_token(&req.token);

    let reset_token = db::password_reset_tokens::find_valid_by_hash(&state.pool, &token_hash)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired reset token".to_string()))?;

    // Mark token as used
    db::password_reset_tokens::mark_used(&state.pool, reset_token.id).await?;

    // Update password
    let pw_hash =
        password::hash(&req.password).map_err(|e| AppError::Internal(e))?;
    db::users::update_password(&state.pool, reset_token.user_id, &pw_hash).await?;

    // Nuke all refresh tokens
    db::refresh_tokens::delete_all_for_user(&state.pool, reset_token.user_id).await?;

    Ok(Json(MessageResponse {
        message: "Password reset successfully".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    if req.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    let valid = password::verify(&req.current_password, &user.password_hash)
        .map_err(|e| AppError::Internal(e))?;

    if !valid {
        return Err(AppError::Unauthorized(
            "Current password is incorrect".to_string(),
        ));
    }

    let pw_hash = password::hash(&req.new_password).map_err(|e| AppError::Internal(e))?;
    db::users::update_password(&state.pool, user.id, &pw_hash).await?;

    // Nuke all existing refresh tokens
    db::refresh_tokens::delete_all_for_user(&state.pool, user.id).await?;

    // Issue fresh tokens
    let claims = Claims::new(
        user.id,
        user.tenant_id,
        user.role.clone(),
        user.is_system_admin,
    );
    let access_token = encode_token(&claims, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(e))?;

    let refresh = generate_refresh_token();
    let refresh_hash = hash_token(&refresh);
    db::refresh_tokens::create(
        &state.pool,
        user.id,
        &refresh_hash,
        Utc::now() + Duration::days(7),
    )
    .await?;

    audit::log_event(
        &state.pool,
        user.tenant_id,
        Some(user.id),
        "user.password_changed",
        "user",
        Some(user.id),
        None,
    )
    .await;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: refresh,
    }))
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
