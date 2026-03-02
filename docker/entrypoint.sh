#!/bin/sh
set -e

# If .env is empty (e.g. host file didn't exist and Docker created empty mount), use .env.example
if [ ! -s /app/.env ] && [ -f /app/.env.example ]; then
    cp /app/.env.example /app/.env
fi

# Start Avahi for mDNS (table-tv.local) - may fail in bridge network, that's ok
avahi-daemon --no-drop-root --no-rlimits 2>/dev/null || true &

# Start stunnel for Facebook RTMPS when USE_STUNNEL_FOR_RTMPS=1 (avoids FFmpeg RTMPS I/O errors)
if [ "$USE_STUNNEL_FOR_RTMPS" = "1" ]; then
  stunnel /app/stunnel-fb.conf &
fi

# API on 8080 (internal only - nginx proxies /api to it)
cd /app && /app/server &
# Wait for API to be ready (max 15s)
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
  if curl -sf http://127.0.0.1:8080/api/info >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

# Nginx on 80: serves UI, proxies /api to API
exec nginx -g 'daemon off;'
