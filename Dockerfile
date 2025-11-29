# GraphLite Dockerfile
# Supports: linux/amd64, linux/arm64
# Base: Ubuntu 22.04 LTS

ARG RUST_VERSION=1.90

# ==============================================================================
# Stage 1: Builder - Compile GraphLite
# ==============================================================================
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo

# Copy all workspace members
COPY graphlite ./graphlite
COPY graphlite-cli ./graphlite-cli
COPY graphlite-sdk ./graphlite-sdk
COPY graphlite-ffi ./graphlite-ffi

# Build for native architecture
RUN cargo build --release --bin graphlite

# Copy binary to known location
RUN cp target/release/graphlite /build/graphlite-bin && \
    ls -lh /build/graphlite-bin

# ==============================================================================
# Stage 2: Runtime - Minimal Ubuntu image
# ==============================================================================
FROM ubuntu:22.04 AS runtime

# Set labels for metadata
LABEL org.opencontainers.image.title="GraphLite"
LABEL org.opencontainers.image.description="A lightweight ISO GQL Graph Database"
LABEL org.opencontainers.image.version="0.0.1"
LABEL org.opencontainers.image.vendor="GraphLite Contributors"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.source="https://github.com/GraphLite-AI/GraphLite"

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd -r graphlite && \
    useradd -r -g graphlite -s /bin/bash -m graphlite

# Create directories
RUN mkdir -p /data /config && \
    chown -R graphlite:graphlite /data /config

# Copy binary from builder
COPY --from=builder /build/graphlite-bin /usr/local/bin/graphlite
RUN chmod +x /usr/local/bin/graphlite

# Switch to non-root user
USER graphlite
WORKDIR /home/graphlite

# Set environment variables
ENV RUST_LOG=info
ENV GRAPHLITE_DATA_PATH=/data
ENV GRAPHLITE_CONFIG_PATH=/config
ENV GRAPHLITE_DB_PATH=/data/default_db
ENV GRAPHLITE_USER=admin
ENV GRAPHLITE_PASSWORD=

# Copy entrypoint script for flexible startup
COPY --chown=graphlite:graphlite entrypoint.sh /home/graphlite/entrypoint.sh
RUN chmod +x /home/graphlite/entrypoint.sh

ENTRYPOINT ["/home/graphlite/entrypoint.sh"]
CMD []

# ==============================================================================
# Usage Examples:
# ==============================================================================
# Build for native architecture:
#   docker build -t graphlite:latest .
#
# Initialize new database:
#   docker run -it -v $(pwd)/mydb:/data graphlite:latest \
#     graphlite install --path /data/mydb --admin-user admin --admin-password secret
#
# Start interactive GQL shell (automatic with environment variables):
#   docker run -it -v $(pwd)/mydb:/data \
#     -e GRAPHLITE_DB_PATH=/data/mydb \
#     -e GRAPHLITE_USER=admin \
#     -e GRAPHLITE_PASSWORD=secret \
#     graphlite:latest
#
# Start GQL shell (manual command):
#   docker run -it -v $(pwd)/mydb:/data graphlite:latest \
#     graphlite gql --path /data/mydb -u admin -p secret
#
# Run any other command:
#   docker run -it graphlite:latest graphlite --version
# ==============================================================================
