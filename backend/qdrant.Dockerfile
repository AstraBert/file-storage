# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.91-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY qdrant-worker/  qdrant-worker/
COPY observability/ observability/
COPY app/  app/
COPY proto/ proto/
COPY proto-definitions/ proto-definitions/
COPY utils/ utils/

RUN cargo build --release --bin server-worker

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/server-worker /usr/local/bin/server-worker

EXPOSE 8000

ENV RUST_LOG=warn,grpc_server=debug,proto=debug
CMD ["server-worker"]
