# ---- build ----
FROM rust:1.78-slim AS builder

RUN apt-get update && apt-get install -y protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY proto ./proto
COPY migrations ./migrations
COPY src ./src

# sqlx_migrate! embeds migrations at compile time; SQLX_OFFLINE skips live-DB query checks
ENV SQLX_OFFLINE=true
ENV DATABASE_URL=postgres://postgres:postgres@localhost:5432/jobqueue

RUN cargo build --release --bin server

# ---- runtime ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/server /usr/local/bin/server

ENV RUST_LOG=info
EXPOSE 50051

ENTRYPOINT ["/usr/local/bin/server"]
