use sqlx::PgPool;
use uuid::Uuid;

/// Log an audit event. This is called explicitly in handlers after mutations.
pub async fn log_event(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    details: Option<serde_json::Value>,
) {
    if let Err(e) = crate::db::audit::log_event(
        pool,
        tenant_id,
        user_id,
        action,
        resource_type,
        resource_id,
        details,
    )
    .await
    {
        tracing::error!("Failed to log audit event: {e}");
    }
}
