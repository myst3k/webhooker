use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse};

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "dashboard/index.html")]
#[allow(dead_code)]
struct DashboardTemplate {
    user_name: String,
    is_system_admin: bool,
    projects: Vec<ProjectWithCount>,
}

#[allow(dead_code)]
struct ProjectWithCount {
    id: String,
    name: String,
    slug: String,
    endpoint_count: i64,
    created_at: String,
}

pub async fn index(
    auth: AuthUser,
    State(state): State<SharedState>,
) -> Result<impl IntoResponse, AppError> {
    let projects = db::projects::list(&state.pool, auth.tenant_id()).await?;

    let mut project_list = Vec::new();
    for project in &projects {
        let endpoints = db::endpoints::list_by_project(&state.pool, project.id).await?;
        project_list.push(ProjectWithCount {
            id: project.id.to_string(),
            name: project.name.clone(),
            slug: project.slug.clone(),
            endpoint_count: endpoints.len() as i64,
            created_at: project.created_at.format("%Y-%m-%d").to_string(),
        });
    }

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let template = DashboardTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        projects: project_list,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
