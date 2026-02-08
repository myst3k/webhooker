use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Submission {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub data: serde_json::Value,
    pub extras: serde_json::Value,
    pub raw: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
