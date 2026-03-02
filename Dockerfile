# =============================================================================
# UI build - only rebuilds when ui/ changes
# npm ci cached unless package*.json changes
# =============================================================================
FROM node:22-alpine AS ui-builder
WORKDIR /app/ui

# Dependencies: only rebuild when package files change
COPY ui/package.json ui/package-lock.json ./
RUN npm ci

# Source: rebuild when any ui source changes
COPY ui/vite.config.js ./
COPY ui/index.html ./
COPY ui/src ./src
# .env at project root - Vite reads it for AUTH0_* at build time
COPY .env* ../
RUN npm run build

# =============================================================================
# API build - only rebuilds when api/ changes
# Dependencies: only rebuild when Cargo.toml or Cargo.lock changes
# =============================================================================
FROM rust:1-bookworm AS api-deps
RUN apt-get update && apt-get install -y libclang-dev clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/api
COPY api/Cargo.toml api/Cargo.lock ./

# Build dependencies only (dummy main so we don't need src yet)
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# =============================================================================
# API app - rebuilds when api/src or api/assets changes
# =============================================================================
FROM api-deps AS api-builder
COPY api/src ./src
COPY api/assets ./assets
RUN touch ./src/main.rs
RUN cargo build --release

# =============================================================================
# Runtime - combines UI + API
# Build targets: ui-builder | api-deps | api-builder | table-tv (default)
# =============================================================================
FROM debian:bookworm-slim AS table-tv
RUN apt-get update && apt-get install -y ca-certificates p11-kit avahi-daemon avahi-utils dumb-init ffmpeg nginx curl fonts-dejavu-core stunnel \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=api-builder /app/api/target/release/table-tv-api /app/server
COPY --from=ui-builder /app/ui/dist /app/ui-dist

# Copy .env if it exists (use .env.example as fallback)
COPY .env* ./
RUN if [ ! -f .env ] && [ -f .env.example ]; then cp .env.example .env; fi

# Create data directory for persistent db storage
RUN mkdir -p /app/data

# Avahi config for container (no D-Bus)
RUN echo "[server]\nhost-name=table-tv\nenable-dbus=no\n" > /etc/avahi/avahi-daemon.conf

COPY docker/entrypoint.sh /app/entrypoint.sh
COPY docker/nginx.conf /etc/nginx/sites-enabled/default
COPY stunnel-fb.conf /app/stunnel-fb.conf
RUN chmod +x /app/entrypoint.sh

# API listens on 8080 (internal only). Nginx serves UI on 80 and proxies /api to 8080.
ENV PORT=8080
ENV SQLITE_PATH=/app/data/table-tv.db
ENV RUST_LOG=info,tower_http=debug
EXPOSE 80
# 8080 is internal only - not published

ENTRYPOINT ["/usr/bin/dumb-init", "--", "/app/entrypoint.sh"]
