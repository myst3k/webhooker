use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Submission;

pub async fn create(
    pool: &PgPool,
    endpoint_id: Uuid,
    data: &serde_json::Value,
    extras: &serde_json::Value,
    raw: &serde_json::Value,
    metadata: &serde_json::Value,
) -> Result<Submission, sqlx::Error> {
    sqlx::query_as::<_, Submission>(
        "INSERT INTO submissions (endpoint_id, data, extras, raw, metadata)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(endpoint_id)
    .bind(data)
    .bind(extras)
    .bind(raw)
    .bind(metadata)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Submission>, sqlx::Error> {
    sqlx::query_as::<_, Submission>("SELECT * FROM submissions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_id_scoped(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<Submission>, sqlx::Error> {
    sqlx::query_as::<_, Submission>(
        "SELECT s.* FROM submissions s
         JOIN endpoints e ON s.endpoint_id = e.id
         JOIN projects p ON e.project_id = p.id
         WHERE s.id = $1 AND p.tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub struct ListParams {
    pub endpoint_id: Uuid,
    pub limit: i64,
    pub offset: i64,
    pub sort_by: String,
    pub sort_order: String,
    pub search: Option<String>,
}

pub async fn list(pool: &PgPool, params: &ListParams) -> Result<Vec<Submission>, sqlx::Error> {
    let order = if params.sort_order == "asc" {
        "ASC"
    } else {
        "DESC"
    };

    let sort_col = match params.sort_by.as_str() {
        "created_at" => "created_at",
        "id" => "id",
        _ => "created_at",
    };

    let query = if let Some(search) = &params.search {
        let search_pattern = format!("%{search}%");
        sqlx::query_as::<_, Submission>(&format!(
            "SELECT * FROM submissions
             WHERE endpoint_id = $1 AND (data::text ILIKE $4 OR extras::text ILIKE $4)
             ORDER BY {sort_col} {order} LIMIT $2 OFFSET $3"
        ))
        .bind(params.endpoint_id)
        .bind(params.limit)
        .bind(params.offset)
        .bind(search_pattern)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, Submission>(&format!(
            "SELECT * FROM submissions
             WHERE endpoint_id = $1
             ORDER BY {sort_col} {order} LIMIT $2 OFFSET $3"
        ))
        .bind(params.endpoint_id)
        .bind(params.limit)
        .bind(params.offset)
        .fetch_all(pool)
        .await
    };

    query
}

pub async fn count(
    pool: &PgPool,
    endpoint_id: Uuid,
    search: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let row: (i64,) = if let Some(search) = search {
        let search_pattern = format!("%{search}%");
        sqlx::query_as(
            "SELECT COUNT(*) FROM submissions
             WHERE endpoint_id = $1 AND (data::text ILIKE $2 OR extras::text ILIKE $2)",
        )
        .bind(endpoint_id)
        .bind(search_pattern)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM submissions WHERE endpoint_id = $1")
            .bind(endpoint_id)
            .fetch_one(pool)
            .await?
    };
    Ok(row.0)
}

pub async fn delete(pool: &PgPool, id: Uuid, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM submissions WHERE id = $1 AND endpoint_id IN (
            SELECT e.id FROM endpoints e JOIN projects p ON e.project_id = p.id WHERE p.tenant_id = $2
        )",
    )
    .bind(id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn bulk_delete(
    pool: &PgPool,
    endpoint_id: Uuid,
    tenant_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM submissions WHERE endpoint_id = $1 AND endpoint_id IN (
            SELECT e.id FROM endpoints e JOIN projects p ON e.project_id = p.id WHERE p.tenant_id = $2
        )",
    )
    .bind(endpoint_id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_for_export(
    pool: &PgPool,
    endpoint_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<Submission>, sqlx::Error> {
    sqlx::query_as::<_, Submission>(
        "SELECT s.* FROM submissions s
         JOIN endpoints e ON s.endpoint_id = e.id
         JOIN projects p ON e.project_id = p.id
         WHERE s.endpoint_id = $1 AND p.tenant_id = $2
         ORDER BY s.created_at DESC",
    )
    .bind(endpoint_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}
