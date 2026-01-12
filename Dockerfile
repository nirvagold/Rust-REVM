# ============================================
# Ruster REVM - Multi-Stage Docker Build
# ============================================
# Optimized for minimal image size (~25MB)
# 
# Build: docker build -t ruster-revm .
# Run:   docker run -p 8080:8080 ruster-revm
# Railway: Automatically uses PORT env var

# ============================================
# Stage 1: Build
# ============================================
FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    mkdir -p src/bin && \
    echo "fn main() {}" > src/bin/ruster_api.rs

# Build dependencies only (cached layer)
RUN cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Touch main files to invalidate cache
RUN touch src/main.rs src/bin/ruster_api.rs

# Build the actual binaries
RUN cargo build --release --bin ruster_api

# ============================================
# Stage 2: Runtime
# ============================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false ruster

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/ruster_api /usr/local/bin/ruster_api

# Create telemetry directory
RUN mkdir -p /app/telemetry && chown ruster:ruster /app/telemetry

# Switch to non-root user
USER ruster

# Environment defaults
ENV RUSTER_HOST=0.0.0.0
ENV RUST_LOG=info

# Expose port (Railway will override with PORT env var)
EXPOSE 8080

# Run the API server
CMD ["ruster_api"]
