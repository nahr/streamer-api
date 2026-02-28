# Build UI
FROM node:22-alpine AS ui-builder
WORKDIR /app/ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build

# Build API
FROM rust:1-bookworm AS api-builder
WORKDIR /app/api
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src ./src
RUN cargo build --release

# Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates avahi-daemon avahi-utils && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=api-builder /app/api/target/release/table-tv-api /app/server
COPY --from=ui-builder /app/ui/dist /app/ui-dist

# Avahi config for container (no D-Bus)
RUN echo "[server]\nhost-name=table-tv\nenable-dbus=no\n" > /etc/avahi/avahi-daemon.conf

COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

ENV PORT=80
EXPOSE 80

ENTRYPOINT ["/app/entrypoint.sh"]
