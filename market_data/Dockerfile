FROM rust:1.83 as builder

WORKDIR /app

# Install build dependencies for rdkafka
RUN apt-get update && apt-get install -y \
    cmake \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files for dependency caching
COPY market_data/Cargo.toml market_data/

# Create dummy source files to build dependencies
RUN mkdir -p market_data/src && \
    echo "fn main() {}" > market_data/src/main.rs

# Build dependencies (cached layer)
WORKDIR /app/market_data
RUN cargo build --release --bin market_data || true

# Remove dummy files and copy actual source code
RUN rm -rf src
COPY market_data/src/ src/

# Clean and rebuild to ensure we get the real binary
RUN cargo clean && cargo build --release --bin market_data

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    librdkafka1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/market_data/target/release/market_data /app/market_data

EXPOSE 3002

CMD ["/app/market_data"]
