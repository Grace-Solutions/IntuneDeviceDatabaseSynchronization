# Build stage
FROM rust:1.75-slim AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    libpq-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies and create app user
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false appuser

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/MSGraphDBSynchronizer /usr/local/bin/msgraph-db-sync

# Create data directory for application files
RUN mkdir -p /app/data/logs /app/data/backups && \
    chown -R appuser:appuser /app

# Copy configuration template and startup script
COPY config.json /app/config.json.template
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Make startup script executable
RUN chmod +x /app/docker-entrypoint.sh

# Create volume for persistent data
VOLUME ["/app/data"]

# Switch to app user
USER appuser

# Expose Prometheus metrics port
EXPOSE 9898

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9898/metrics || exit 1

# Use startup script
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["run"]
