# 1. Build stage
FROM rust:1.74 AS builder

WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev

# 🔧 Upgrade to latest Cargo that supports lock file v4
RUN rustup install stable && rustup override set stable

RUN cargo build --release

# 2. Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary
COPY --from=builder /app/target/release/wallet_analyzer .

# ✅ Copy required runtime data
COPY config ./config
COPY data ./data
COPY cache ./cache

ENV RUST_LOG=info

CMD ["./wallet_analyzer"]
