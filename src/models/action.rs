use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Action {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub action_type: String,
    pub config: serde_json::Value,
    pub position: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
