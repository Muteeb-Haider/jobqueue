use sqlx::PgPool;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::db;
use crate::proto::{
    job_queue_server::JobQueue, AckRequest, AckResponse, EnqueueRequest, EnqueueResponse,
    FailRequest, FailResponse, Job, PullRequest, PullResponse,
};

pub struct JobQueueService {
    pub pool: PgPool,
}

#[tonic::async_trait]
impl JobQueue for JobQueueService {
    async fn enqueue(
        &self,
        req: Request<EnqueueRequest>,
    ) -> Result<Response<EnqueueResponse>, Status> {
        let r = req.into_inner();
        let max_attempts = if r.max_attempts <= 0 { 3 } else { r.max_attempts };
        let (job_id, deduplicated) =
            db::enqueue(&self.pool, &r.idempotency_key, &r.queue, &r.payload, max_attempts)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(EnqueueResponse {
            job_id: job_id.to_string(),
            deduplicated,
        }))
    }

    async fn pull(&self, req: Request<PullRequest>) -> Result<Response<PullResponse>, Status> {
        let r = req.into_inner();
        let count = if r.count <= 0 { 1 } else { r.count };
        let jobs = db::pull(&self.pool, &r.queue, count)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let proto_jobs = jobs
            .into_iter()
            .map(|j| Job {
                job_id: j.id.to_string(),
                queue: j.queue,
                payload: j.payload,
                attempt: j.attempt,
            })
            .collect();
        Ok(Response::new(PullResponse { jobs: proto_jobs }))
    }

    async fn acknowledge(
        &self,
        req: Request<AckRequest>,
    ) -> Result<Response<AckResponse>, Status> {
        let r = req.into_inner();
        let id = Uuid::parse_str(&r.job_id).map_err(|_| Status::invalid_argument("bad job_id"))?;
        db::acknowledge(&self.pool, id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(AckResponse {}))
    }

    async fn fail(&self, req: Request<FailRequest>) -> Result<Response<FailResponse>, Status> {
        let r = req.into_inner();
        let id = Uuid::parse_str(&r.job_id).map_err(|_| Status::invalid_argument("bad job_id"))?;
        db::fail(&self.pool, id, &r.error)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(FailResponse {}))
    }
}
