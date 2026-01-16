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

# Build both binaries
RUN cargo build --release -p db --features tcp-server

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

# Switch to non-root user
USER app

# Expose default port
EXPOSE 5432

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD nc -z localhost 5432 || exit 1

# Use entrypoint script to handle database initialization
COPY --chmod=755 <<'EOF' /entrypoint.sh
#!/bin/sh
DB_PATH="/data/mydb"
if [ ! -f "$DB_PATH" ]; then
    mkdir -p "$(dirname "$DB_PATH")"
fi
exec rdbmsd --db "$DB_PATH" --listen "0.0.0.0:5432"
EOF

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--db", "/data/mydb", "--listen", "0.0.0.0:5432"]
