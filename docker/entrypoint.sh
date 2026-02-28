#!/bin/sh
set -e

# Start Avahi for mDNS (table-tv.local) - may fail in bridge network, that's ok
avahi-daemon --no-drop-root --no-rlimits 2>/dev/null || true &

# Run the server (foreground)
exec /app/server
