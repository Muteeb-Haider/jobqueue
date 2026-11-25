/// Integration + concurrency tests using Testcontainers (real Postgres).
/// Run with: cargo test --test integration
use jobqueue::db;
use testcontainers::clients::Cli;
use testcontainers_modules::postgres::Postgres;

fn docker() -> Cli {
    Cli::default()
}

async fn make_pool(url: &str) -> sqlx::PgPool {
    db::connect(url).await.expect("connect")
}

fn pg_url(port: u16) -> String {
    format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres")
}

#[tokio::test]
async fn test_enqueue_and_pull() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    let (id, dedup) = db::enqueue(&pool, "key-1", "default", b"hello", 3)
        .await
        .unwrap();
    assert!(!dedup);

    let jobs = db::pull(&pool, "default", 1).await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, id);
    assert_eq!(jobs[0].attempt, 1);
}

#[tokio::test]
async fn test_idempotency_deduplication() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    let (id1, dedup1) = db::enqueue(&pool, "same-key", "q", b"a", 3).await.unwrap();
    let (id2, dedup2) = db::enqueue(&pool, "same-key", "q", b"b", 3).await.unwrap();

    assert!(!dedup1);
    assert!(dedup2);
    assert_eq!(id1, id2, "duplicate must return same job id");

    let jobs = db::pull(&pool, "q", 10).await.unwrap();
    assert_eq!(jobs.len(), 1);
}

#[tokio::test]
async fn test_acknowledge_marks_done() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    let (id, _) = db::enqueue(&pool, "ack-key", "q", b"x", 3).await.unwrap();
    db::pull(&pool, "q", 1).await.unwrap();
    db::acknowledge(&pool, id).await.unwrap();

    let jobs = db::pull(&pool, "q", 1).await.unwrap();
    assert!(jobs.is_empty());
}

#[tokio::test]
async fn test_fail_schedules_retry() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    let (id, _) = db::enqueue(&pool, "fail-key", "q", b"x", 3).await.unwrap();
    db::pull(&pool, "q", 1).await.unwrap();
    db::fail(&pool, id, "boom").await.unwrap();

    let status: String = sqlx::query_scalar_unchecked!(
        "SELECT status FROM jobs WHERE id = $1",
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "retrying");
}

#[tokio::test]
async fn test_fail_exhausted_marks_failed() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    let (id, _) = db::enqueue(&pool, "exhaust-key", "q", b"x", 1).await.unwrap();
    db::pull(&pool, "q", 1).await.unwrap();
    db::fail(&pool, id, "boom").await.unwrap();

    let status: String = sqlx::query_scalar_unchecked!(
        "SELECT status FROM jobs WHERE id = $1",
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
}

/// Concurrency test: 20 workers race to pull the same single job.
/// Exactly one must win — proves SKIP LOCKED prevents double execution.
#[tokio::test]
async fn test_no_job_runs_twice() {
    let docker = docker();
    let node = docker.run(Postgres::default());
    let pool = make_pool(&pg_url(node.get_host_port_ipv4(5432))).await;

    db::enqueue(&pool, "once-key", "q", b"x", 3).await.unwrap();

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let p = pool.clone();
            tokio::spawn(async move { db::pull(&p, "q", 1).await.unwrap() })
        })
        .collect();

    let total_claimed: usize = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap().len())
        .sum();

    assert_eq!(total_claimed, 1, "exactly one worker must claim the job");
}

