# Mosman Dry Eye Clinic PMS — Rust edition (`pms-rs/`)

The new Rust-based practice management system. See `../COLLABORATION/` for the
full architecture and roadmap.

## Layout

```
pms-rs/
├── shared/        — shared types (API contract between server, Tauri, frontend)
├── server/        — Rust LAN server (axum + sqlx + SQLite). The clinic backend.
│   ├── src/       — auth, routes (patients, appointments, blocked times, calendar)
│   └── migrations — SQL schema + seed
├── frontend/      — React + Vite + TypeScript UI (dark mode, calendar)
└── tauri-app/     — Tauri desktop shell wrapping the frontend
```

## Run it (localhost dev)

### 1. Start the Rust server
```bash
cd pms-rs
DATABASE_URL="sqlite://pms.db?mode=rwc" cargo run -p server
```
On first boot it prints a generated admin password to the log. Use `admin` + that
password to log in.

### 2. Open the desktop app
```bash
cd pms-rs
cargo tauri dev
```
This starts the Vite dev server and opens the Tauri window, which talks to the
server on `http://localhost:3000`.

### Or: run just the frontend in a browser
```bash
cd pms-rs/frontend
npm install && npm run dev
# open http://localhost:5173  (proxies /api -> :3000)
```

## What works

- ✅ Argon2 password hashing + JWT auth (always on; no unauthenticated PHI).
- ✅ First-boot admin provisioning with a generated password (no hardcoded creds).
- ✅ Patients CRUD with search + auto-generated MRN.
- ✅ Appointments CRUD + "today" view.
- ✅ Blocked times (calendar) — block lunch/leave; shows on the calendar.
- ✅ Combined calendar endpoint (`/api/calendar/:from/:to`) — appointments + blocked.
- ✅ React UI: Dashboard, Patients (with add modal), Week Calendar (with block-time modal).
- ✅ Dark / light mode toggle (persisted).
- ✅ Tauri desktop window.

## What's next (per roadmap)

- Phase 2: multi-PC LAN (server discovery, Windows Service installer).
- Phase 3: Cloudflare Worker booking pipeline + sync engine.
- Phase 4: website availability widget.
- Phase 5: multi-tenant + hardening + backups drill.

## Notes

- The `opticore/` TypeScript backend remains as the **specification** we ported
  from. It is not used at runtime by `pms-rs`.
- `JWT_SECRET` must be set in production (the server warns and falls back to an
  insecure dev secret otherwise).
