FROM rust:1.85-slim-bookworm AS builder

WORKDIR /app

# Install dependencies for building
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY accounts/Cargo.toml ./Cargo.toml

# Create dummy main.rs and lib.rs to build dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs

# Build dependencies only
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY accounts/src ./src
COPY accounts/migrations ./migrations

# Build actual binary (touch to invalidate cache)
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/accounts /app/accounts

EXPOSE 3001

CMD ["./accounts"]
