use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use uuid::Uuid;

use crate::auth::jwt;
use crate::error::AppError;
use crate::state::SharedState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub role: String,
    pub is_system_admin: bool,
}

impl AuthUser {
    pub fn require_system_admin(&self) -> Result<(), AppError> {
        if self.is_system_admin {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "System admin access required".to_string(),
            ))
        }
    }

    pub fn require_owner_or_admin(&self) -> Result<(), AppError> {
        if self.is_system_admin || self.role == "owner" {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Owner or admin access required".to_string(),
            ))
        }
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }
}

impl FromRequestParts<SharedState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        // Try Bearer token from Authorization header first
        if let Some(auth_header) = parts.headers.get("authorization") {
            let auth_str = auth_header
                .to_str()
                .map_err(|_| AppError::Unauthorized("Invalid authorization header".to_string()))?;

            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let claims = jwt::decode_token(token, &state.config.jwt_secret)
                    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

                return Ok(AuthUser {
                    user_id: claims.sub,
                    tenant_id: claims.tid,
                    role: claims.role,
                    is_system_admin: claims.sys,
                });
            }
        }

        // Try cookie-based auth
        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = jar.get("access_token") {
            let claims = jwt::decode_token(cookie.value(), &state.config.jwt_secret)
                .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

            return Ok(AuthUser {
                user_id: claims.sub,
                tenant_id: claims.tid,
                role: claims.role,
                is_system_admin: claims.sys,
            });
        }

        Err(AppError::Unauthorized(
            "Missing authentication token".to_string(),
        ))
    }
}
