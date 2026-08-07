# OptiCore — LAN Multi-Device Access Guide

OptiCore can be used from any device on your local network (tablets, laptops,
other desktops) — not just the machine running the server. Every device reads
and writes the **same SQLite database**, so data stays in sync.

---

## Quick start

### On the server machine (this one)

```bash
cd /home/fivelidz/projects/opticore-app-pub

# Build the frontend (only needed once, or after UI changes)
cd frontend && npm run build && cd ..

# Start the server (binds to 0.0.0.0:3000 — all interfaces)
PORT=3000 cargo run --bin server
# OR use the launch script:
./launch-opticore.sh
```

The server prints:
```
🩺 OptiCore server listening on http://0.0.0.0:3000
```

### From other devices

Open a browser and navigate to one of:

| Network | URL | Use case |
|---------|-----|----------|
| **LAN (WiFi/Ethernet)** | `http://192.168.0.11:3000` | Same router/WiFi |
| **Tailscale** | `http://100.73.134.20:3000` | Remote devices on your Tailnet |

Log in with `admin` / `admin` (or whatever `DEV_ADMIN_PASSWORD` was set to).

That's it — the full PMS UI loads in the browser, and every read/write hits
the shared database on the server machine.

---

## How it works

Three pieces work together:

### 1. Server binds to all interfaces
The axum server binds to `0.0.0.0:3000` (not `127.0.0.1`), so it accepts
connections from any network interface — LAN, WiFi, Tailscale, etc.

### 2. SQLite WAL mode (concurrent access)
The database uses **WAL (Write-Ahead Logging)** journal mode with a 5-second
busy timeout. This is critical for multi-device access:

- **Without WAL** (old default): every write locks the entire database. A
  tablet writing a new appointment would block the reception desk from reading
  until the write finishes — and under contention, you'd get "database is
  locked" errors.
- **With WAL**: readers and one writer can coexist. Multiple devices can read
  simultaneously while one writes. The 5s busy timeout absorbs brief contention
  windows instead of failing instantly.

### 3. Frontend served by the server
The axum server serves the built React SPA (`frontend/dist/`) as a fallback.
When a browser on another device loads `http://<server-IP>:3000/`, it gets the
full PMS UI. The frontend uses **relative API URLs** (no hardcoded
`localhost`), so API calls automatically target whatever host:port the browser
loaded the page from.

---

## Configuration (env vars)

| Env var | Default | Purpose |
|---------|---------|---------|
| `PORT` | `3000` | Port the server listens on |
| `DATABASE_URL` | `sqlite://opticore.db?mode=rwc` | SQLite DB path (shared by all clients) |
| `FRONTEND_DIST` | `frontend/dist` | Path to the built SPA (served as fallback) |
| `DEV_ADMIN_PASSWORD` | `admin` | Initial admin password (first boot only) |
| `JWT_SECRET` | insecure dev fallback | JWT signing secret (set for production) |
| `OPTICORE_MODE` | demo | Set to `live` for real data mode |

---

## Firewall

Port 3000 must be open on the server machine's firewall. On CachyOS/Arch:

```bash
# Check if the port is open
sudo firewall-cmd --list-ports

# Open it if needed (firewalld)
sudo firewall-cmd --add-port=3000/tcp --permanent
sudo firewall-cmd --reload

# Or if using ufw
sudo ufw allow 3000/tcp
```

If you're on Tailscale, no firewall config is needed — Tailscale devices can
reach each other directly.

---

## Troubleshooting

**"Connection refused" from another device**
- Verify the server is running: `curl http://localhost:3000/api/health`
- Check the firewall (see above)
- Verify you're using the right IP: `ip addr | grep "inet "`

**"Database is locked" errors**
- These should be extremely rare with WAL mode + 5s busy_timeout
- If they persist, the contention is sustained (many simultaneous writers).
  SQLite is single-writer by design — for a small clinic this is fine, but
  if you hit this regularly, consider increasing busy_timeout or moving to
  PostgreSQL.

**UI loads but API calls fail (blank page, errors in console)**
- The frontend uses relative URLs, so this shouldn't happen
- Check browser console for CORS errors (shouldn't happen — CORS is `Any`)
- Verify the server is serving the SPA: `curl http://<server-IP>:3000/` should
  return HTML

**Changes not appearing on another device**
- All devices share the same DB, so changes are immediate
- Hard-refresh the browser (Ctrl+Shift+R) to clear cached frontend assets
