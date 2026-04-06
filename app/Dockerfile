# syntax=docker/dockerfile:1
# Build stage
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY app ./app
COPY cli ./cli
COPY programs ./programs

# Build release binary with BuildKit cache mounts for cargo registry + target dir.
# This survives even when Docker layer caching is disabled.
ENV CARGO_BUILD_JOBS=2
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --package relay44-backend \
    && cp /app/target/release/relay44-api /usr/local/bin/relay44-api

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/local/bin/relay44-api /app/relay44-api

# Copy migrations
COPY migrations ./migrations

# Create non-root user
RUN useradd -r -s /bin/false relay44
USER relay44

# Expose port
EXPOSE 10000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD sh -c 'curl -f "http://localhost:${PORT:-10000}/health" || exit 1'

# Run
CMD ["/app/relay44-api"]
