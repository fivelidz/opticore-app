import { useEffect, useState } from "react";
import { users as userApi, type StaffUser } from "../api";

const ROLES: Record<string, { color: string; perms: string }> = {
  admin: { color: "var(--red)", perms: "Full access" },
  doctor: { color: "var(--accent)", perms: "Clinical + billing" },
  nurse: { color: "var(--green)", perms: "Clinical" },
  receptionist: { color: "var(--amber)", perms: "Bookings + patients" },
  readonly: { color: "var(--text-dim)", perms: "View only" },
};

export function UsersPage() {
  const [list, setList] = useState<StaffUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [editing, setEditing] = useState<StaffUser | null>(null);
  const [err, setErr] = useState("");

  const load = () => {
    setLoading(true); setErr("");
    userApi.list().then((r) => setList(r.data)).catch((e) => setErr(e?.response?.data?.error || "Failed to load")).finally(() => setLoading(false));
  };
  useEffect(load, []);

  const stats = {
    total: list.length,
    active: list.filter((u) => u.is_active).length,
    admins: list.filter((u) => u.role === "admin").length,
    doctors: list.filter((u) => u.role === "doctor").length,
  };

  return (
    <div>
      <div className="header-row">
        <div>
          <h1 className="page-title">👥 User Management</h1>
          <p className="page-sub">{stats.active} active · {stats.total} total</p>
        </div>
        <button className="btn-primary" onClick={() => setShowAdd(true)}>+ Add User</button>
      </div>

      <div className="ustat-grid">
        <div className="card ustat"><div className="ustat-v">{stats.total}</div><div className="ustat-l">Total Users</div></div>
        <div className="card ustat"><div className="ustat-v" style={{ color: "var(--green)" }}>{stats.active}</div><div className="ustat-l">Active</div></div>
        <div className="card ustat"><div className="ustat-v" style={{ color: "var(--red)" }}>{stats.admins}</div><div className="ustat-l">Admins</div></div>
        <div className="card ustat"><div className="ustat-v" style={{ color: "var(--accent)" }}>{stats.doctors}</div><div className="ustat-l">Doctors</div></div>
      </div>

      {err && <div className="card" style={{ color: "var(--red)", marginBottom: 16 }}>{err}</div>}

      <div className="card">
        {loading ? <div className="empty">Loading…</div> :
          <table className="user-table">
            <thead>
              <tr><th>Name</th><th>Username</th><th>Email</th><th>Role</th><th>Status</th><th>Created</th><th></th></tr>
            </thead>
            <tbody>
              {list.map((u) => (
                <tr key={u.id}>
                  <td><strong>{u.first_name} {u.last_name}</strong></td>
                  <td className="mono">@{u.username}</td>
                  <td>{u.email}</td>
                  <td><span className="role-badge" style={{ background: ROLES[u.role]?.color + "22", color: ROLES[u.role]?.color }}>{u.role}</span></td>
                  <td>{u.is_active ? <span className="status-on">● Active</span> : <span className="status-off">● Disabled</span>}</td>
                  <td className="muted">{new Date(u.created_at).toLocaleDateString()}</td>
                  <td>
                    <div className="row-actions">
                      <button className="mini" onClick={() => setEditing(u)}>Edit</button>
                      <button className="mini" onClick={async () => { await userApi.toggle(u.id); load(); }}>{u.is_active ? "Disable" : "Enable"}</button>
                      {u.role !== "admin" && <button className="mini danger" onClick={async () => { if (confirm(`Delete user ${u.first_name} ${u.last_name}?`)) { await userApi.remove(u.id); load(); } }}>Delete</button>}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>}
      </div>

      {showAdd && <UserModal onClose={() => setShowAdd(false)} onSaved={() => { setShowAdd(false); load(); }} />}
      {editing && <UserModal user={editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); load(); }} />}

      <style>{`
        .header-row { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 20px; }
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; }
        .ustat-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-bottom: 20px; }
        .ustat { padding: 16px 20px; }
        .ustat-v { font-size: 28px; font-weight: 700; }
        .ustat-l { font-size: 12px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; }
        .empty { padding: 32px; text-align: center; color: var(--text-dim); }
        .user-table { width: 100%; border-collapse: collapse; }
        .user-table th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-dim); padding: 10px 12px; border-bottom: 1px solid var(--border); }
        .user-table td { padding: 12px; border-bottom: 1px solid var(--border); font-size: 14px; }
        .user-table tr:hover td { background: var(--bg-elev-2); }
        .mono { font-family: ui-monospace, monospace; font-size: 13px; color: var(--text-dim); }
        .muted { color: var(--text-dim); font-size: 13px; }
        .role-badge { padding: 2px 10px; border-radius: 999px; font-size: 11px; font-weight: 600; text-transform: capitalize; }
        .status-on { color: var(--green); font-size: 13px; }
        .status-off { color: var(--text-dim); font-size: 13px; }
        .row-actions { display: flex; gap: 4px; }
        .mini { background: var(--bg-elev-2); color: var(--text); border: 1px solid var(--border); padding: 4px 10px; border-radius: 6px; font-size: 12px; }
        .mini.danger { color: var(--red); }
      `}</style>
    </div>
  );
}

function UserModal({ user, onClose, onSaved }: { user?: StaffUser; onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState({
    first_name: user?.first_name || "",
    last_name: user?.last_name || "",
    username: user?.username || "",
    email: user?.email || "",
    role: user?.role || "receptionist",
    password: "",
  });
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");
  const set = (k: string, v: string) => setForm((f) => ({ ...f, [k]: v }));

  const save = async () => {
    setSaving(true); setErr("");
    try {
      if (user) {
        const patch: any = { first_name: form.first_name, last_name: form.last_name, email: form.email, role: form.role };
        if (form.password) patch.password = form.password;
        await userApi.update(user.id, patch);
      } else {
        await userApi.create(form);
      }
      onSaved();
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Failed to save");
    } finally { setSaving(false); }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 480 }}>
        <h2>{user ? "Edit User" : "Add User"}</h2>
        <div className="form-grid">
          <div><label style={LAB}>First name *</label><input value={form.first_name} onChange={(e) => set("first_name", e.target.value)} /></div>
          <div><label style={LAB}>Last name *</label><input value={form.last_name} onChange={(e) => set("last_name", e.target.value)} /></div>
          <div><label style={LAB}>Username *</label><input value={form.username} onChange={(e) => set("username", e.target.value)} disabled={!!user} /></div>
          <div><label style={LAB}>Email *</label><input value={form.email} onChange={(e) => set("email", e.target.value)} /></div>
          <div><label style={LAB}>Role</label>
            <select value={form.role} onChange={(e) => set("role", e.target.value)}>
              <option value="admin">Admin — full access</option>
              <option value="doctor">Doctor — clinical + billing</option>
              <option value="nurse">Nurse — clinical</option>
              <option value="receptionist">Receptionist — bookings + patients</option>
              <option value="readonly">Read-only — view only</option>
            </select>
          </div>
          <div><label style={LAB}>{user ? "New password (blank = keep)" : "Password *"}</label><input type="password" value={form.password} onChange={(e) => set("password", e.target.value)} /></div>
        </div>
        {form.role && <div className="role-hint" style={{ color: ROLES[form.role]?.color }}>{ROLES[form.role]?.perms}</div>}
        {err && <div style={{ color: "var(--red)", fontSize: 13, marginTop: 12 }}>{err}</div>}
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={save} disabled={saving || !form.first_name || !form.last_name || (!user && (!form.username || !form.password))}>{saving ? "Saving…" : (user ? "Save Changes" : "Create User")}</button>
        </div>
        <style>{`
          .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
          .modal { width: 480px; max-height: 90vh; overflow-y: auto; }
          .modal h2 { margin-bottom: 20px; }
          .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
          .form-grid label { display: block; font-size: 12px; font-weight: 600; color: var(--text-dim); margin-bottom: 5px; text-transform: uppercase; letter-spacing: 0.5px; }
          .role-hint { font-size: 12px; margin-top: 12px; font-weight: 600; }
          .modal-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 24px; }
        `}</style>
      </div>
    </div>
  );
}

const LAB: React.CSSProperties = { display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-dim)", marginBottom: 5, textTransform: "uppercase", letterSpacing: 0.5 };
