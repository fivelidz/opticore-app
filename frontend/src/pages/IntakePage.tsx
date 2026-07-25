import { useEffect, useState } from "react";
import { intake, type IntakeSubmission } from "../api";

export function IntakePage() {
  const [list, setList] = useState<IntakeSubmission[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<"new" | "all">("new");

  const load = () => {
    setLoading(true);
    intake.list().then((r) => setList(r.data)).finally(() => setLoading(false));
  };
  useEffect(load, []);

  const shown = filter === "new" ? list.filter((s) => s.status === "new") : list;
  const newCount = list.filter((s) => s.status === "new").length;

  const doImport = async (id: number) => { await intake.import(id); load(); };
  const doArchive = async (id: number) => { await intake.archive(id); load(); };

  return (
    <div>
      <div className="header-row">
        <div>
          <h1 className="page-title">📥 Intake Submissions</h1>
          <p className="page-sub">{newCount} new · {list.length} total · from the <a href="/input" target="_blank">input page</a></p>
        </div>
        <div className="filter-tabs">
          <button className={filter === "new" ? "ftab on" : "ftab"} onClick={() => setFilter("new")}>New ({newCount})</button>
          <button className={filter === "all" ? "ftab on" : "ftab"} onClick={() => setFilter("all")}>All</button>
          {newCount > 0 && <button className="btn-primary" onClick={async () => { const r = await intake.autoImport(); alert(`Imported ${r.data.imported} of ${r.data.total_new} submissions`); load(); }}>⚡ Auto-import all ({newCount})</button>}
        </div>
      </div>

      <div className="card info-banner">
        💡 The public input page is at <a href="/input" target="_blank"><strong>http://localhost:3000/input</strong></a>. Anyone can fill it in (no login). Submissions appear here for staff to review and import as patients.
      </div>

      {loading ? <div className="card empty">Loading…</div> :
        shown.length === 0 ? <div className="card empty">No submissions.</div> :
        <div className="intake-list">
          {shown.map((s) => (
            <div key={s.id} className={`card intake-card ${s.status}`}>
              <div className="ic-head">
                <div className="ic-name">{s.first_name} {s.last_name}</div>
                <span className={`badge ${s.status === "new" ? "badge-scheduled" : s.status === "imported" ? "badge-confirmed" : "badge-completed"}`}>{s.status}</span>
                <span className="ic-date">{new Date(s.submitted_at).toLocaleString()}</span>
              </div>
              <div className="ic-grid">
                {s.date_of_birth && <div><span>DOB</span>{s.date_of_birth}</div>}
                {s.phone && <div><span>Phone</span>{s.phone}</div>}
                {s.email && <div><span>Email</span>{s.email}</div>}
                {s.medicare_number && <div><span>Medicare</span>{s.medicare_number}</div>}
                {s.preferred_date && <div><span>Prefers</span>{s.preferred_date} {s.preferred_time || ""}</div>}
                {s.appointment_type && <div><span>Type</span>{s.appointment_type}</div>}
              </div>
              {s.symptoms && <div className="ic-symptoms"><span>Symptoms:</span> {s.symptoms}</div>}
              {s.status === "new" && (
                <div className="ic-actions">
                  <button className="btn-primary" onClick={() => doImport(s.id)}>✓ Import as Patient</button>
                  <button className="btn-ghost" onClick={() => doArchive(s.id)}>Archive</button>
                </div>
              )}
              {s.status === "imported" && s.matched_patient_id && (
                <a className="ic-link" href={`#/patients/${s.matched_patient_id}`}>→ View patient record</a>
              )}
            </div>
          ))}
        </div>}

      <style>{`
        .header-row { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 20px; }
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; }
        .page-sub a { color: var(--accent); }
        .filter-tabs { display: flex; gap: 4px; }
        .ftab { background: var(--bg-elev); color: var(--text-dim); border: 1px solid var(--border); padding: 6px 14px; border-radius: 8px; font-size: 13px; }
        .ftab.on { background: var(--accent); color: white; border-color: var(--accent); }
        .info-banner { margin-bottom: 16px; font-size: 13px; color: var(--text-dim); }
        .info-banner a { color: var(--accent); }
        .empty { padding: 32px; text-align: center; color: var(--text-dim); }
        .intake-list { display: flex; flex-direction: column; gap: 12px; }
        .intake-card { padding: 16px 20px; }
        .intake-card.imported { opacity: 0.7; }
        .ic-head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
        .ic-name { font-weight: 700; font-size: 16px; flex: 1; }
        .ic-date { color: var(--text-dim); font-size: 12px; }
        .ic-grid { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px 16px; font-size: 13px; }
        .ic-grid div span { color: var(--text-dim); display: block; font-size: 11px; text-transform: uppercase; }
        .ic-symptoms { font-size: 13px; margin-top: 10px; color: var(--text-dim); }
        .ic-symptoms span { color: var(--text); font-weight: 600; }
        .ic-actions { display: flex; gap: 8px; margin-top: 14px; }
        .ic-link { display: inline-block; margin-top: 10px; color: var(--accent); font-size: 13px; }
      `}</style>
    </div>
  );
}
