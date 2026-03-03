#!/bin/sh
set -e

# Config is loaded by table-tv from /etc/table-tv/table-tv.config

# Start stunnel for Facebook RTMPS when config has use_stunnel_for_rtmps = true
if [ -f /etc/table-tv/table-tv.config ] && grep -qE 'use_stunnel_for_rtmps\s*=\s*true' /etc/table-tv/table-tv.config; then
    export USE_STUNNEL_FOR_RTMPS=1
fi

# Start MediaMTX (RTSP, Control API, Playback) - must run before table-tv
if [ -x /usr/lib/table-tv/mediamtx ]; then
    /usr/lib/table-tv/mediamtx /etc/table-tv/mediamtx.yml &
fi

# Start Avahi for mDNS (table-tv.local)
avahi-daemon --no-drop-root --no-rlimits 2>/dev/null || true &

# Start stunnel for Facebook RTMPS when USE_STUNNEL_FOR_RTMPS=1
if [ "$USE_STUNNEL_FOR_RTMPS" = "1" ]; then
    stunnel /etc/table-tv/stunnel-fb.conf &
fi

# API on 80: serves UI and /api (ui_dist_path from config)
exec /usr/bin/table-tv
