pub mod auth;
pub mod projects;
pub mod endpoints;
pub mod submissions;
pub mod actions;
pub mod admin;
pub mod tenant;
pub mod modules;
pub mod ingest;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::SharedState;

pub fn api_routes() -> Router<SharedState> {
    Router::new()
        // Auth
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/forgot-password", post(auth::forgot_password))
        .route("/api/v1/auth/reset-password", post(auth::reset_password))
        .route("/api/v1/auth/change-password", post(auth::change_password))
        // Projects
        .route("/api/v1/projects", get(projects::list).post(projects::create))
        .route(
            "/api/v1/projects/{id}",
            get(projects::get)
                .put(projects::update)
                .delete(projects::delete),
        )
        // Endpoints
        .route(
            "/api/v1/projects/{id}/endpoints",
            get(endpoints::list_by_project).post(endpoints::create),
        )
        .route(
            "/api/v1/endpoints/{id}",
            get(endpoints::get)
                .put(endpoints::update)
                .delete(endpoints::delete),
        )
        // Submissions
        .route(
            "/api/v1/endpoints/{id}/submissions",
            get(submissions::list).delete(submissions::bulk_delete),
        )
        .route(
            "/api/v1/endpoints/{id}/submissions/export",
            get(submissions::export),
        )
        .route(
            "/api/v1/submissions/{id}",
            get(submissions::get).delete(submissions::delete),
        )
        // Actions
        .route(
            "/api/v1/endpoints/{id}/actions",
            get(actions::list_by_endpoint).post(actions::create),
        )
        .route(
            "/api/v1/actions/{id}",
            put(actions::update).delete(actions::delete),
        )
        .route("/api/v1/actions/{id}/log", get(actions::log))
        // Modules
        .route("/api/v1/modules", get(modules::list_modules))
        // Admin
        .route(
            "/api/v1/admin/tenants",
            get(admin::list_tenants).post(admin::create_tenant),
        )
        .route(
            "/api/v1/admin/tenants/{id}",
            get(admin::get_tenant).delete(admin::delete_tenant),
        )
        .route(
            "/api/v1/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route("/api/v1/admin/users/{id}", delete(admin::delete_user))
        // Tenant
        .route(
            "/api/v1/tenant",
            get(tenant::get_tenant).put(tenant::update_tenant),
        )
        .route(
            "/api/v1/tenant/members",
            get(tenant::list_members).post(tenant::add_member),
        )
        .route(
            "/api/v1/tenant/members/{id}",
            put(tenant::update_member_role).delete(tenant::remove_member),
        )
        .route(
            "/api/v1/tenant/members/{id}/reset-password",
            post(tenant::reset_member_password),
        )
        // Tenant SMTP
        .route(
            "/api/v1/tenant/smtp",
            get(tenant::get_smtp)
                .put(tenant::update_smtp)
                .delete(tenant::delete_smtp),
        )
        .route("/api/v1/tenant/smtp/test", post(tenant::test_smtp))
}

pub fn ingest_routes() -> Router<SharedState> {
    Router::new()
        .route("/v1/e/{endpoint_id}", post(ingest::ingest))
        .route("/v1/e/{endpoint_id}", axum::routing::options(ingest::ingest_options))
}
