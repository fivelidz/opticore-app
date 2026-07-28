#!/usr/bin/env bash
# OptiCore PRODUCTION — empty database, connected to online booking.
# Bookings from the website intake form sync in automatically.
cd "$(dirname "$0")"
export DEV_ADMIN_PASSWORD=admin
export DATABASE_URL="sqlite://$(pwd)/opticore.db?mode=rwc"
export PORT=3000
export WORKER_URL=https://opticore-booking.fivelidz.workers.dev
export SYNC_SECRET=opticore-sync-2026
export WEBKIT_DISABLE_DMABUF_RENDERER=1
exec ./target/release/tauri-app "$@"
