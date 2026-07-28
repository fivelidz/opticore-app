import { useEffect, useRef, useState, type TextareaHTMLAttributes } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  patientDetail, clinical, billing, appointments as apptApi, patients as patApi, photos as photoApi,
  type PatientDetail as PDetail, type PatientPhoto,
} from "../api";

// ---------- Auto-expanding textarea ----------
// Grows in height as text is added: sets height=auto then height=scrollHeight.
function autoGrow(el: HTMLTextAreaElement | null) {
  if (!el) return;
  el.style.height = "auto";
  el.style.height = el.scrollHeight + "px";
}

function AutoTextarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const ref = useRef<HTMLTextAreaElement | null>(null);
  // resize on mount and whenever the value changes (controlled inputs)
  useEffect(() => { autoGrow(ref.current); }, [props.value]);
  return (
    <textarea
      {...props}
      ref={ref}
      onInput={(e) => { autoGrow(e.currentTarget); props.onInput?.(e); }}
    />
  );
}

// ---------- Collapsible section wrapper ----------
function CollapsibleSection({
  id, title, extra, collapsed, onToggle, children,
}: {
  id: string;
  title: React.ReactNode;
  extra?: React.ReactNode;
  collapsed: boolean;
  onToggle: (id: string) => void;
  children: React.ReactNode;
}) {
  return (
    <section className="card">
      <h3 className="card-h collapsible" onClick={() => onToggle(id)}>
        <span className="collapse-chev">{collapsed ? "▸" : "▾"}</span>
        <span className="collapse-title">{title}</span>
        {extra && <span className="collapse-extra" onClick={(e) => e.stopPropagation()}>{extra}</span>}
      </h3>
      {!collapsed && <div className="collapse-body">{children}</div>}
    </section>
  );
}

// ---------- CSV export helpers ----------
function csvCell(v: unknown): string {
  const s = v == null ? "" : String(v);
  // escape if the value contains a comma, quote, or newline
  if (/[",\n\r]/.test(s)) return '"' + s.replace(/"/g, '""') + '"';
  return s;
}

function toCsv(headers: string[], rows: unknown[][]): string {
  const lines = [headers.map(csvCell).join(",")];
  for (const row of rows) lines.push(row.map(csvCell).join(","));
  return lines.join("\r\n");
}

function downloadCsv(filename: string, csv: string) {
  // prepend BOM so Excel opens UTF-8 correctly
  const blob = new Blob(["\uFEFF" + csv], { type: "text/csv;charset=utf-8;" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function PatientDetailPage() {
  const { id } = useParams();
  const nav = useNavigate();
  const pid = Number(id);
  const [data, setData] = useState<PDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadErr, setLoadErr] = useState("");
  const [showOsdi, setShowOsdi] = useState(false);
  const [showIpl, setShowIpl] = useState(false);
  // Collapse state: a Set of section ids that are currently collapsed.
  // Default: the longer sections start collapsed to keep the page compact.
  const [collapsed, setCollapsed] = useState<Set<string>>(
    () => new Set(["photos", "invoices"]),
  );
  const toggle = (sid: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      next.has(sid) ? next.delete(sid) : next.add(sid);
      return next;
    });

  const load = () => {
    setLoading(true); setLoadErr("");
    patientDetail.get(pid).then((r) => setData(r.data))
      .catch((e) => setLoadErr(e?.response?.status === 404 ? "Patient not found" : "Failed to load patient"))
      .finally(() => setLoading(false));
  };
  useEffect(load, [pid]);

  if (loading) return <div className="page-title">Loading…</div>;
  if (loadErr) return <div className="card" style={{ maxWidth: 420, margin: "60px auto", textAlign: "center", color: "var(--red)" }}>{loadErr}</div>;
  if (!data) return null;
  const p = data.patient;
  const age = p.date_of_birth ? Math.floor((Date.now() - new Date(p.date_of_birth).getTime()) / (365.25 * 86400 * 1000)) : 0;

  return (
    <div className="pd-page">
      <button className="btn-ghost back-btn" onClick={() => nav("/patients")}>‹ Back to Patients</button>

      {/* Header */}
      <div className="pd-header card">
        <div className="pd-avatar">{(p.first_name?.[0] || "?")}{(p.last_name?.[0] || "")}</div>
        <div className="pd-id">
          <h1>{p.first_name} {p.last_name}</h1>
          <div className="pd-meta"><span className="mono">{p.mrn}</span> · {age}y · {p.gender || "—"} · {p.phone || "no phone"}</div>
          <div className="pd-stats">
            <span><strong>{data.stats.total_visits}</strong> visits</span>
            <span><strong>${data.stats.total_spent.toFixed(0)}</strong> spent</span>
            {data.stats.outstanding > 0 && <span className="out"><strong>${data.stats.outstanding.toFixed(0)}</strong> outstanding</span>}
            {data.allergies.length > 0 && <span className="allergy-flag">⚠️ {data.allergies.length} allergy</span>}
            {data.stats.first_visit && <span className="muted">First: {new Date(data.stats.first_visit).toLocaleDateString()}</span>}
          </div>
        </div>
        <div className="pd-actions">
          <button className="btn-primary" onClick={() => nav(`/checkout/${pid}`)}>💳 Checkout</button>
          <button className="btn-ghost" onClick={() => nav("/calendar")}>📅 Book</button>
        </div>
      </div>

      {/* Everything below scrolls on one page */}
      <div className="pd-scroll">
        {/* Contact + allergies side by side */}
        <div className="pd-row-2">
          <ContactCard patient={p} age={age} firstVisit={data.stats.first_visit} pid={pid} onChanged={load} collapsed={collapsed.has("contact")} onToggle={toggle} />
          <CollapsibleSection id="allergies" collapsed={collapsed.has("allergies")} onToggle={toggle}
            title={<>Allergies {data.allergies.length > 0 && <span className="count-warn">⚠️</span>}</>}>
            {data.allergies.length === 0 ? <div className="empty-sm">✓ No known allergies</div> :
              data.allergies.map((a) => (
                <div key={a.id} className="allergy-row">
                  <span className="allergy-sub">{a.substance}</span>
                  <span className={`sev sev-${a.severity}`}>{a.severity}</span>
                  <button className="mini danger" onClick={async () => { await clinical.delAllergy(a.id); load(); }}>×</button>
                </div>
              ))}
            <AddAllergy pid={pid} onAdded={load} />
          </CollapsibleSection>
        </div>

        {/* OSDI + IPL */}
        <div className="pd-row-2">
          <CollapsibleSection id="osdi" collapsed={collapsed.has("osdi")} onToggle={toggle}
            title="OSDI Scores (dry eye)"
            extra={<button className="mini add-btn" onClick={() => setShowOsdi(true)}>+ Add</button>}>
            {data.osdi_scores.length === 0 ? <div className="empty-sm">No OSDI recorded</div> :
              data.osdi_scores.map((o) => (
                <div key={o.id} className="osdi-row">
                  <div className="osdi-big" style={{ color: osdiColor(o.total_score) }}>{o.total_score.toFixed(1)}</div>
                  <div>
                    <div className="osdi-lab">{osdiLabel(o.total_score)}</div>
                    <div className="osdi-sub">Ocular {o.ocular_symptoms?.toFixed(0)} · Vision {o.vision_function?.toFixed(0)} · Env {o.environmental_triggers?.toFixed(0)}</div>
                  </div>
                  <div className="osdi-d">{new Date(o.score_date).toLocaleDateString()}</div>
                </div>
              ))}
          </CollapsibleSection>
          <CollapsibleSection id="ipl" collapsed={collapsed.has("ipl")} onToggle={toggle}
            title="IPL Treatments"
            extra={<button className="mini add-btn" onClick={() => setShowIpl(true)}>+ Add</button>}>
            {data.ipl_treatments.length === 0 ? <div className="empty-sm">No IPL treatments</div> :
              data.ipl_treatments.map((t) => (
                <div key={t.id} className="ipl-row">
                  <div className="ipl-badge">S{t.session_number}</div>
                  <div className="ipl-det">
                    <div className="ipl-date">{new Date(t.treatment_date).toLocaleDateString()}</div>
                    <div className="muted">{t.fluence_j_cm2?.toFixed(1)} J/cm² · {t.number_of_pulses || 0} pulses · {t.operator_name}</div>
                    {t.clinical_notes && <div className="ipl-note">{t.clinical_notes}</div>}
                  </div>
                </div>
              ))}
          </CollapsibleSection>
        </div>

        {/* Appointments */}
        <CollapsibleSection id="appts" collapsed={collapsed.has("appts")} onToggle={toggle}
          title={`Appointment History (${data.appointments.length})`}
          extra={data.appointments.length > 0 && (
            <button className="mini" onClick={() => exportAppointmentNotes(p, data.appointments)}>⬇ Export appointment notes (CSV)</button>
          )}>
          {data.appointments.length === 0 ? <div className="empty-sm">No appointments</div> :
            <div className="appt-history">
              {data.appointments.map((a) => (
                <AppointmentRow key={a.id} appt={a} onChanged={load} />
              ))}
            </div>}
        </CollapsibleSection>

        {/* Photos & documents */}
        <PhotosSection pid={pid} onChanged={load} collapsed={collapsed.has("photos")} onToggle={toggle} />

        {/* Clinical notes */}
        <NotesSection pid={pid} patient={p} notes={data.notes} onChanged={load} collapsed={collapsed.has("notes")} onToggle={toggle} />

        {/* Billing */}
        <CollapsibleSection id="invoices" collapsed={collapsed.has("invoices")} onToggle={toggle}
          title={`Invoices & Payments (${data.invoices.length})`}>
          {data.invoices.length === 0 ? <div className="empty-sm">No invoices</div> :
            <table className="dt">
              <thead><tr><th>Invoice</th><th>Date</th><th>Total</th><th>Paid</th><th>Balance</th><th>Status</th></tr></thead>
              <tbody>
                {data.invoices.map((inv) => (
                  <tr key={inv.id}>
                    <td className="mono">{inv.invoice_number}</td>
                    <td>{new Date(inv.invoice_date).toLocaleDateString()}</td>
                    <td>${inv.total_amount.toFixed(2)}</td>
                    <td>${inv.amount_paid.toFixed(2)}</td>
                    <td className={inv.balance_due > 0 ? "out" : ""}>${inv.balance_due.toFixed(2)}</td>
                    <td><span className={`badge badge-${inv.status === "paid" ? "confirmed" : inv.status === "partially_paid" ? "scheduled" : "completed"}`}>{inv.status}</span></td>
                  </tr>
                ))}
              </tbody>
            </table>}
        </CollapsibleSection>
      </div>
      {showOsdi && <AddOsdiModal pid={pid} onClose={() => setShowOsdi(false)} onSaved={() => { setShowOsdi(false); load(); }} />}
      {showIpl && <AddIplModal pid={pid} onClose={() => setShowIpl(false)} onSaved={() => { setShowIpl(false); load(); }} />}
      <style>{PD_STYLE}</style>
    </div>
  );
}

function PhotosSection({ pid, onChanged, collapsed, onToggle }: { pid: number; onChanged: () => void; collapsed: boolean; onToggle: (id: string) => void }) {
  const [photos, setPhotos] = useState<PatientPhoto[]>([]);
  const [tab, setTab] = useState<"profile" | "medical" | "document">("medical");
  const [dataCache, setDataCache] = useState<Record<number, string>>({});
  const [viewing, setViewing] = useState<PatientPhoto | null>(null);
  const [uploading, setUploading] = useState(false);

  const load = () => photoApi.list(pid).then((r) => setPhotos(r.data));
  useEffect(() => { load(); }, [pid]);

  // lazy-load image data for visible photos
  useEffect(() => {
    photos.filter((p) => p.category === tab && !dataCache[p.id]).slice(0, 12).forEach((p) => {
      photoApi.getData(pid, p.id).then((r) => setDataCache((c) => ({ ...c, [p.id]: `data:${r.data.mime};base64,${r.data.data}` })));
    });
  }, [photos, tab]);

  const shown = photos.filter((p) => p.category === tab);

  const onFile = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setUploading(true);
    for (const f of Array.from(files)) {
      const data_base64 = await fileToBase64(f);
      await photoApi.upload(pid, { category: tab, filename: f.name, mime_type: f.type || "image/jpeg", data_base64 });
    }
    setUploading(false); load();
  };

  const tabs: [typeof tab, string, string][] = [
    ["profile", "Profile", "🪪"],
    ["medical", "Medical", "🔬"],
    ["document", "Documents", "📄"],
  ];

  return (
    <CollapsibleSection id="photos" collapsed={collapsed} onToggle={onToggle}
      title={`Photos & Documents (${photos.length})`}>
      <div className="photo-tabs">
        {tabs.map(([t, label, icon]) => (
          <button key={t} className={tab === t ? "photo-tab on" : "photo-tab"} onClick={() => setTab(t)}>
            {icon} {label} <span className="photo-count">{photos.filter((p) => p.category === t).length}</span>
          </button>
        ))}
        <label className="photo-upload-btn">
          {uploading ? "Uploading…" : "+ Upload"}
          <input type="file" multiple accept="image/*,application/pdf" style={{ display: "none" }} onChange={(e) => onFile(e.target.files)} />
        </label>
      </div>
      <div className="photo-grid">
        {shown.length === 0 && <div className="empty-sm">No {tab} files yet</div>}
        {shown.map((p) => (
          <div key={p.id} className="photo-tile" onClick={() => setViewing(p)}>
            {dataCache[p.id] ? (
              <img src={dataCache[p.id]} alt={p.filename} />
            ) : (
              <div className="photo-loading">…</div>
            )}
            <div className="photo-cap">{p.caption || p.filename}</div>
            <div className="photo-date">{new Date(p.captured_at).toLocaleDateString()}</div>
          </div>
        ))}
      </div>
      {viewing && <PhotoViewer pid={pid} photo={viewing} dataCache={dataCache} onClose={() => setViewing(null)} onChanged={() => { load(); onChanged(); }} />}
      <style>{`
        .photo-tabs { display: flex; gap: 4px; margin-bottom: 14px; align-items: center; flex-wrap: wrap; }
        .photo-tab { padding: 6px 12px; border-radius: 8px; font-size: 13px; font-weight: 600; color: var(--text-dim); background: var(--bg-elev-2); border: 1px solid var(--border); }
        .photo-tab.on { background: var(--accent); color: white; border-color: var(--accent); }
        .photo-count { font-size: 11px; opacity: 0.7; }
        .photo-upload-btn { margin-left: auto; padding: 6px 14px; border-radius: 8px; background: var(--accent); color: white; font-size: 13px; font-weight: 600; cursor: pointer; }
        .photo-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; }
        .photo-tile { background: var(--bg-elev-2); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; cursor: pointer; transition: border-color 0.15s; }
        .photo-tile:hover { border-color: var(--accent); }
        .photo-tile img { width: 100%; height: 120px; object-fit: cover; display: block; }
        .photo-loading { height: 120px; display: flex; align-items: center; justify-content: center; color: var(--text-dim); }
        .photo-cap { padding: 6px 8px 2px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .photo-date { padding: 0 8px 6px; font-size: 10px; color: var(--text-dim); }
        .empty-sm { padding: 12px; text-align: center; color: var(--text-dim); font-size: 13px; grid-column: 1/-1; }
      `}</style>
    </CollapsibleSection>
  );
}

function PhotoViewer({ pid, photo, dataCache, onClose, onChanged }: any) {
  const [src, setSrc] = useState(dataCache[photo.id] || "");
  const [caption, setCaption] = useState(photo.caption || "");
  useEffect(() => { if (!src) photoApi.getData(pid, photo.id).then((r) => setSrc(`data:${r.data.mime};base64,${r.data.data}`)); }, []);
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card photo-viewer" onClick={(e) => e.stopPropagation()}>
        <div className="pv-head">
          <span>{photo.filename}</span>
          <button className="sp-close" onClick={onClose}>×</button>
        </div>
        {src ? <img src={src} alt={photo.filename} className="pv-img" /> : <div>Loading…</div>}
        <div className="pv-meta">
          <span className={`badge badge-${photo.category === "profile" ? "confirmed" : photo.category === "medical" ? "scheduled" : "completed"}`}>{photo.category}</span>
          <span className="muted">{new Date(photo.captured_at).toLocaleString()}</span>
        </div>
        <div className="pv-actions">
          {photo.category !== "profile" && <button className="mini" onClick={async () => { await photoApi.makeProfile(pid, photo.id); onChanged(); }}>Set as profile</button>}
          <button className="mini danger" onClick={async () => { if (confirm("Delete this file?")) { await photoApi.remove(pid, photo.id); onChanged(); } }}>Delete</button>
        </div>
        <style>{`
          .photo-viewer { width: 600px; max-width: 90vw; }
          .pv-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; font-weight: 600; }
          .sp-close { background: none; color: var(--text-dim); font-size: 22px; }
          .pv-img { width: 100%; max-height: 60vh; object-fit: contain; border-radius: 8px; background: var(--bg-elev-2); }
          .pv-meta { display: flex; gap: 12px; align-items: center; margin-top: 12px; }
          .muted { color: var(--text-dim); font-size: 13px; }
          .pv-actions { display: flex; gap: 8px; margin-top: 12px; }
          .mini { background: var(--bg-elev-2); color: var(--text); border: 1px solid var(--border); padding: 4px 10px; border-radius: 6px; font-size: 12px; }
          .mini.danger { color: var(--red); }
        `}</style>
      </div>
    </div>
  );
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      // strip the data URL prefix
      const base64 = result.includes(",") ? result.split(",")[1] : result;
      resolve(base64);
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

function AppointmentRow({ appt, onChanged }: { appt: PDetail["appointments"][0]; onChanged: () => void }) {
  const [expanded, setExpanded] = useState(false);
  const [editNotes, setEditNotes] = useState(false);
  const [notes, setNotes] = useState(appt.notes || "");
  const [saving, setSaving] = useState(false);
  const dt = new Date(appt.appointment_date);
  const saveNotes = async () => {
    setSaving(true);
    await apptApi.update(appt.id, { appointment_type: appt.appointment_type, appointment_date: appt.appointment_date, duration_minutes: appt.duration_minutes, practitioner: appt.practitioner || undefined, status: appt.status, notes });
    setSaving(false); setEditNotes(false); onChanged();
  };
  return (
    <div className={`appt-h-row ${expanded ? "open" : ""}`}>
      <div className="appt-h-head" onClick={() => setExpanded(!expanded)}>
        <span className="appt-h-date">{dt.toLocaleDateString()} <span className="muted">{dt.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })}</span></span>
        <span className="appt-h-type"><strong>{appt.appointment_type}</strong></span>
        <span className="muted">{appt.duration_minutes}m</span>
        {appt.practitioner && <span className="muted">{appt.practitioner}</span>}
        <span className={`badge badge-${appt.status}`}>{appt.status}</span>
        {appt.notes && <span className="appt-has-note" title="Has notes">📝</span>}
        <span className="appt-h-chev">{expanded ? "▲" : "▼"}</span>
      </div>
      {expanded && (
        <div className="appt-h-body">
          <div className="appt-h-detail">
            <div><span className="muted">Date:</span> {dt.toLocaleString()}</div>
            <div><span className="muted">Duration:</span> {appt.duration_minutes} minutes</div>
            <div><span className="muted">Practitioner:</span> {appt.practitioner || "—"}</div>
            <div><span className="muted">Status:</span> <span className={`badge badge-${appt.status}`}>{appt.status}</span></div>
          </div>
          <div className="appt-h-notes-section">
            <div className="appt-h-notes-label">📝 Appointment Notes</div>
            {editNotes ? (
              <>
                <AutoTextarea className="appt-h-edit" rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Pre-visit reason, post-visit findings, instructions…" />
                <div className="appt-h-edit-actions">
                  <button className="mini" onClick={() => { setEditNotes(false); setNotes(appt.notes || ""); }}>Cancel</button>
                  <button className="btn-primary mini" onClick={saveNotes} disabled={saving}>{saving ? "Saving…" : "Save Notes"}</button>
                </div>
              </>
            ) : (
              <>
                <div className="appt-h-note-text">{appt.notes || <span className="muted">No notes recorded for this appointment.</span>}</div>
                <button className="mini" onClick={() => { setEditNotes(true); setNotes(appt.notes || ""); }}>✎ Edit notes</button>
              </>
            )}
          </div>
          <div className="appt-h-actions">
            {appt.status === "scheduled" && <button className="mini" onClick={async () => { await apptApi.update(appt.id, { appointment_type: appt.appointment_type, appointment_date: appt.appointment_date, duration_minutes: appt.duration_minutes, practitioner: appt.practitioner || undefined, status: "completed", notes: appt.notes || undefined }); onChanged(); }}>✓ Mark completed</button>}
            {appt.status === "scheduled" && <button className="mini danger" onClick={async () => { await apptApi.update(appt.id, { appointment_type: appt.appointment_type, appointment_date: appt.appointment_date, duration_minutes: appt.duration_minutes, practitioner: appt.practitioner || undefined, status: "cancelled", notes: appt.notes || undefined }); onChanged(); }}>✕ Cancel</button>}
          </div>
        </div>
      )}
    </div>
  );
}

function NotesSection({ pid, patient, notes, onChanged, collapsed, onToggle }: { pid: number; patient: PDetail["patient"]; notes: PDetail["notes"]; onChanged: () => void; collapsed: boolean; onToggle: (id: string) => void }) {
  const [note, setNote] = useState("");
  const [cat, setCat] = useState("general");
  const [saving, setSaving] = useState(false);
  const [confirmDel, setConfirmDel] = useState<number | null>(null);
  const author = (() => { try { const u = JSON.parse(localStorage.getItem("pms_user") || "{}"); return `${u.first_name || ""} ${u.last_name || ""}`.trim() || "Staff"; } catch { return "Staff"; } })();
  const add = async () => {
    if (!note.trim()) return;
    setSaving(true);
    await clinical.addNote({ patient_id: pid, category: cat, note, author });
    setNote(""); setSaving(false); onChanged();
  };
  return (
    <CollapsibleSection id="notes" collapsed={collapsed} onToggle={onToggle}
      title={`Clinical Notes (${notes.length})`}
      extra={notes.length > 0 && (
        <button className="mini" onClick={() => exportClinicalNotes(patient, notes)}>⬇ Export all clinical notes (CSV)</button>
      )}>
      <div className="note-add-row">
        <select value={cat} onChange={(e) => setCat(e.target.value)} style={{ width: "auto" }}>
          <option value="general">General</option><option value="assessment">Assessment</option>
          <option value="treatment">Treatment</option><option value="followup">Follow-up</option>
        </select>
        <AutoTextarea className="note-add-input" rows={1} placeholder="Add a clinical note…" value={note}
          onChange={(e) => setNote(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); add(); } }} />
        <button className="btn-primary" onClick={add} disabled={saving || !note.trim()}>{saving ? "…" : "Add"}</button>
      </div>
      <div className="note-list">
        {notes.map((n) => (
          <div key={n.id} className="note-item">
            <div className="note-head">
              <span className={`badge badge-${n.category === "assessment" ? "confirmed" : n.category === "treatment" ? "scheduled" : "completed"}`}>{n.category}</span>
              <span className="note-author">{n.author || "Unknown"}</span>
              <span className="note-date">{new Date(n.created_at).toLocaleString()}</span>
              {confirmDel === n.id ? (
                <>
                  <button className="mini danger" onClick={async () => { await clinical.delNote(n.id); setConfirmDel(null); onChanged(); }}>Confirm delete</button>
                  <button className="mini" onClick={() => setConfirmDel(null)}>Cancel</button>
                </>
              ) : (
                <button className="mini danger" onClick={() => setConfirmDel(n.id)}>Delete</button>
              )}
            </div>
            <p className="note-text">{n.note}</p>
          </div>
        ))}
        {notes.length === 0 && <div className="empty-sm">No notes yet</div>}
      </div>
    </CollapsibleSection>
  );
}

// ---------- CSV export builders ----------
function exportClinicalNotes(patient: PDetail["patient"], notes: PDetail["notes"]) {
  const rows = notes.map((n) => [
    new Date(n.created_at).toLocaleString(),
    n.category,
    n.author || "",
    n.note,
  ]);
  const csv = toCsv(["Date", "Category", "Author", "Note"], rows);
  downloadCsv(`clinical-notes_${patient.mrn || patient.id}.csv`, csv);
}

function exportAppointmentNotes(patient: PDetail["patient"], appts: PDetail["appointments"]) {
  const rows = appts.map((a) => [
    new Date(a.appointment_date).toLocaleString(),
    a.appointment_type,
    a.practitioner || "",
    a.status,
    a.notes || "",
  ]);
  const csv = toCsv(["Appointment Date", "Type", "Practitioner", "Status", "Notes"], rows);
  downloadCsv(`appointment-notes_${patient.mrn || patient.id}.csv`, csv);
}

function AddAllergy({ pid, onAdded }: { pid: number; onAdded: () => void }) {
  const [sub, setSub] = useState("");
  const [sev, setSev] = useState("mild");
  const add = async () => {
    if (!sub.trim()) return;
    await clinical.addAllergy({ patient_id: pid, substance: sub, severity: sev });
    setSub(""); onAdded();
  };
  return (
    <div className="add-allergy">
      <input placeholder="+ Add allergy…" value={sub} onChange={(e) => setSub(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} />
      <select value={sev} onChange={(e) => setSev(e.target.value)}>
        <option value="mild">Mild</option><option value="moderate">Moderate</option><option value="severe">Severe</option>
      </select>
      <button className="mini" onClick={add}>Add</button>
    </div>
  );
}

function Row({ label, v }: { label: string; v?: string | null }) {
  return <div className="ov-row"><span className="ov-label">{label}</span><span>{v || "—"}</span></div>;
}
function osdiLabel(s: number) { return s <= 12 ? "Normal" : s <= 22 ? "Mild" : s <= 32 ? "Moderate" : s <= 45 ? "Severe" : "Very Severe"; }
function osdiColor(s: number) { return s <= 12 ? "var(--green)" : s <= 22 ? "var(--accent)" : s <= 32 ? "var(--amber)" : "var(--red)"; }

function ContactCard({ patient, age, firstVisit, pid, onChanged, collapsed, onToggle }: any) {
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState({ ...patient });
  const [saving, setSaving] = useState(false);
  const set = (k: string, v: string) => setForm((f: any) => ({ ...f, [k]: v }));
  const save = async () => {
    setSaving(true);
    try {
      const { id, created_at, updated_at, mrn, ...rest } = form;
      await patApi.update(pid, rest);
      setEditing(false); onChanged();
    } finally { setSaving(false); }
  };
  return (
    <CollapsibleSection id="contact" collapsed={collapsed} onToggle={onToggle}
      title="Contact & Demographics"
      extra={!editing && <button className="mini add-btn" onClick={() => { setForm({ ...patient }); setEditing(true); }}>✎ Edit</button>}>
      {!editing ? (
        <>
          <Row label="Phone" v={patient.phone} />
          <Row label="Email" v={patient.email} />
          <Row label="Address" v={patient.address} />
          <Row label="Date of birth" v={patient.date_of_birth ? `${new Date(patient.date_of_birth).toLocaleDateString()} (${age} years)` : "—"} />
          <Row label="Gender" v={patient.gender} />
          <Row label="Medicare" v={patient.medicare_number} />
          <Row label="Patient since" v={firstVisit ? new Date(firstVisit).toLocaleDateString() : "—"} />
        </>
      ) : (
        <div className="edit-grid">
          <div><label style={LAB}>Phone</label><input value={form.phone || ""} onChange={(e) => set("phone", e.target.value)} /></div>
          <div><label style={LAB}>Email</label><input value={form.email || ""} onChange={(e) => set("email", e.target.value)} /></div>
          <div className="full"><label style={LAB}>Address</label><input value={form.address || ""} onChange={(e) => set("address", e.target.value)} /></div>
          <div><label style={LAB}>Date of birth</label><input type="date" value={form.date_of_birth?.slice(0, 10) || ""} onChange={(e) => set("date_of_birth", e.target.value)} /></div>
          <div><label style={LAB}>Gender</label><select value={form.gender || ""} onChange={(e) => set("gender", e.target.value)}><option value="">—</option><option>female</option><option>male</option><option>other</option></select></div>
          <div className="full"><label style={LAB}>Medicare</label><input value={form.medicare_number || ""} onChange={(e) => set("medicare_number", e.target.value)} /></div>
          <div className="edit-actions">
            <button className="btn-ghost" onClick={() => setEditing(false)}>Cancel</button>
            <button className="btn-primary" onClick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
          </div>
        </div>
      )}
    </CollapsibleSection>
  );
}

function AddOsdiModal({ pid, onClose, onSaved }: any) {
  const [total, setTotal] = useState("");
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    await clinical.addOsdi({ patient_id: pid, score_date: date, total_score: Number(total) });
    setSaving(false); onSaved();
  };
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 380 }}>
        <h2>Add OSDI Score</h2>
        <label style={LAB}>Score date</label>
        <input type="date" value={date} onChange={(e) => setDate(e.target.value)} style={{ marginBottom: 12 }} />
        <label style={LAB}>Total score (0–100)</label>
        <input type="number" min="0" max="100" step="0.1" value={total} onChange={(e) => setTotal(e.target.value)} autoFocus style={{ marginBottom: 12 }} />
        {total && <div className="muted" style={{ marginBottom: 12 }}>Severity: {osdiLabel(Number(total))}</div>}
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={save} disabled={saving || !total}>{saving ? "Saving…" : "Add Score"}</button>
        </div>
      </div>
    </div>
  );
}

function AddIplModal({ pid, onClose, onSaved }: any) {
  const [session, setSession] = useState("1");
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const [fluence, setFluence] = useState("");
  const [pulses, setPulses] = useState("");
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    await clinical.addIpl({ patient_id: pid, treatment_date: date, session_number: Number(session), fluence_j_cm2: fluence ? Number(fluence) : undefined, number_of_pulses: pulses ? Number(pulses) : undefined, operator_name: "Dr. Chapman-Davies", clinical_notes: notes || undefined });
    setSaving(false); onSaved();
  };
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 420 }}>
        <h2>Add IPL Treatment</h2>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
          <div><label style={LAB}>Session #</label><input type="number" min="1" value={session} onChange={(e) => setSession(e.target.value)} /></div>
          <div><label style={LAB}>Date</label><input type="date" value={date} onChange={(e) => setDate(e.target.value)} /></div>
          <div><label style={LAB}>Fluence (J/cm²)</label><input type="number" step="0.1" value={fluence} onChange={(e) => setFluence(e.target.value)} /></div>
          <div><label style={LAB}>Pulses</label><input type="number" value={pulses} onChange={(e) => setPulses(e.target.value)} /></div>
        </div>
        <label style={{ ...LAB, marginTop: 12 }}>Clinical notes</label>
        <textarea rows={2} value={notes} onChange={(e) => setNotes(e.target.value)} />
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={save} disabled={saving || !session}>{saving ? "Saving…" : "Add Treatment"}</button>
        </div>
      </div>
    </div>
  );
}

const LAB: React.CSSProperties = { display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-dim)", marginBottom: 5, textTransform: "uppercase", letterSpacing: 0.5 };

const PD_STYLE = `
.pd-page { display: flex; flex-direction: column; height: 100%; }
.back-btn { align-self: flex-start; margin-bottom: 12px; }
.pd-header { display: flex; align-items: center; gap: 20px; margin-bottom: 16px; flex-shrink: 0; }
.pd-avatar { width: 64px; height: 64px; border-radius: 50%; background: var(--accent); color: white; display: flex; align-items: center; justify-content: center; font-size: 24px; font-weight: 700; flex-shrink: 0; }
.pd-id { flex: 1; }
.pd-id h1 { font-size: 24px; }
.pd-meta { color: var(--text-dim); margin-top: 4px; font-size: 14px; }
.pd-stats { display: flex; gap: 18px; margin-top: 10px; font-size: 13px; color: var(--text-dim); flex-wrap: wrap; }
.pd-stats strong { color: var(--text); }
.pd-stats .out { color: var(--red); }
.allergy-flag { color: var(--amber); }
.muted { color: var(--text-dim); }
.pd-actions { display: flex; flex-direction: column; gap: 8px; }
.pd-scroll { overflow-y: auto; padding-right: 4px; padding-bottom: 40px; }
.pd-row-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 16px; }
.card-h { font-size: 13px; font-weight: 600; margin-bottom: 14px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; display: flex; align-items: center; gap: 8px; }
.count-warn { color: var(--amber); }
.ov-row { display: flex; justify-content: space-between; padding: 6px 0; border-bottom: 1px solid var(--border); font-size: 14px; gap: 12px; }
.ov-row:last-child { border-bottom: none; }
.ov-label { color: var(--text-dim); }
.empty-sm { padding: 12px; text-align: center; color: var(--text-dim); font-size: 13px; }
.allergy-row { display: flex; align-items: center; gap: 10px; padding: 7px 0; border-bottom: 1px solid var(--border); }
.allergy-sub { flex: 1; }
.sev { padding: 2px 8px; border-radius: 999px; font-size: 11px; font-weight: 600; }
.sev-mild { background: rgba(74,222,128,0.15); color: var(--green); }
.sev-moderate { background: rgba(251,191,36,0.15); color: var(--amber); }
.sev-severe { background: rgba(248,113,113,0.15); color: var(--red); }
.add-allergy { display: flex; gap: 6px; margin-top: 10px; }
.add-allergy select { width: auto; }
.add-btn { float: right; font-size: 11px; }
.card-h { display: flex; align-items: center; gap: 8px; }
.card-h .add-btn { margin-left: auto; float: none; }
.edit-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.edit-grid .full { grid-column: 1 / -1; }
.edit-actions { grid-column: 1 / -1; display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.modal { width: 460px; max-height: 90vh; overflow-y: auto; }
.modal h2 { margin-bottom: 16px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 16px; }
.osdi-row { display: flex; align-items: center; gap: 14px; padding: 10px 0; border-bottom: 1px solid var(--border); }
.osdi-big { font-size: 24px; font-weight: 700; width: 56px; }
.osdi-lab { font-weight: 600; font-size: 14px; }
.osdi-sub { font-size: 12px; color: var(--text-dim); }
.osdi-d { font-size: 12px; color: var(--text-dim); margin-left: auto; }
.ipl-row { display: flex; gap: 12px; padding: 10px 0; border-bottom: 1px solid var(--border); }
.ipl-badge { width: 36px; height: 36px; border-radius: 8px; background: var(--accent-soft); color: var(--accent); display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 13px; flex-shrink: 0; }
.ipl-date { font-weight: 600; font-size: 14px; }
.ipl-note { font-size: 12px; color: var(--text-dim); margin-top: 3px; font-style: italic; }
.dt { width: 100%; border-collapse: collapse; }
.dt th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-dim); padding: 8px 10px; border-bottom: 1px solid var(--border); }
.dt td { padding: 9px 10px; border-bottom: 1px solid var(--border); font-size: 14px; }
.dt tr:hover td { background: var(--bg-elev-2); }
.appt-history { display: flex; flex-direction: column; gap: 6px; }
.appt-h-row { border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
.appt-h-row.open { border-color: var(--accent); }
.appt-h-head { display: flex; align-items: center; gap: 12px; padding: 10px 14px; cursor: pointer; font-size: 13px; flex-wrap: wrap; }
.appt-h-head:hover { background: var(--bg-elev-2); }
.appt-h-date { font-weight: 600; min-width: 140px; }
.appt-h-type { flex: 1; }
.appt-has-note { font-size: 14px; }
.appt-h-chev { color: var(--text-dim); margin-left: auto; }
.appt-h-body { padding: 14px; border-top: 1px solid var(--border); background: var(--bg-elev-2); }
.appt-h-detail { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 16px; font-size: 13px; margin-bottom: 14px; }
.appt-h-notes-section { margin-bottom: 12px; }
.appt-h-notes-label { font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; margin-bottom: 6px; }
.appt-h-note-text { font-size: 14px; line-height: 1.5; padding: 10px; background: var(--bg-elev); border-radius: 6px; margin-bottom: 8px; white-space: pre-wrap; }
.appt-h-edit { width: 100%; font-family: inherit; font-size: 13px; padding: 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-elev); color: var(--text); resize: vertical; margin-bottom: 6px; }
.appt-h-edit-actions { display: flex; gap: 6px; }
.appt-h-actions { display: flex; gap: 6px; }
.mono { font-family: ui-monospace, monospace; font-size: 12px; }
.mini { background: var(--bg-elev-2); color: var(--text); border: 1px solid var(--border); padding: 4px 10px; border-radius: 6px; font-size: 12px; }
.mini.danger { color: var(--red); }
.out { color: var(--red); font-weight: 600; }
.note-add-row { display: flex; gap: 8px; margin-bottom: 14px; align-items: flex-start; }
.note-add-input { flex: 1; font-family: inherit; font-size: 14px; padding: 8px 10px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-elev); color: var(--text); resize: none; overflow: hidden; line-height: 1.4; min-height: 38px; }
/* Collapsible section headers */
.card-h.collapsible { cursor: pointer; user-select: none; }
.card-h.collapsible:hover .collapse-title { color: var(--text); }
.collapse-chev { color: var(--text-dim); font-size: 12px; width: 12px; flex-shrink: 0; transition: color 0.15s; }
.card-h.collapsible:hover .collapse-chev { color: var(--accent); }
.collapse-title { display: inline-flex; align-items: center; gap: 6px; }
.collapse-extra { margin-left: auto; display: inline-flex; gap: 6px; }
.note-item { padding: 12px 0; border-bottom: 1px solid var(--border); }
.note-head { display: flex; align-items: center; gap: 10px; margin-bottom: 6px; }
.note-author { font-weight: 600; font-size: 13px; }
.note-date { color: var(--text-dim); font-size: 12px; flex: 1; }
.note-text { font-size: 14px; line-height: 1.5; }
section.card { margin-bottom: 16px; }
`;