pub mod config;
pub mod error;
pub mod state;
pub mod auth;
pub mod db;
pub mod models;
pub mod middleware;
pub mod routes;
pub mod views;
pub mod actions;
pub mod email;
pub mod submission;
pub mod crypto;
pub mod rate_limit;

use std::sync::Arc;

use axum::http::{HeaderName, HeaderValue};
use axum::Router;
use sqlx::PgPool;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::middleware::auth_redirect::redirect_unauthorized;

use crate::actions::webhook::WebhookModule;
use crate::actions::email::EmailModule;
use crate::actions::ModuleRegistry;
use crate::config::Config;
use crate::email::SystemMailer;
use crate::rate_limit::{LoginRateLimiter, SubmissionRateLimiter};
use crate::state::{AppState, SharedState};

pub fn build_app(pool: PgPool, config: Config) -> Router {
    // Build module registry
    let mut modules = ModuleRegistry::new();
    modules.register(Arc::new(WebhookModule::new()));
    modules.register(Arc::new(EmailModule::new(config.encryption_key.clone())));

    // Build system mailer
    let system_mailer = config.smtp.as_ref().and_then(|smtp| {
        match SystemMailer::new(smtp) {
            Ok(mailer) => {
                tracing::info!("System SMTP configured");
                Some(Arc::new(mailer))
            }
            Err(e) => {
                tracing::warn!("System SMTP not available: {e}");
                None
            }
        }
    });

    let state: SharedState = Arc::new(AppState {
        pool,
        config,
        modules,
        system_mailer,
        submission_limiter: SubmissionRateLimiter::new(),
        login_limiter: LoginRateLimiter::new(),
    });

    // Security headers
    let security_headers = Router::new()
        .merge(routes::api_routes())
        .merge(routes::ingest_routes())
        .merge(views::view_routes().layer(axum::middleware::from_fn(redirect_unauthorized)))
        .nest_service("/static", ServeDir::new("static"))
        .route("/health", axum::routing::get(health))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .with_state(state);

    security_headers
}

async fn health() -> &'static str {
    "ok"
}
