use sqlx::PgPool;
use uuid::Uuid;

use crate::models::ActionLog;

pub async fn create(
    pool: &PgPool,
    action_id: Uuid,
    submission_id: Uuid,
    status: &str,
    response: Option<&serde_json::Value>,
) -> Result<ActionLog, sqlx::Error> {
    sqlx::query_as::<_, ActionLog>(
        "INSERT INTO action_log (action_id, submission_id, status, response)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(action_id)
    .bind(submission_id)
    .bind(status)
    .bind(response)
    .fetch_one(pool)
    .await
}

pub async fn list_by_action(
    pool: &PgPool,
    action_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<ActionLog>, sqlx::Error> {
    sqlx::query_as::<_, ActionLog>(
        "SELECT * FROM action_log WHERE action_id = $1
         ORDER BY executed_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(action_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}
