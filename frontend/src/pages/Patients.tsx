import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { patients as patApi } from "../api";

type SortKey = "last_name" | "first_name" | "mrn" | "date_of_birth" | "phone" | "next_appointment" | "last_appointment" | "total_visits" | "total_spent" | "outstanding";

export function Patients() {
  const nav = useNavigate();
  const [rows, setRows] = useState<any[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [showAdd, setShowAdd] = useState(false);
  const [sortKey, setSortKey] = useState<SortKey>("last_name");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  const load = (s?: string) => {
    setLoading(true);
    patApi.listEnriched(s).then((r) => setRows(r.data)).finally(() => setLoading(false));
  };
  useEffect(() => {
    const t = setTimeout(() => load(search || undefined), 250);
    return () => clearTimeout(t);
  }, [search]);
  useEffect(() => load(), []);

  const sorted = [...rows].sort((a, b) => {
    const av = a[sortKey] ?? "";
    const bv = b[sortKey] ?? "";
    // dates as strings sort correctly; numbers need numeric compare
    let cmp: number;
    if (typeof av === "number" && typeof bv === "number") cmp = av - bv;
    else cmp = String(av).localeCompare(String(bv));
    return sortDir === "asc" ? cmp : -cmp;
  });

  const toggleSort = (k: SortKey) => {
    if (sortKey === k) setSortDir(sortDir === "asc" ? "desc" : "asc");
    else { setSortKey(k); setSortDir("asc"); }
  };

  const arrow = (k: SortKey) => sortKey === k ? (sortDir === "asc" ? " ↑" : " ↓") : "";

  const exportCsv = () => {
    const headers = ["MRN", "Last Name", "First Name", "DOB", "Age", "Gender", "Phone", "Email", "Medicare", "Next Appointment", "Last Appointment", "Total Visits", "Total Spent", "Outstanding"];
    const age = (dob: string) => { try { return Math.floor((Date.now() - new Date(dob).getTime()) / (365.25 * 86400 * 1000)); } catch { return ""; } };
    const esc = (v: any) => { const s = String(v ?? ""); return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; };
    const fmtDt = (s?: string) => s ? new Date(s).toLocaleString() : "";
    const num = (v: any) => (typeof v === "number" ? v : 0);
    const lines = [headers.join(",")];
    for (const r of sorted) {
      lines.push([
        r.mrn, r.last_name, r.first_name, r.date_of_birth, age(r.date_of_birth), r.gender || "",
        r.phone || "", r.email || "", r.medicare_number || "",
        fmtDt(r.next_appointment), fmtDt(r.last_appointment),
        num(r.total_visits), num(r.total_spent).toFixed(2), num(r.outstanding).toFixed(2),
      ].map(esc).join(","));
    }
    const csv = lines.join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `patients-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const fmtDt = (s?: string) => s ? new Date(s).toLocaleDateString([], { day: "2-digit", month: "short" }) : "—";
  const fmtTime = (s?: string) => s ? new Date(s).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false }) : "";

  return (
    <div>
      <div className="header-row">
        <div>
          <h1 className="page-title">Patients</h1>
          <p className="page-sub">{rows.length} records</p>
        </div>
        <div className="actions-row">
          <button className="btn-ghost" onClick={exportCsv} disabled={rows.length === 0}>⬇ Export CSV</button>
          <button className="btn-primary" onClick={() => setShowAdd(true)}>+ Add Patient</button>
        </div>
      </div>

      <div className="card" style={{ marginBottom: 16 }}>
        <input
          placeholder="🔍 Search by name, MRN, phone, or email…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="card table-card">
        {loading ? <div className="empty">Loading…</div> :
         rows.length === 0 ? <div className="empty">No patients found.</div> :
          <table className="pat-table">
            <thead>
              <tr>
                <th className="sortable" onClick={() => toggleSort("mrn")}>MRN{arrow("mrn")}</th>
                <th className="sortable" onClick={() => toggleSort("last_name")}>Name{arrow("last_name")}</th>
                <th className="sortable" onClick={() => toggleSort("date_of_birth")}>DOB{arrow("date_of_birth")}</th>
                <th className="sortable" onClick={() => toggleSort("phone")}>Phone{arrow("phone")}</th>
                <th className="sortable next-col" onClick={() => toggleSort("next_appointment")}>Next Appt{arrow("next_appointment")}</th>
                <th className="sortable" onClick={() => toggleSort("last_appointment")}>Last Appt{arrow("last_appointment")}</th>
                <th className="sortable num" onClick={() => toggleSort("total_visits")}>Visits{arrow("total_visits")}</th>
                <th className="sortable num" onClick={() => toggleSort("total_spent")}>Spent{arrow("total_spent")}</th>
                <th className="sortable num" onClick={() => toggleSort("outstanding")}>Outstanding{arrow("outstanding")}</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((p) => (
                <tr key={p.id} className="pat-row" onClick={() => nav(`/patients/${p.id}`)}>
                  <td className="mono">{p.mrn}</td>
                  <td><strong>{p.last_name}, {p.first_name}</strong></td>
                  <td>{new Date(p.date_of_birth).toLocaleDateString()}</td>
                  <td>{p.phone || "—"}</td>
                  <td className="next-col">
                    {p.next_appointment ? (
                      <span className="appt-cell next">
                        <span className="appt-date">{fmtDt(p.next_appointment)}</span>
                        <span className="appt-time">{fmtTime(p.next_appointment)}</span>
                      </span>
                    ) : <span className="muted">—</span>}
                  </td>
                  <td>
                    {p.last_appointment ? (
                      <span className="appt-cell">
                        <span className="appt-date">{fmtDt(p.last_appointment)}</span>
                      </span>
                    ) : <span className="muted">—</span>}
                  </td>
                  <td className="num">{p.total_visits || 0}</td>
                  <td className="num">${(p.total_spent || 0).toFixed(0)}</td>
                  <td className="num">{(p.outstanding || 0) > 0 ? <span className="out">${(p.outstanding || 0).toFixed(0)}</span> : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>}
      </div>

      {showAdd && <AddPatientModal onClose={() => setShowAdd(false)} onSaved={() => { setShowAdd(false); load(search || undefined); }} />}

      <style>{`
        .header-row { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 20px; gap: 12px; flex-wrap: wrap; }
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; }
        .actions-row { display: flex; gap: 8px; }
        .empty { padding: 32px; text-align: center; color: var(--text-dim); }
        .table-card { overflow: auto; }
        .pat-table { width: 100%; border-collapse: collapse; }
        .pat-table th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-dim); padding: 10px 12px; border-bottom: 1px solid var(--border); white-space: nowrap; position: sticky; top: 0; background: var(--bg-elev); z-index: 1; }
        .pat-table th.sortable { cursor: pointer; user-select: none; }
        .pat-table th.sortable:hover { color: var(--accent); }
        .pat-table th.num { text-align: right; }
        .pat-table td { padding: 11px 12px; border-bottom: 1px solid var(--border); font-size: 14px; white-space: nowrap; }
        .pat-table tr:hover td { background: var(--bg-elev-2); }
        .pat-row { cursor: pointer; }
        .mono { font-family: ui-monospace, monospace; font-size: 12px; color: var(--text-dim); }
        .num { text-align: right; font-variant-numeric: tabular-nums; }
        .out { color: var(--red); font-weight: 600; }
        .muted { color: var(--text-dim); }
        .next-col { min-width: 110px; }
        .appt-cell { display: flex; flex-direction: column; gap: 1px; }
        .appt-cell.next .appt-date { color: var(--accent); font-weight: 600; }
        .appt-date { font-size: 13px; }
        .appt-time { font-size: 11px; color: var(--text-dim); }
      `}</style>
    </div>
  );
}

function AddPatientModal({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState({ first_name: "", last_name: "", date_of_birth: "", phone: "", email: "", gender: "", address: "", medicare_number: "" });
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState("");
  const set = (k: string, v: string) => setForm((f) => ({ ...f, [k]: v }));
  const save = async () => {
    setSaving(true); setErr("");
    try {
      await patApi.create(form);
      onSaved();
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Failed to save");
    } finally { setSaving(false); }
  };
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()}>
        <h2>New Patient</h2>
        <div className="form-grid">
          <div><label>First name *</label><input value={form.first_name} onChange={(e) => set("first_name", e.target.value)} /></div>
          <div><label>Last name *</label><input value={form.last_name} onChange={(e) => set("last_name", e.target.value)} /></div>
          <div><label>Date of birth *</label><input type="date" value={form.date_of_birth} onChange={(e) => set("date_of_birth", e.target.value)} /></div>
          <div><label>Gender</label><select value={form.gender} onChange={(e) => set("gender", e.target.value)}><option value="">—</option><option>female</option><option>male</option><option>other</option></select></div>
          <div><label>Phone</label><input value={form.phone} onChange={(e) => set("phone", e.target.value)} /></div>
          <div><label>Email</label><input value={form.email} onChange={(e) => set("email", e.target.value)} /></div>
          <div className="full"><label>Address</label><input value={form.address} onChange={(e) => set("address", e.target.value)} /></div>
          <div className="full"><label>Medicare number</label><input value={form.medicare_number} onChange={(e) => set("medicare_number", e.target.value)} /></div>
        </div>
        {err && <div style={{ color: "var(--red)", fontSize: 13, marginTop: 12 }}>{err}</div>}
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={save} disabled={saving || !form.first_name || !form.last_name || !form.date_of_birth}>{saving ? "Saving…" : "Save Patient"}</button>
        </div>
        <style>{`
          .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
          .modal { width: 560px; max-height: 90vh; overflow-y: auto; }
          .modal h2 { margin-bottom: 20px; }
          .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
          .form-grid label { display: block; font-size: 12px; font-weight: 600; color: var(--text-dim); margin-bottom: 5px; text-transform: uppercase; letter-spacing: 0.5px; }
          .form-grid .full { grid-column: 1 / -1; }
          .modal-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 24px; }
        `}</style>
      </div>
    </div>
  );
}
