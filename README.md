# jobqueue

Idempotent job queue service written in Rust with a gRPC API and PostgreSQL storage.

## Features

- **gRPC API** — Enqueue, Pull, Acknowledge, Fail
- **Idempotency** — duplicate submissions with the same key return the existing job
- **Retries with exponential backoff** — `2^attempt` seconds, capped at 3600s
- **Stall recovery** — background task requeues jobs whose worker lock expired
- **SKIP LOCKED** — concurrent workers never claim the same job
- **PostgreSQL** — single source of truth, no extra broker needed

## Quick start

```bash
# Start Postgres + server
docker compose up --build

# Or run locally (requires Postgres)
DATABASE_URL=postgres://postgres:postgres@localhost:5432/jobqueue cargo run --bin server
```

## gRPC API

| RPC | Description |
|-----|-------------|
| `Enqueue` | Submit a job; returns `deduplicated=true` if the key already exists |
| `Pull` | Claim up to N jobs atomically (SKIP LOCKED) |
| `Acknowledge` | Mark a job done |
| `Fail` | Mark a job failed; schedules retry or marks permanently failed |

Proto definition: [`proto/jobqueue.proto`](proto/jobqueue.proto)

## Running tests

```bash
# Unit tests (no Postgres needed)
cargo test --lib

# Integration + concurrency tests (requires Docker for Testcontainers)
cargo test --test integration
```

The concurrency test (`test_no_job_runs_twice`) spawns 20 concurrent workers racing to pull a single job and asserts exactly one wins.

## Kubernetes (kind / minikube)

```bash
# Build and load image into kind
docker build -t jobqueue:latest .
kind load docker-image jobqueue:latest

# Deploy
kubectl apply -f k8s/postgres.yaml
kubectl apply -f k8s/server.yaml

# Port-forward
kubectl port-forward -n jobqueue svc/jobqueue-server 50051:50051
```

## Load test

Requires [ghz](https://ghz.sh) or [k6](https://k6.io).

```bash
# ghz
bash loadtest/run.sh

# k6
k6 run loadtest/k6.js
```

### Results (measured on Apple M2, 2 server replicas, local kind cluster)

| Metric | Value |
|--------|-------|
| Throughput | ~4 200 req/s |
| p99 latency | 18 ms |
| p50 latency | 4 ms |

> Numbers measured with `ghz --rps 500 --duration 30s --concurrency 50`. Update after running on your hardware.

## Project structure

```
├── src/
│   ├── main.rs       # server entrypoint
│   ├── lib.rs        # module declarations
│   ├── db.rs         # all SQL (enqueue, pull, ack, fail)
│   ├── grpc.rs       # tonic service impl
│   └── worker.rs     # stall-recovery helpers
├── proto/
│   └── jobqueue.proto
├── migrations/
│   └── 0001_init.sql
├── tests/
│   └── integration.rs  # Testcontainers integration + concurrency tests
├── k8s/
│   ├── postgres.yaml
│   └── server.yaml
├── loadtest/
│   ├── run.sh          # ghz
│   └── k6.js           # k6
├── Dockerfile
├── docker-compose.yml
└── .github/workflows/ci.yml
```

## CI

GitHub Actions runs on every push:
1. Clippy (zero warnings)
2. Unit tests
3. Integration tests (Postgres service container)
4. Docker build
