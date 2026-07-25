#!/usr/bin/env bash
# Launch OptiCore.
# By default starts with a CLEAN database (no demo data) — production mode.
# Set DEMO_MODE=1 to start with demo data instead.
cd "$(dirname "$0")"

if [ -z "$DEMO_MODE" ]; then
  export CLEAN_START=1
fi
export DEV_ADMIN_PASSWORD="${DEV_ADMIN_PASSWORD:-admin}"
export DATABASE_URL="sqlite://$(pwd)/pms.db?mode=rwc"
export PORT=3000
exec ./target/release/tauri-app "$@"
