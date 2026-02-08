use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TenantSmtpConfig {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub host: String,
    pub port: i32,
    #[serde(skip_serializing)]
    pub username_enc: Vec<u8>,
    #[serde(skip_serializing)]
    pub password_enc: Vec<u8>,
    pub from_address: String,
    pub from_name: Option<String>,
    pub tls_mode: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
