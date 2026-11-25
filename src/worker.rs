/// Requeue jobs whose lock expired but are still marked 'running'.
/// Call this periodically (e.g. every 10s) to recover stalled workers.
use anyhow::Result;
use sqlx::PgPool;

pub async fn requeue_stalled(pool: &PgPool) -> Result<u64> {
    let rows = sqlx::query!(
        r#"
        UPDATE jobs SET
            status = 'retrying',
            last_error = 'worker timeout',
            run_at = NOW(),
            updated_at = NOW()
        WHERE status = 'running'
          AND locked_until < NOW()
          AND attempt < max_attempts
        "#
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows)
}

pub async fn fail_stalled(pool: &PgPool) -> Result<u64> {
    let rows = sqlx::query!(
        r#"
        UPDATE jobs SET
            status = 'failed',
            last_error = 'worker timeout - max attempts reached',
            updated_at = NOW()
        WHERE status = 'running'
          AND locked_until < NOW()
          AND attempt >= max_attempts
        "#
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(rows)
}
