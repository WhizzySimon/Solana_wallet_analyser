# Dockerfile

# 1. Build stage
FROM rust:1.72 as builder

WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev
RUN cargo build --release

# 2. Runtime stage
FROM debian:buster-slim
RUN apt-get update && apt-get install -y ca-certificates libssl1.1 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/wallet-analyzer /app/

# Make sure .env can be passed by fly
ENV RUST_LOG=info

CMD ["./wallet-analyzer"]
