import { useEffect, useState } from "react";
import { auth, database, type DatabaseInfo } from "../api";

// The Tauri global is exposed via `withGlobalTauri: true` in tauri.conf.json.
// It is only present inside the desktop app (not in a plain browser / LAN
// access), so every use is guarded. Typed loosely as `any` to avoid adding a
// @tauri-apps/api npm dependency to the frontend.
function tauri(): any | null {
  return (typeof window !== "undefined" && (window as any).__TAURI__) || null;
}
function isTauri(): boolean {
  return tauri() != null;
}

/** Open a native "open file" dialog and return the chosen path (or null). */
async function pickExistingDb(): Promise<string | null> {
  const t = tauri();
  if (!t?.dialog?.open) return null;
  const selected = await t.dialog.open({
    multiple: false,
    directory: false,
    title: "Choose an OptiCore database file",
    filters: [{ name: "OptiCore database", extensions: ["db", "sqlite", "sqlite3"] }],
  });
  return typeof selected === "string" ? selected : null;
}

/** Open a native "save file" dialog and return the chosen path (or null). */
async function pickNewDb(): Promise<string | null> {
  const t = tauri();
  if (!t?.dialog?.save) return null;
  const selected = await t.dialog.save({
    title: "Create a new OptiCore database",
    defaultPath: "clinic.db",
    filters: [{ name: "OptiCore database", extensions: ["db", "sqlite", "sqlite3"] }],
  });
  return typeof selected === "string" ? selected : null;
}

/** Ask the Tauri shell to restart the whole app so the new DB takes effect. */
async function restartApp() {
  const t = tauri();
  if (t?.core?.invoke) {
    await t.core.invoke("restart_app");
  } else {
    // Fallback for older global shapes.
    if (t?.invoke) await t.invoke("restart_app");
  }
}

function fmtBytes(n: number): string {
  if (!n || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

function DatabaseCard() {
  const [info, setInfo] = useState<DatabaseInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [msg, setMsg] = useState("");

  const load = async () => {
    try {
      const r = await database.info();
      setInfo(r.data);
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Could not load database info.");
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { load(); }, []);

  const afterConfigured = async () => {
    setMsg("Database updated — restarting OptiCore…");
    // Give the user a moment to read the message, then restart.
    setTimeout(() => { restartApp().catch(() => {}); }, 700);
  };

  const onLink = async () => {
    setErr(""); setMsg("");
    if (!isTauri()) { setErr("File picking is only available in the desktop app."); return; }
    const path = await pickExistingDb();
    if (!path) return;
    // Prompt for the database password. For an unencrypted DB the user can
    // leave this blank, but encrypted DBs require the correct password.
    const password = prompt(
      "Enter the password for this database.\n\n" +
      "• If this is an encrypted OptiCore database, enter its password.\n" +
      "• If it has no password, leave this blank and click OK.\n\n" +
      "(OptiCore will remember the password so you only enter it once.)"
    );
    if (password === null) return; // user clicked Cancel
    setBusy(true);
    try {
      await database.link(path, password || undefined);
      await afterConfigured();
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Could not link to that database.");
      setBusy(false);
    }
  };

  const onNew = async () => {
    setErr(""); setMsg("");
    if (!isTauri()) { setErr("File picking is only available in the desktop app."); return; }
    const path = await pickNewDb();
    if (!path) return;
    const password = prompt(
      "Create a password for this new database.\n\n" +
      "⚠️  IMPORTANT: This password protects the database file. If you\n" +
      "    forget it, the data CANNOT be recovered — there is no reset.\n" +
      "    Write it down somewhere safe.\n\n" +
      "    The database file on its own (e.g. emailed or copied) cannot be\n" +
      "    opened without this password."
    );
    if (password === null) return;
    if (password.length < 4) {
      setErr("Password must be at least 4 characters.");
      return;
    }
    const confirm = prompt("Re-enter the password to confirm it is correct.");
    if (confirm === null) return;
    if (password !== confirm) {
      setErr("The passwords did not match. Please try again.");
      return;
    }
    setBusy(true);
    try {
      await database.create(path, password);
      await afterConfigured();
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Could not create a new database.");
      setBusy(false);
    }
  };

  const onDuplicate = async () => {
    setErr(""); setMsg("");
    if (!isTauri() || !info) { setErr("Only available in the desktop app."); return; }
    const dest = await pickNewDb();
    if (!dest) return;
    const password = prompt(
      "Enter the password for the current database to make a copy.\n" +
      "(The copy will have the same password.)"
    );
    if (password === null) return;
    setBusy(true);
    try {
      const r = await database.duplicate(info.current_path, dest, password || undefined);
      setMsg(r.data.restart_required
        ? "Copied — restarting OptiCore…"
        : `Copied to ${dest}. The original is unchanged.`);
      if (r.data.restart_required) await afterConfigured();
      else setBusy(false);
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Could not duplicate the database.");
      setBusy(false);
    }
  };

  const onLoadDemo = async () => {
    setErr(""); setMsg("");
    setBusy(true);
    try {
      await database.loadDemo();
      await afterConfigured();
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Could not load demo data.");
      setBusy(false);
    }
  };

  return (
    <div className="card" style={{ maxWidth: 640, marginBottom: 24 }}>
      <h2 className="section-title">Database</h2>

      {loading ? (
        <div style={{ color: "var(--text-dim)", fontSize: 13 }}>Loading…</div>
      ) : info ? (
        <>
          <label style={lab}>Current database file</label>
          <div style={pathBox}>{info.current_path}</div>

          <div style={{ display: "flex", gap: 18, flexWrap: "wrap", marginTop: 14, fontSize: 13 }}>
            <span><strong>{info.patient_count}</strong> patient{info.patient_count === 1 ? "" : "s"}</span>
            <span>{info.file_exists ? fmtBytes(info.file_size_bytes) : "not created yet"}</span>
            {info.is_demo_seeded && (
              <span style={badge}>DEMO DATA</span>
            )}
          </div>

          <div style={{ marginTop: 16, padding: "10px 12px", background: "var(--bg-soft, rgba(0,0,0,0.03))", borderRadius: 8, fontSize: 12.5, color: "var(--text-dim)", lineHeight: 1.5 }}>
            Each database is <strong>password-protected</strong> and stored as a
            separate file — so it survives uninstalling and reinstalling OptiCore,
            and a copied/emailed file can't be opened without its password.
            Changing the database restarts OptiCore. Your current data is never
            deleted — to go back to it, link to it again.
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 18 }}>
            <button className="btn-ghost" onClick={onLink} disabled={busy} style={btn}>
              Link to an existing database…
            </button>
            <button className="btn-ghost" onClick={onNew} disabled={busy} style={btn}>
              Start a new empty database…
            </button>
            <button className="btn-ghost" onClick={onDuplicate} disabled={busy || !info.file_exists} style={btn}>
              Duplicate this database (backup / copy)…
            </button>
            <button
              className="btn-ghost"
              onClick={onLoadDemo}
              disabled={busy || info.patient_count !== 0}
              style={btn}
              title={info.patient_count !== 0 ? "Only available when the database is empty" : ""}
            >
              Load sample demo data into this database
            </button>
          </div>

          {!isTauri() && (
            <div style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 12 }}>
              Note: linking or creating a database file is only available in the
              OptiCore desktop app.
            </div>
          )}

          {err && <div style={{ color: "var(--red)", fontSize: 13, marginTop: 12 }}>{err}</div>}
          {msg && <div style={{ color: "var(--green)", fontSize: 13, marginTop: 12 }}>{msg}</div>}
        </>
      ) : (
        <div style={{ color: "var(--red)", fontSize: 13 }}>{err || "Unavailable."}</div>
      )}
    </div>
  );
}

export function Settings() {
  const [cur, setCur] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr(""); setMsg("");
    if (next !== confirm) { setErr("New passwords don't match."); return; }
    if (next.length < 4) { setErr("New password must be at least 4 characters."); return; }
    setSaving(true);
    try {
      const r = await auth.changePassword(cur, next);
      setMsg(r.data.message || "Password changed.");
      setCur(""); setNext(""); setConfirm("");
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Failed to change password.");
    } finally { setSaving(false); }
  };

  return (
    <div>
      <h1 className="page-title">Settings</h1>
      <p className="page-sub">Manage your account and clinic data.</p>

      <DatabaseCard />

      <div className="card" style={{ maxWidth: 460 }}>
        <h2 className="section-title">Change Password</h2>
        <form onSubmit={submit}>
          <label style={lab}>Current password</label>
          <input type="password" value={cur} onChange={(e) => setCur(e.target.value)} autoFocus style={{ marginBottom: 14 }} />
          <label style={lab}>New password</label>
          <input type="password" value={next} onChange={(e) => setNext(e.target.value)} style={{ marginBottom: 14 }} />
          <label style={lab}>Confirm new password</label>
          <input type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
          {err && <div style={{ color: "var(--red)", fontSize: 13, marginTop: 12 }}>{err}</div>}
          {msg && <div style={{ color: "var(--green)", fontSize: 13, marginTop: 12 }}>{msg}</div>}
          <button type="submit" className="btn-primary" style={{ marginTop: 18, width: "100%" }} disabled={saving || !cur || !next || !confirm}>
            {saving ? "Saving…" : "Update Password"}
          </button>
        </form>
      </div>

      <style>{`
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; margin-bottom: 28px; }
        .section-title { font-size: 17px; font-weight: 600; margin-bottom: 16px; }
      `}</style>
    </div>
  );
}

const lab: React.CSSProperties = { display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-dim)", marginBottom: 5, textTransform: "uppercase", letterSpacing: 0.5 };
const pathBox: React.CSSProperties = { fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace", fontSize: 13, padding: "9px 11px", background: "var(--bg-soft, rgba(0,0,0,0.04))", border: "1px solid var(--border, rgba(0,0,0,0.1))", borderRadius: 8, wordBreak: "break-all" };
const badge: React.CSSProperties = { fontSize: 11, fontWeight: 700, letterSpacing: 0.5, color: "var(--amber, #b45309)", background: "rgba(180,83,9,0.12)", padding: "2px 8px", borderRadius: 999 };
const btn: React.CSSProperties = { width: "100%", textAlign: "left" };
