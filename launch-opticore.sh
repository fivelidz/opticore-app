#!/usr/bin/env bash
# Launch OptiCore — the Tauri app embeds the Rust server automatically.
cd "$(dirname "$0")"

# Demo credentials (change in production via Settings → Change Password)
export DEV_ADMIN_PASSWORD="${DEV_ADMIN_PASSWORD:-admin}"

# Database: opticore.db in the current directory (created on first run)
export DATABASE_URL="sqlite://$(pwd)/opticore.db?mode=rwc"
export PORT=3000

# Optional: enable Cloudflare Worker sync (online bookings)
# export WORKER_URL=https://opticore-booking.fivelidz.workers.dev
# export SYNC_SECRET=opticore-sync-2026

# WebKit fix for Linux
export WEBKIT_DISABLE_DMABUF_RENDERER=1

exec ./target/release/tauri-app "$@"
