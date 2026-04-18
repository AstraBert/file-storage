# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.91-slim AS builder

# System deps needed by some crates (openssl, protobuf, etc.)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Copy workspace-level Cargo files first for layer caching
COPY Cargo.toml Cargo.lock ./

# Copy both workspace members
COPY app/  app/
COPY proto/ proto/
COPY proto-definitions/ proto-definitions/

# Build only the rest-server binary in release mode
RUN cargo build --release --bin rest-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/rest-server /usr/local/bin/rest-server

EXPOSE 4444

CMD ["rest-server"]
