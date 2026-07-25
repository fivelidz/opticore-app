import { useState } from "react";
import { auth } from "../api";

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
      <p className="page-sub">Manage your account.</p>

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
