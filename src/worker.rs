use anyhow::Result;
use sqlx::PgPool;

/// Requeue jobs whose lock expired but still have attempts remaining.
pub async fn requeue_stalled(pool: &PgPool) -> Result<u64> {
    let rows = sqlx::query_unchecked!(
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

/// Permanently fail stalled jobs that have exhausted all attempts.
pub async fn fail_stalled(pool: &PgPool) -> Result<u64> {
    let rows = sqlx::query_unchecked!(
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
