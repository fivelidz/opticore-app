#!/usr/bin/env bash
# OptiCore DEMO — starts with demo data (sample patients, appointments, etc.)
cd "$(dirname "$0")"
export DEV_ADMIN_PASSWORD=admin
export DATABASE_URL="sqlite://$(pwd)/opticore-demo.db?mode=rwc"
export PORT=3000
export WEBKIT_DISABLE_DMABUF_RENDERER=1
exec ./target/release/tauri-app "$@"
