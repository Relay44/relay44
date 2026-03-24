# Relay44 Backend API
# Multi-stage build for minimal production image

# Build stage
FROM rust:1.88-bookworm AS builder

WORKDIR /usr/src/relay44/app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest first for dependency caching
COPY app/Cargo.toml ./Cargo.toml

# Create dummy source for dependency compilation
RUN mkdir -p src && \
    echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source
COPY app/src ./src

# Build release binary
RUN touch src/main.rs && cargo build --release

# Build sqlx-cli for migrations
RUN cargo install sqlx-cli --no-default-features --features postgres

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd --create-home --shell /bin/bash relay44
USER relay44
WORKDIR /home/relay44

# Copy binary and sqlx-cli from builder
COPY --from=builder /usr/src/relay44/app/target/release/relay44-api ./
COPY --from=builder /usr/local/cargo/bin/sqlx ./

# Copy migrations
COPY migrations ./migrations

# Runtime configuration
ENV RUST_LOG=info
ENV HOST=0.0.0.0
ENV PORT=8080

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["./relay44-api"]
