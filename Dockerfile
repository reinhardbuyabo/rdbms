# Multi-stage build for minimal image size
FROM rust:1-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the server binary with release profile
RUN cargo build --release --bin rdbmsd -p db

# Runtime stage
FROM alpine:3.19 AS runtime

# Install runtime dependencies
RUN apk add --no-cache openssl ca-certificates

# Create non-root user
RUN addgroup -g 1000 app && \
    adduser -u 1000 -G app -s /bin/sh -D app

# Create data directory
RUN mkdir -p /data && chown -R app:app /data

# Copy binary from builder
COPY --from=builder /app/target/release/rdbmsd /usr/local/bin/

# Switch to non-root user
USER app

# Expose default port
EXPOSE 5432

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD nc -z localhost 5432 || exit 1

# Set environment variables
ENV RDBMS_DATA_DIR=/data
ENV RDBMS_LISTEN=0.0.0.0:5432

# Default command
ENTRYPOINT ["rdbmsd"]
CMD ["--db", "/data", "--listen", "0.0.0.0:5432"]
