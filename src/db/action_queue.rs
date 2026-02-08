use sqlx::PgPool;
use uuid::Uuid;

use crate::models::action_queue::ActionQueueItem;

pub async fn enqueue(
    pool: &PgPool,
    submission_id: Uuid,
    action_id: Uuid,
) -> Result<ActionQueueItem, sqlx::Error> {
    sqlx::query_as::<_, ActionQueueItem>(
        "INSERT INTO action_queue (submission_id, action_id)
         VALUES ($1, $2) RETURNING *",
    )
    .bind(submission_id)
    .bind(action_id)
    .fetch_one(pool)
    .await
}

/// Atomically claim the next ready item using SELECT FOR UPDATE SKIP LOCKED.
pub async fn claim_next(pool: &PgPool) -> Result<Option<ActionQueueItem>, sqlx::Error> {
    sqlx::query_as::<_, ActionQueueItem>(
        "UPDATE action_queue SET status = 'processing', attempts = attempts + 1
         WHERE id = (
             SELECT id FROM action_queue
             WHERE status IN ('pending', 'failed')
               AND next_retry_at <= now()
             ORDER BY next_retry_at ASC
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         RETURNING *",
    )
    .fetch_optional(pool)
    .await
}

pub async fn mark_completed(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE action_queue SET status = 'completed', completed_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark as failed with exponential backoff. If max attempts reached, stays 'failed' permanently.
pub async fn mark_failed(
    pool: &PgPool,
    id: Uuid,
    attempts: i32,
    max_attempts: i32,
    error: &str,
) -> Result<(), sqlx::Error> {
    if attempts >= max_attempts {
        sqlx::query(
            "UPDATE action_queue SET status = 'failed', last_error = $2, completed_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    } else {
        // Retry with exponential backoff: 2^attempts seconds
        let backoff_secs = 2_i64.pow(attempts as u32);
        sqlx::query(
            "UPDATE action_queue
             SET status = 'failed',
                 last_error = $2,
                 next_retry_at = now() + make_interval(secs => $3::double precision)
             WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .bind(backoff_secs as f64)
        .execute(pool)
        .await?;
    }
    Ok(())
}
