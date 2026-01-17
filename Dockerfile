# Multi-stage build for minimal image size
FROM rust:1-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev

# Set working directory
WORKDIR /app

# Copy only source files first (faster cache)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build dependencies first (cached layer)
RUN mkdir -p target/release && \
    echo 'fn main() {}' > dummy.rs && \
    rustc --edition 2024 --crate-type lib dummy.rs -o target/release/libdummy.rlib || true

# Copy source and build
COPY . .

# Build all binaries: REPL, TCP server, and backend-service (REST API)
RUN cargo build --release -p db --features tcp-server
RUN cargo build --release -p backend_service

# Runtime stage
FROM alpine:3.19 AS runtime

# Install runtime dependencies
RUN apk add --no-cache openssl ca-certificates netcat-openbsd bash

# Create non-root user
RUN addgroup -g 1000 app && \
    adduser -u 1000 -G app -s /bin/sh -D app

# Create data directory (parent of database file)
RUN mkdir -p /data && chown -R app:app /data

# Copy binaries from builder
COPY --from=builder /app/target/release/rdbmsd /usr/local/bin/
COPY --from=builder /app/target/release/rdbms /usr/local/bin/
COPY --from=builder /app/target/release/backend-service /usr/local/bin/

# Switch to non-root user
USER app

# Expose default ports
# 5432 - TCP server (rdbmsd)
# 8080 - REST API (backend-service)
EXPOSE 5432 8080

# Health check based on SERVICE mode
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD sh -c 'if [ "$SERVICE" = "api" ]; then curl -f http://localhost:8080/api/health || exit 1; else nc -z localhost 5432 || exit 1; fi'

# Entrypoint handles both server modes
COPY --chmod=755 <<'EOF' /entrypoint.sh
#!/bin/sh

# Default configuration
DB_PATH="${DB_PATH:-/data/database.db}"
LISTEN="${LISTEN:-0.0.0.0:5432}"
SERVICE="${SERVICE:-tcp}"

# Ensure data directory exists
mkdir -p "$(dirname "$DB_PATH")"

case "$SERVICE" in
    tcp)
        echo "Starting RDBMS TCP server on $LISTEN..."
        exec rdbmsd --db "$DB_PATH" --listen "$LISTEN"
        ;;
    api)
        echo "Starting RDBMS REST API on port 8080..."
        exec backend-service --db "$DB_PATH" --port 8080
        ;;
    *)
        echo "Unknown service mode: $SERVICE"
        echo "Use SERVICE=tcp or SERVICE=api"
        exit 1
        ;;
esac
EOF

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--db", "/data/database.db", "--listen", "0.0.0.0:5432"]
