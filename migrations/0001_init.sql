-- Jobs table
CREATE TABLE jobs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    idempotency_key TEXT NOT NULL,
    queue       TEXT NOT NULL,
    payload     BYTEA NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending | running | done | failed
    attempt     INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    last_error  TEXT,
    run_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_until TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX jobs_idempotency_queue ON jobs (idempotency_key, queue);
CREATE INDEX jobs_queue_status_run_at ON jobs (queue, status, run_at);
