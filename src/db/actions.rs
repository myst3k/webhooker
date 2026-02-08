use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Action;

pub async fn list_by_endpoint(
    pool: &PgPool,
    endpoint_id: Uuid,
) -> Result<Vec<Action>, sqlx::Error> {
    sqlx::query_as::<_, Action>(
        "SELECT * FROM actions WHERE endpoint_id = $1 ORDER BY position ASC",
    )
    .bind(endpoint_id)
    .fetch_all(pool)
    .await
}

pub async fn list_enabled_ordered(
    pool: &PgPool,
    endpoint_id: Uuid,
) -> Result<Vec<Action>, sqlx::Error> {
    sqlx::query_as::<_, Action>(
        "SELECT * FROM actions WHERE endpoint_id = $1 AND enabled = true ORDER BY position ASC",
    )
    .bind(endpoint_id)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &PgPool,
    endpoint_id: Uuid,
    action_type: &str,
    config: &serde_json::Value,
    position: i32,
) -> Result<Action, sqlx::Error> {
    sqlx::query_as::<_, Action>(
        "INSERT INTO actions (endpoint_id, action_type, config, position)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(endpoint_id)
    .bind(action_type)
    .bind(config)
    .bind(position)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Action>, sqlx::Error> {
    sqlx::query_as::<_, Action>("SELECT * FROM actions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_id_scoped(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<Action>, sqlx::Error> {
    sqlx::query_as::<_, Action>(
        "SELECT a.* FROM actions a
         JOIN endpoints e ON a.endpoint_id = e.id
         JOIN projects p ON e.project_id = p.id
         WHERE a.id = $1 AND p.tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
    action_type: &str,
    config: &serde_json::Value,
    position: i32,
    enabled: bool,
) -> Result<Action, sqlx::Error> {
    sqlx::query_as::<_, Action>(
        "UPDATE actions SET action_type = $3, config = $4, position = $5, enabled = $6
         WHERE id = $1 AND endpoint_id IN (
            SELECT e.id FROM endpoints e JOIN projects p ON e.project_id = p.id WHERE p.tenant_id = $2
         ) RETURNING *",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(action_type)
    .bind(config)
    .bind(position)
    .bind(enabled)
    .fetch_one(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM actions WHERE id = $1 AND endpoint_id IN (
            SELECT e.id FROM endpoints e JOIN projects p ON e.project_id = p.id WHERE p.tenant_id = $2
        )",
    )
    .bind(id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(())
}
