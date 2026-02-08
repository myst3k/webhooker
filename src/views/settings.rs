use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::models::User;
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "settings/account.html")]
#[allow(dead_code)]
struct AccountTemplate {
    user_name: String,
    user_email: String,
    is_system_admin: bool,
}

#[derive(Template)]
#[template(path = "settings/smtp.html")]
#[allow(dead_code)]
struct SmtpTemplate {
    user_name: String,
    is_system_admin: bool,
    smtp_configured: bool,
    smtp_host: String,
    smtp_port: i32,
    smtp_from: String,
    smtp_from_name: String,
    smtp_tls: String,
}

#[derive(Template)]
#[template(path = "settings/members.html")]
#[allow(dead_code)]
struct MembersTemplate {
    user_name: String,
    is_system_admin: bool,
    is_owner: bool,
    members: Vec<User>,
}

pub async fn account_page(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let template = AccountTemplate {
        user_name: user.name.clone(),
        user_email: user.email,
        is_system_admin: auth.is_system_admin,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn smtp_page(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    auth.require_owner_or_admin()?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let config = db::tenant_smtp::find_by_tenant(&state.pool, auth.tenant_id()).await?;

    let template = SmtpTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        smtp_configured: config.is_some(),
        smtp_host: config.as_ref().map(|c| c.host.clone()).unwrap_or_default(),
        smtp_port: config.as_ref().map(|c| c.port).unwrap_or(587),
        smtp_from: config
            .as_ref()
            .map(|c| c.from_address.clone())
            .unwrap_or_default(),
        smtp_from_name: config
            .as_ref()
            .and_then(|c| c.from_name.clone())
            .unwrap_or_default(),
        smtp_tls: config
            .as_ref()
            .map(|c| c.tls_mode.clone())
            .unwrap_or_else(|| "starttls".to_string()),
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn members_page(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let members = db::users::list_by_tenant(&state.pool, auth.tenant_id()).await?;

    let template = MembersTemplate {
        user_name: user.name.clone(),
        is_system_admin: auth.is_system_admin,
        is_owner: auth.role == "owner",
        members,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
