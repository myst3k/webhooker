use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Project;

pub async fn list(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> Result<Project, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "INSERT INTO projects (tenant_id, name, slug) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_slug(
    pool: &PgPool,
    slug: &str,
    tenant_id: Uuid,
) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE slug = $1 AND tenant_id = $2",
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
) -> Result<Project, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        "UPDATE projects SET name = $3, slug = $4, updated_at = now()
         WHERE id = $1 AND tenant_id = $2 RETURNING *",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
}

/// Unscoped lookup — used by ingestion pipeline.
pub async fn find_by_id_unscoped(pool: &PgPool, id: Uuid) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn delete(pool: &PgPool, id: Uuid, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM projects WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(())
}
