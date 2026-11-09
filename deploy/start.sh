#!/bin/sh
# Start the world in its container (Fly.io, or Docker on any machine). A world
# dropped in as /data/import.db (see docs/hosting-fly.md) takes over here, before
# the server opens anything, so it's never swapped under a running world.
set -e
if [ -f /data/import.db ]; then
    if [ -f "$WORLD_DB" ]; then
        mkdir -p /data/replaced
        mv "$WORLD_DB" "/data/replaced/world-$(date +%Y%m%d-%H%M%S).db"
    fi
    rm -f "$WORLD_DB-wal" "$WORLD_DB-shm"
    mv /data/import.db "$WORLD_DB"
    echo "📦 moved in the uploaded world (the one it replaced is in /data/replaced)"
fi
# Everything in /data belongs to the world's own user (an upload, a command run
# as root, or a folder shared from the host can leave things that don't), and
# the world runs as that user rather than as root.
chown -R world:world /data
exec setpriv --reuid=world --regid=world --init-groups -- world.txt serve --addr 0.0.0.0:8080
