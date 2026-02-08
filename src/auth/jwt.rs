use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub tid: Uuid,
    pub role: String,
    pub sys: bool,
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: Uuid, tenant_id: Uuid, role: String, is_system_admin: bool) -> Self {
        Self {
            sub: user_id,
            tid: tenant_id,
            role,
            sys: is_system_admin,
            exp: (Utc::now() + Duration::minutes(15)).timestamp(),
        }
    }
}

pub fn encode_token(claims: &Claims, secret: &str) -> Result<String, String> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode failed: {e}"))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("JWT decode failed: {e}"))
}
