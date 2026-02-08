use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use crate::auth::jwt;
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/forgot_password.html")]
struct ForgotPasswordTemplate {
    message: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/reset_password.html")]
struct ResetPasswordTemplate {
    token: String,
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct ResetQuery {
    pub token: Option<String>,
}

pub async fn login_page(
    State(state): State<SharedState>,
    jar: CookieJar,
) -> Response {
    // If already logged in, redirect to dashboard
    if let Some(cookie) = jar.get("access_token") {
        if jwt::decode_token(cookie.value(), &state.config.jwt_secret).is_ok() {
            return Redirect::to("/dashboard").into_response();
        }
    }

    let template = LoginTemplate { error: None };
    Html(template.render().unwrap_or_default()).into_response()
}

pub async fn forgot_password_page() -> impl IntoResponse {
    let template = ForgotPasswordTemplate { message: None };
    Html(template.render().unwrap_or_default())
}

pub async fn reset_password_page(Query(q): Query<ResetQuery>) -> impl IntoResponse {
    let token = q.token.unwrap_or_default();
    let template = ResetPasswordTemplate {
        token,
        error: None,
    };
    Html(template.render().unwrap_or_default())
}
