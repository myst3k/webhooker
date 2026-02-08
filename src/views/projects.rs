use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::models::Endpoint;
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "dashboard/project.html")]
#[allow(dead_code)]
struct ProjectTemplate {
    user_name: String,
    is_system_admin: bool,
    project_name: String,
    project_id: String,
    endpoints: Vec<Endpoint>,
}

pub async fn show(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let project = db::projects::find_by_id(&state.pool, id, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    let endpoints = db::endpoints::list_by_project(&state.pool, project.id).await?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let template = ProjectTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        project_name: project.name,
        project_id: project.id.to_string(),
        endpoints,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
