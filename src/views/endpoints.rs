use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::AppError;
use crate::models::{Action, Endpoint};
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "dashboard/submissions.html")]
#[allow(dead_code)]
struct SubmissionsTemplate {
    user_name: String,
    is_system_admin: bool,
    endpoint: Endpoint,
    endpoint_id: String,
    project_name: String,
    base_url: String,
}

#[derive(Template)]
#[template(path = "dashboard/endpoint_settings.html")]
#[allow(dead_code)]
struct EndpointSettingsTemplate {
    user_name: String,
    is_system_admin: bool,
    endpoint: Endpoint,
    endpoint_id: String,
    rate_limit: u64,
    rate_limit_window: u64,
    cors_origins: String,
    honeypot_field: String,
    store_metadata: bool,
    redirect_url: String,
    retention_days: String,
    field_defs: Vec<FieldDef>,
}

#[derive(Template)]
#[template(path = "dashboard/actions.html")]
#[allow(dead_code)]
struct ActionsTemplate {
    user_name: String,
    is_system_admin: bool,
    endpoint: Endpoint,
    endpoint_id: String,
    actions: Vec<Action>,
    available_modules: Vec<ModuleInfo>,
}

#[allow(dead_code)]
struct ModuleInfo {
    id: String,
    name: String,
}

#[derive(Template)]
#[template(path = "dashboard/snippet.html")]
#[allow(dead_code)]
struct SnippetTemplate {
    user_name: String,
    is_system_admin: bool,
    endpoint: Endpoint,
    endpoint_id: String,
    base_url: String,
    fields: Vec<FieldDef>,
}

#[allow(dead_code)]
struct FieldDef {
    name: String,
    field_type: String,
    required: bool,
    label: String,
}

pub async fn submissions_page(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let endpoint = db::endpoints::find_by_slug_scoped(&state.pool, &slug, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let project = db::projects::find_by_id_unscoped(&state.pool, endpoint.project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".to_string()))?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let template = SubmissionsTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        endpoint_id: endpoint.id.to_string(),
        endpoint,
        project_name: project.name,
        base_url: state.config.base_url.clone(),
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn settings_page(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let endpoint = db::endpoints::find_by_slug_scoped(&state.pool, &slug, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let s = endpoint.settings.as_ref();

    let rate_limit = s
        .and_then(|v| v.get("rate_limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(10);
    let rate_limit_window = s
        .and_then(|v| v.get("rate_limit_window_secs"))
        .and_then(|v| v.as_u64())
        .unwrap_or(60);
    let cors_origins = s
        .and_then(|v| v.get("cors_origins"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let honeypot_field = s
        .and_then(|v| v.get("honeypot_field"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let store_metadata = s
        .and_then(|v| v.get("store_metadata"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let redirect_url = s
        .and_then(|v| v.get("redirect_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let retention_days = s
        .and_then(|v| v.get("retention_days"))
        .and_then(|v| v.as_u64())
        .map(|d| d.to_string())
        .unwrap_or_default();

    let field_defs: Vec<FieldDef> = endpoint
        .fields
        .as_ref()
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    Some(FieldDef {
                        name: f.get("name")?.as_str()?.to_string(),
                        field_type: f
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("text")
                            .to_string(),
                        required: f.get("required").and_then(|r| r.as_bool()).unwrap_or(false),
                        label: f
                            .get("label")
                            .and_then(|l| l.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let template = EndpointSettingsTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        endpoint_id: endpoint.id.to_string(),
        endpoint,
        rate_limit,
        rate_limit_window,
        cors_origins,
        honeypot_field,
        store_metadata,
        redirect_url,
        retention_days,
        field_defs,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn actions_page(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let endpoint = db::endpoints::find_by_slug_scoped(&state.pool, &slug, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let actions = db::actions::list_by_endpoint(&state.pool, endpoint.id).await?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let available_modules: Vec<ModuleInfo> = state
        .modules
        .list()
        .iter()
        .map(|m| ModuleInfo {
            id: m.id().to_string(),
            name: m.name().to_string(),
        })
        .collect();

    let template = ActionsTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        endpoint_id: endpoint.id.to_string(),
        endpoint,
        actions,
        available_modules,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn snippet_page(
    auth: AuthUser,
    State(state): State<SharedState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let endpoint = db::endpoints::find_by_slug_scoped(&state.pool, &slug, auth.tenant_id())
        .await?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    let user = db::users::find_by_id(&state.pool, auth.user_id)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();

    let fields: Vec<FieldDef> = endpoint
        .fields
        .as_ref()
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    Some(FieldDef {
                        name: f.get("name")?.as_str()?.to_string(),
                        field_type: f
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("text")
                            .to_string(),
                        required: f.get("required").and_then(|r| r.as_bool()).unwrap_or(false),
                        label: f
                            .get("label")
                            .and_then(|l| l.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let template = SnippetTemplate {
        user_name: user,
        is_system_admin: auth.is_system_admin,
        endpoint_id: endpoint.id.to_string(),
        endpoint,
        base_url: state.config.base_url.clone(),
        fields,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
