use jobqueue::{
    db,
    grpc::JobQueueService,
    proto::job_queue_server::JobQueueServer,
    worker,
};
use std::net::SocketAddr;
use tokio::time::{interval, Duration};
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/jobqueue".into());
    let addr: SocketAddr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".into())
        .parse()?;

    let pool = db::connect(&database_url).await?;
    info!("Connected to database, listening on {addr}");

    // Background task: requeue stalled jobs every 10s
    let pool_bg = pool.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            let _ = worker::requeue_stalled(&pool_bg).await;
            let _ = worker::fail_stalled(&pool_bg).await;
        }
    });

    let svc = JobQueueService { pool };
    Server::builder()
        .add_service(JobQueueServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}
