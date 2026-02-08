use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::models::{Tenant, User};
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "admin/tenants.html")]
#[allow(dead_code)]
struct TenantsTemplate {
    user_name: String,
    is_system_admin: bool,
    tenants: Vec<Tenant>,
}

#[derive(Template)]
#[template(path = "admin/users.html")]
#[allow(dead_code)]
struct UsersTemplate {
    user_name: String,
    is_system_admin: bool,
    users: Vec<User>,
    tenants: Vec<Tenant>,
}

pub async fn tenants_page(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    auth.require_system_admin()?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let tenants = db::tenants::list(&state.pool).await?;

    let template = TenantsTemplate {
        user_name: user,
        is_system_admin: true,
        tenants,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn users_page(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    auth.require_system_admin()?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let users = db::users::list_all(&state.pool).await?;
    let tenants = db::tenants::list(&state.pool).await?;

    let template = UsersTemplate {
        user_name: user,
        is_system_admin: true,
        users,
        tenants,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
