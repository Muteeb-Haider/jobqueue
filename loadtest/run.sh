#!/usr/bin/env bash
# Run load test with ghz against the Enqueue RPC.
# Prerequisites: ghz installed (https://ghz.sh), server running on localhost:50051
set -euo pipefail

PROTO="$(dirname "$0")/../proto/jobqueue.proto"
ADDR="localhost:50051"
DURATION="30s"
CONCURRENCY=50
RPS=500

echo "=== Enqueue load test: ${RPS} RPS for ${DURATION} with ${CONCURRENCY} workers ==="

ghz \
  --proto "$PROTO" \
  --call jobqueue.JobQueue.Enqueue \
  --data '{"idempotency_key":"{{.RequestNumber}}","queue":"load","payload":"aGVsbG8=","max_attempts":3}' \
  --rps "$RPS" \
  --duration "$DURATION" \
  --concurrency "$CONCURRENCY" \
  --format summary \
  "$ADDR"
