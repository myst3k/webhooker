use sqlx::PgPool;
use uuid::Uuid;

use crate::models::TenantSmtpConfig;

pub async fn find_by_tenant(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Option<TenantSmtpConfig>, sqlx::Error> {
    sqlx::query_as::<_, TenantSmtpConfig>(
        "SELECT * FROM tenant_smtp_configs WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn upsert(
    pool: &PgPool,
    tenant_id: Uuid,
    host: &str,
    port: i32,
    username_enc: &[u8],
    password_enc: &[u8],
    from_address: &str,
    from_name: Option<&str>,
    tls_mode: &str,
) -> Result<TenantSmtpConfig, sqlx::Error> {
    sqlx::query_as::<_, TenantSmtpConfig>(
        "INSERT INTO tenant_smtp_configs (tenant_id, host, port, username_enc, password_enc, from_address, from_name, tls_mode)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (tenant_id) DO UPDATE SET
           host = EXCLUDED.host,
           port = EXCLUDED.port,
           username_enc = EXCLUDED.username_enc,
           password_enc = EXCLUDED.password_enc,
           from_address = EXCLUDED.from_address,
           from_name = EXCLUDED.from_name,
           tls_mode = EXCLUDED.tls_mode,
           updated_at = now()
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(host)
    .bind(port)
    .bind(username_enc)
    .bind(password_enc)
    .bind(from_address)
    .bind(from_name)
    .bind(tls_mode)
    .fetch_one(pool)
    .await
}

pub async fn delete(pool: &PgPool, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tenant_smtp_configs WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(())
}
