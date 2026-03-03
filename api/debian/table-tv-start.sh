#!/bin/sh
set -e

# Config is loaded by table-tv-api from /etc/table-tv/config.toml

# Start stunnel for Facebook RTMPS when config has use_stunnel_for_rtmps = true
if [ -f /etc/table-tv/config.toml ] && grep -qE 'use_stunnel_for_rtmps\s*=\s*true' /etc/table-tv/config.toml; then
    export USE_STUNNEL_FOR_RTMPS=1
fi

# Start MediaMTX (RTSP, Control API, Playback) - must run before table-tv-api
if [ -x /usr/lib/table-tv/mediamtx ]; then
    /usr/lib/table-tv/mediamtx /etc/table-tv/mediamtx.yml &
fi

# Start Avahi for mDNS (table-tv.local)
avahi-daemon --no-drop-root --no-rlimits 2>/dev/null || true &

# Start stunnel for Facebook RTMPS when USE_STUNNEL_FOR_RTMPS=1
if [ "$USE_STUNNEL_FOR_RTMPS" = "1" ]; then
    stunnel /etc/table-tv/stunnel-fb.conf &
fi

# API on 8080 (internal only - nginx proxies /api to it)
/usr/bin/table-tv-api &
# Wait for API to be ready (max 15s)
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
    if curl -sf http://127.0.0.1:8080/api/info >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

# Nginx on 80: serves UI, proxies /api to API
exec nginx -c /etc/table-tv/nginx.conf -g 'daemon off;'
