import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { messages as msgApi, patients as patApi, type Message, type Patient } from "../api";

const CHANNEL_META: Record<string, { icon: string; color: string; label: string }> = {
  email: { icon: "✉️", color: "var(--accent)", label: "Email" },
  whatsapp: { icon: "💬", color: "var(--green)", label: "WhatsApp" },
  website: { icon: "🌐", color: "var(--amber)", label: "Website" },
  sms: { icon: "📱", color: "var(--text-dim)", label: "SMS" },
};

export function MessagesPage() {
  const nav = useNavigate();
  const [list, setList] = useState<Message[]>([]);
  const [loading, setLoading] = useState(true);
  const [channel, setChannel] = useState<string>("");
  const [selected, setSelected] = useState<Message | null>(null);
  const [linking, setLinking] = useState<number | null>(null);

  const load = () => {
    setLoading(true);
    msgApi.list(channel ? { channel } : {}).then((r) => setList(r.data)).finally(() => setLoading(false));
  };
  useEffect(load, [channel]);

  const unread = list.filter((m) => m.status === "unread").length;
  const channels = ["", "email", "whatsapp", "website"];

  const open = async (m: Message) => {
    setSelected(m);
    if (m.status === "unread") { await msgApi.markRead(m.id); load(); }
  };

  return (
    <div className="msg-page">
      <div className="msg-toolbar">
        <div>
          <h1 className="page-title">📬 Messages</h1>
          <p className="page-sub">{unread} unread · {list.length} total</p>
        </div>
        <div className="chan-tabs">
          {channels.map((c) => (
            <button key={c || "all"} className={channel === c ? "ctab on" : "ctab"} onClick={() => setChannel(c)}>
              {c ? `${CHANNEL_META[c]?.icon || "·"} ${CHANNEL_META[c]?.label || c}` : "All"}
            </button>
          ))}
        </div>
      </div>

      <div className="msg-layout">
        <div className="msg-list">
          {loading ? <div className="card empty">Loading…</div> :
            list.length === 0 ? <div className="card empty">No messages</div> :
            list.map((m) => {
              const cm = CHANNEL_META[m.channel] || { icon: "·", color: "var(--text-dim)" };
              return (
                <div key={m.id} className={`msg-row ${m.status === "unread" ? "unread" : ""} ${selected?.id === m.id ? "sel" : ""}`} onClick={() => open(m)}>
                  <div className="msg-chan" style={{ color: cm.color }}>{cm.icon}</div>
                  <div className="msg-main">
                    <div className="msg-from">
                      {m.status === "unread" && <span className="dot" />}
                      <strong>{m.from_name || m.from_contact || "Unknown"}</strong>
                    </div>
                    <div className="msg-subj">{m.subject || m.body.slice(0, 60) + "…"}</div>
                  </div>
                  <div className="msg-time">{timeAgo(m.received_at)}</div>
                </div>
              );
            })}
        </div>

        {selected ? (
          <aside className="msg-detail card">
            <div className="md-head">
              <div className={`md-chan ${selected.channel}`} style={{ color: CHANNEL_META[selected.channel]?.color }}>
                {CHANNEL_META[selected.channel]?.icon} {CHANNEL_META[selected.channel]?.label}
              </div>
              <button className="sp-close" onClick={() => setSelected(null)}>×</button>
            </div>
            <h2>{selected.subject || "(no subject)"}</h2>
            <div className="md-from">
              <strong>{selected.from_name || "Unknown"}</strong>
              {selected.from_contact && <span className="muted"> · {selected.from_contact}</span>}
            </div>
            <div className="md-date muted">{new Date(selected.received_at).toLocaleString()}</div>
            <div className="md-body">{selected.body}</div>

            <div className="md-actions">
              {selected.linked_patient_id ? (
                <button className="btn-primary" onClick={() => nav(`/patients/${selected.linked_patient_id}`)}>Open Patient File →</button>
              ) : (
                <button className="btn-ghost" onClick={() => setLinking(selected.id)}>🔗 Link to Patient</button>
              )}
              <button className="btn-ghost" onClick={async () => { await msgApi.archive(selected.id); setSelected(null); load(); }}>Archive</button>
            </div>

            <div className="md-reply">
              <div className="md-reply-label">Quick reply ({selected.channel})</div>
              <textarea rows={3} placeholder={`Reply via ${selected.channel}…`} />
              <button className="btn-primary" disabled>Send (coming with Phase 3)</button>
            </div>
          </aside>
        ) : (
          <aside className="msg-detail card empty-side">
            <div className="empty">Select a message to read</div>
          </aside>
        )}
      </div>

      <style>{`
        .msg-page { display: flex; flex-direction: column; height: 100%; }
        .msg-toolbar { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px; flex-wrap: wrap; gap: 12px; }
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; }
        .chan-tabs { display: flex; gap: 4px; }
        .ctab { background: var(--bg-elev); color: var(--text-dim); border: 1px solid var(--border); padding: 6px 14px; border-radius: 8px; font-size: 13px; }
        .ctab.on { background: var(--accent); color: white; border-color: var(--accent); }
        .msg-layout { display: flex; gap: 16px; flex: 1; min-height: 0; }
        .msg-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
        .empty { padding: 40px; text-align: center; color: var(--text-dim); }
        .msg-row { display: flex; align-items: center; gap: 12px; padding: 12px 14px; background: var(--bg-elev); border: 1px solid var(--border); border-radius: 10px; cursor: pointer; }
        .msg-row:hover { border-color: var(--accent); }
        .msg-row.unread { border-left: 3px solid var(--accent); }
        .msg-row.sel { background: var(--accent-soft); border-color: var(--accent); }
        .msg-chan { font-size: 20px; }
        .msg-main { flex: 1; min-width: 0; }
        .msg-from { display: flex; align-items: center; gap: 6px; font-size: 14px; }
        .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
        .msg-subj { font-size: 13px; color: var(--text-dim); margin-top: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .msg-time { font-size: 11px; color: var(--text-dim); }
        .msg-detail { width: 380px; flex-shrink: 0; overflow-y: auto; padding: 20px; }
        .msg-detail.empty-side { display: flex; align-items: center; justify-content: center; }
        .md-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
        .md-chan { font-size: 12px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; }
        .sp-close { background: none; color: var(--text-dim); font-size: 22px; padding: 0 8px; }
        .sp-close:hover { color: var(--text); }
        .msg-detail h2 { font-size: 18px; margin-bottom: 8px; }
        .md-from { font-size: 14px; }
        .muted { color: var(--text-dim); }
        .md-date { font-size: 12px; margin-top: 2px; margin-bottom: 16px; }
        .md-body { font-size: 14px; line-height: 1.6; padding: 14px; background: var(--bg-elev-2); border-radius: 8px; white-space: pre-wrap; }
        .md-actions { display: flex; gap: 8px; margin-top: 16px; }
        .md-reply { margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--border); }
        .md-reply-label { font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; margin-bottom: 8px; }
        .md-reply textarea { margin-bottom: 8px; }
      `}</style>
      {linking !== null && <LinkPatientModal msgId={linking} onClose={() => setLinking(null)} onLinked={() => { setLinking(null); setSelected(null); load(); }} />}
    </div>
  );
}

function LinkPatientModal({ msgId, onClose, onLinked }: { msgId: number; onClose: () => void; onLinked: () => void }) {
  const [search, setSearch] = useState("");
  const [results, setResults] = useState<Patient[]>([]);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    if (!search.trim()) { setResults([]); return; }
    setLoading(true);
    const t = setTimeout(() => {
      patApi.list(search).then((r) => setResults(r.data.patients.slice(0, 8))).finally(() => setLoading(false));
    }, 250);
    return () => clearTimeout(t);
  }, [search]);
  const link = async (pid: number) => { await msgApi.linkPatient(msgId, pid); onLinked(); };
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 420 }}>
        <h2>🔗 Link to Patient</h2>
        <input placeholder="🔍 Search patient by name, MRN, phone…" value={search} onChange={(e) => setSearch(e.target.value)} autoFocus style={{ marginBottom: 12 }} />
        {loading && <div className="muted">Searching…</div>}
        {results.map((p) => (
          <div key={p.id} className="link-pick" onClick={() => link(p.id)}>
            <strong>{p.first_name} {p.last_name}</strong> <span className="mono">{p.mrn}</span>
            <div className="muted">{p.phone} {p.email ? `· ${p.email}` : ""}</div>
          </div>
        ))}
        {search && !loading && results.length === 0 && <div className="muted">No matches.</div>}
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
        </div>
        <style>{`
          .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
          .modal { width: 460px; max-height: 90vh; overflow-y: auto; }
          .modal h2 { margin-bottom: 16px; }
          .link-pick { padding: 10px; border: 1px solid var(--border); border-radius: 8px; margin-bottom: 6px; cursor: pointer; font-size: 14px; }
          .link-pick:hover { background: var(--bg-elev-2); border-color: var(--accent); }
          .mono { font-family: ui-monospace, monospace; font-size: 11px; color: var(--text-dim); }
          .muted { color: var(--text-dim); font-size: 12px; }
          .modal-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 16px; }
        `}</style>
      </div>
    </div>
  );
}

function timeAgo(s: string) {
  const d = new Date(s);
  const mins = Math.floor((Date.now() - d.getTime()) / 60000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  const days = Math.floor(hrs / 24);
  if (days < 7) return `${days}d`;
  return d.toLocaleDateString();
}
