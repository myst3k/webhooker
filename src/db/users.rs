use sqlx::PgPool;
use uuid::Uuid;

use crate::models::User;

pub async fn create<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    tenant_id: Uuid,
    email: &str,
    password_hash: &str,
    name: &str,
    role: &str,
    is_system_admin: bool,
) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (tenant_id, email, password_hash, name, role, is_system_admin)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(tenant_id)
    .bind(email)
    .bind(password_hash)
    .bind(name)
    .bind(role)
    .bind(is_system_admin)
    .fetch_one(executor)
    .await
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn count_all<'e, E: sqlx::PgExecutor<'e>>(executor: E) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(executor)
        .await?;
    Ok(row.0)
}

pub async fn list_all(pool: &PgPool) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn list_by_tenant(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn update_password(
    pool: &PgPool,
    id: Uuid,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(password_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_role(pool: &PgPool, id: Uuid, role: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET role = $2 WHERE id = $1")
        .bind(id)
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
