use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::extract::Request;

/// Middleware that redirects 401 responses to `/auth/login` for browser requests.
pub async fn redirect_unauthorized(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    if response.status() == StatusCode::UNAUTHORIZED {
        Redirect::to("/auth/login").into_response()
    } else {
        response
    }
}
