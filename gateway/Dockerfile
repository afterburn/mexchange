FROM rust:1.85-slim-bookworm as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files for dependency caching
COPY matching_engine/Cargo.toml matching_engine/
COPY gateway/Cargo.toml gateway/
COPY udp_proto/Cargo.toml udp_proto/

# Create dummy source files to build dependencies
RUN mkdir -p matching_engine/src gateway/src udp_proto/src && \
    echo "fn main() {}" > gateway/src/main.rs && \
    echo "pub fn dummy() {}" > matching_engine/src/lib.rs && \
    echo "pub fn dummy() {}" > udp_proto/src/lib.rs

# Build dependencies (cached layer)
WORKDIR /app/gateway
RUN cargo build --release --bin gateway || true

# Remove dummy files and copy actual source code
RUN rm -rf ../matching_engine/src ../udp_proto/src src
COPY matching_engine/src/ ../matching_engine/src/
COPY udp_proto/src/ ../udp_proto/src/
COPY gateway/src/ src/

# Clean and rebuild to ensure we get the real binary
RUN cargo clean && cargo build --release --bin gateway

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/gateway/target/release/gateway /app/gateway

EXPOSE 3000

CMD ["/app/gateway"]
