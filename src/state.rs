use std::sync::Arc;

use sqlx::PgPool;

use crate::actions::ModuleRegistry;
use crate::config::Config;
use crate::email::SystemMailer;
use crate::rate_limit::LoginRateLimiter;
use crate::rate_limit::SubmissionRateLimiter;

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub modules: ModuleRegistry,
    pub system_mailer: Option<Arc<SystemMailer>>,
    pub submission_limiter: SubmissionRateLimiter,
    pub login_limiter: LoginRateLimiter,
}
