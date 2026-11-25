use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub idempotency_key: String,
    pub queue: String,
    pub payload: Vec<u8>,
    pub status: String,
    pub attempt: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub run_at: DateTime<Utc>,
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Insert a job, returning (id, deduplicated).
/// Uses INSERT ... ON CONFLICT DO NOTHING so duplicate idempotency keys are ignored.
pub async fn enqueue(
    pool: &PgPool,
    idempotency_key: &str,
    queue: &str,
    payload: &[u8],
    max_attempts: i32,
) -> Result<(Uuid, bool)> {
    let id = Uuid::new_v4();
    let affected = sqlx::query!(
        r#"
        INSERT INTO jobs (id, idempotency_key, queue, payload, max_attempts)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (idempotency_key, queue) DO NOTHING
        "#,
        id,
        idempotency_key,
        queue,
        payload,
        max_attempts,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        // Return the existing job id
        let existing = sqlx::query_scalar!(
            "SELECT id FROM jobs WHERE idempotency_key = $1 AND queue = $2",
            idempotency_key,
            queue
        )
        .fetch_one(pool)
        .await?;
        Ok((existing, true))
    } else {
        Ok((id, false))
    }
}

/// Atomically claim up to `count` pending/retryable jobs using SKIP LOCKED.
pub async fn pull(pool: &PgPool, queue: &str, count: i32) -> Result<Vec<Job>> {
    let lock_secs = 30i64;
    let jobs = sqlx::query_as!(
        Job,
        r#"
        UPDATE jobs SET
            status = 'running',
            attempt = attempt + 1,
            locked_until = NOW() + ($3 * INTERVAL '1 second'),
            updated_at = NOW()
        WHERE id IN (
            SELECT id FROM jobs
            WHERE queue = $1
              AND status IN ('pending', 'retrying')
              AND run_at <= NOW()
            ORDER BY run_at
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        RETURNING *
        "#,
        queue,
        count as i64,
        lock_secs as f64,
    )
    .fetch_all(pool)
    .await?;
    Ok(jobs)
}

pub async fn acknowledge(pool: &PgPool, job_id: Uuid) -> Result<()> {
    sqlx::query!(
        "UPDATE jobs SET status = 'done', updated_at = NOW() WHERE id = $1",
        job_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a job failed; schedule retry with exponential backoff if attempts remain.
pub async fn fail(pool: &PgPool, job_id: Uuid, error: &str) -> Result<()> {
    let job = sqlx::query!(
        "SELECT attempt, max_attempts FROM jobs WHERE id = $1",
        job_id
    )
    .fetch_one(pool)
    .await?;

    if job.attempt >= job.max_attempts {
        sqlx::query!(
            "UPDATE jobs SET status = 'failed', last_error = $2, updated_at = NOW() WHERE id = $1",
            job_id,
            error
        )
        .execute(pool)
        .await?;
    } else {
        // backoff: 2^attempt seconds (capped at 3600)
        let backoff = (2i64.pow(job.attempt as u32)).min(3600);
        sqlx::query!(
            r#"
            UPDATE jobs SET
                status = 'retrying',
                last_error = $2,
                run_at = NOW() + ($3 * INTERVAL '1 second'),
                updated_at = NOW()
            WHERE id = $1
            "#,
            job_id,
            error,
            backoff as f64,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}
