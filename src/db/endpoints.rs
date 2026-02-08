use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Endpoint;

pub async fn list_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Endpoint>, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "SELECT * FROM endpoints WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &PgPool,
    project_id: Uuid,
    name: &str,
    slug: &str,
    fields: Option<&serde_json::Value>,
    settings: Option<&serde_json::Value>,
) -> Result<Endpoint, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "INSERT INTO endpoints (project_id, name, slug, fields, settings)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(project_id)
    .bind(name)
    .bind(slug)
    .bind(fields)
    .bind(settings)
    .fetch_one(pool)
    .await
}

/// Public lookup — used by ingestion, no tenant check.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Endpoint>, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>("SELECT * FROM endpoints WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Dashboard lookup — tenant-scoped via project.
pub async fn find_by_id_scoped(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<Endpoint>, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "SELECT e.* FROM endpoints e
         JOIN projects p ON e.project_id = p.id
         WHERE e.id = $1 AND p.tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_slug(
    pool: &PgPool,
    slug: &str,
    project_id: Uuid,
) -> Result<Option<Endpoint>, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "SELECT * FROM endpoints WHERE slug = $1 AND project_id = $2",
    )
    .bind(slug)
    .bind(project_id)
    .fetch_optional(pool)
    .await
}

/// Find endpoint by slug with tenant scope (for dashboard views).
pub async fn find_by_slug_scoped(
    pool: &PgPool,
    slug: &str,
    tenant_id: Uuid,
) -> Result<Option<Endpoint>, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "SELECT e.* FROM endpoints e
         JOIN projects p ON e.project_id = p.id
         WHERE e.slug = $1 AND p.tenant_id = $2",
    )
    .bind(slug)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
    fields: Option<&serde_json::Value>,
    settings: Option<&serde_json::Value>,
) -> Result<Endpoint, sqlx::Error> {
    sqlx::query_as::<_, Endpoint>(
        "UPDATE endpoints SET name = $3, slug = $4, fields = $5, settings = $6, updated_at = now()
         WHERE id = $1 AND project_id IN (SELECT id FROM projects WHERE tenant_id = $2)
         RETURNING *",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(name)
    .bind(slug)
    .bind(fields)
    .bind(settings)
    .fetch_one(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM endpoints WHERE id = $1 AND project_id IN (SELECT id FROM projects WHERE tenant_id = $2)",
    )
    .bind(id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(())
}
