#!/bin/sh
# Start the world on Fly.io. A world uploaded as /data/import.db (see the
# README) takes over here, before the server opens anything, so it's never
# swapped under a running world.
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
exec world.txt serve --addr 0.0.0.0:8080
