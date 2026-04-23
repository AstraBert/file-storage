# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.91-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY app/  app/
COPY proto/ proto/
COPY observability/ observability/
COPY proto-definitions/ proto-definitions/

RUN cargo build --release --bin grpc-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/grpc-server /usr/local/bin/grpc-server

EXPOSE 50051

ENV RUST_LOG=warn,grpc_server=debug,proto=debug
CMD ["grpc-server"]
