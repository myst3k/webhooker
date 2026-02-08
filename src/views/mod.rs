pub mod auth;
pub mod dashboard;
pub mod projects;
pub mod endpoints;
pub mod submissions;
pub mod settings;
pub mod admin;

use axum::routing::get;
use axum::Router;

use crate::state::SharedState;

pub fn view_routes() -> Router<SharedState> {
    Router::new()
        // Auth views
        .route("/", get(auth::login_page))
        .route("/auth/login", get(auth::login_page))
        .route("/auth/forgot-password", get(auth::forgot_password_page))
        .route("/auth/reset-password", get(auth::reset_password_page))
        // Dashboard
        .route("/dashboard", get(dashboard::index))
        // Projects
        .route("/projects/{slug}", get(projects::show))
        // Endpoints
        .route("/endpoints/{slug}", get(endpoints::submissions_page))
        .route("/endpoints/{slug}/settings", get(endpoints::settings_page))
        .route("/endpoints/{slug}/actions", get(endpoints::actions_page))
        .route("/endpoints/{slug}/snippet", get(endpoints::snippet_page))
        // Settings
        .route("/settings", get(settings::account_page))
        .route("/settings/smtp", get(settings::smtp_page))
        .route("/settings/members", get(settings::members_page))
        // Admin
        .route("/admin/tenants", get(admin::tenants_page))
        .route("/admin/users", get(admin::users_page))
        // HTMX partials
        .route("/htmx/submissions/{endpoint_id}", get(submissions::table_partial))
}
