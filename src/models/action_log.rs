use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ActionLog {
    pub id: Uuid,
    pub action_id: Uuid,
    pub submission_id: Uuid,
    pub status: String,
    pub response: Option<serde_json::Value>,
    pub executed_at: DateTime<Utc>,
}
