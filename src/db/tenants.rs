use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Tenant;

pub async fn create(pool: &PgPool, name: &str, slug: &str) -> Result<Tenant, sqlx::Error> {
    sqlx::query_as::<_, Tenant>(
        "INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING *",
    )
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

pub async fn list(pool: &PgPool) -> Result<Vec<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT * FROM tenants ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    slug: &str,
) -> Result<Tenant, sqlx::Error> {
    sqlx::query_as::<_, Tenant>(
        "UPDATE tenants SET name = $2, slug = $3, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
