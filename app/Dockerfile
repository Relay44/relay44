# Build stage
FROM rust:1.88-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies — only re-downloaded/compiled when Cargo.toml/Cargo.lock change
COPY Cargo.toml Cargo.lock ./
COPY app/Cargo.toml ./app/Cargo.toml
COPY cli/Cargo.toml ./cli/Cargo.toml
COPY programs/polyguard-orderbook/Cargo.toml ./programs/polyguard-orderbook/Cargo.toml

# Create stub source files so cargo can resolve the workspace and build deps
RUN mkdir -p app/src cli/src programs/polyguard-orderbook/src \
    && echo "fn main() {}" > app/src/main.rs \
    && echo "fn main() {}" > cli/src/main.rs \
    && touch programs/polyguard-orderbook/src/lib.rs

ENV CARGO_BUILD_JOBS=2
RUN cargo build --profile ci --package relay44-backend 2>/dev/null || true
RUN rm -rf app/src cli/src programs/polyguard-orderbook/src

# Copy real source and build
COPY app ./app
COPY cli ./cli
COPY programs ./programs

RUN cargo build --profile ci --package relay44-backend

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/ci/relay44-api /app/relay44-api
COPY migrations ./migrations

RUN useradd -r -s /bin/false relay44
USER relay44

EXPOSE 10000

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD sh -c 'curl -f "http://localhost:${PORT:-10000}/health" || exit 1'

CMD ["/app/relay44-api"]
