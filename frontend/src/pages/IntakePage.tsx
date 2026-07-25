import { useEffect, useRef, useState } from "react";
import {
  intake,
  bookingSettings,
  type IntakeSubmission,
  type BookingSettings,
  type BookingNotification,
} from "../api";

const EMPTY_SETTINGS: BookingSettings = {
  booking_mode: "approval",
  auto_confirm_message: true,
  auto_reminder_message: true,
  reminder_hours_before: 24,
  email_provider: "",
  email_from: "",
  sms_provider: "",
  sms_sender: "",
  template_booking_received: "",
  template_booking_confirmed: "",
  template_booking_declined: "",
  template_reminder: "",
};

const EMAIL_PROVIDERS = ["", "smtp", "sendgrid", "mailgun", "ses", "postmark"];
const SMS_PROVIDERS = ["", "twilio", "messagemedia", "clicksend", "vonage"];

export function IntakePage() {
  const [list, setList] = useState<IntakeSubmission[]>([]);
  const [notifications, setNotifications] = useState<BookingNotification[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<"new" | "all">("new");

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<BookingSettings>(EMPTY_SETTINGS);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);

  const timerRef = useRef<number | null>(null);

  const loadSubmissions = () => {
    intake
      .list()
      .then((r) => setList(r.data))
      .catch(() => {})
      .finally(() => setLoading(false));
  };

  const loadNotifications = () => {
    bookingSettings
      .notifications()
      .then((r) => setNotifications(r.data))
      .catch(() => {});
  };

  const loadSettings = () => {
    bookingSettings
      .get()
      .then((r) => {
        setSettings({ ...EMPTY_SETTINGS, ...r.data });
        setSettingsLoaded(true);
      })
      .catch(() => {});
  };

  const refreshLive = () => {
    loadSubmissions();
    loadNotifications();
  };

  useEffect(() => {
    setLoading(true);
    loadSubmissions();
    loadNotifications();
    loadSettings();
    timerRef.current = window.setInterval(refreshLive, 15000);
    return () => {
      if (timerRef.current !== null) window.clearInterval(timerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const shown = filter === "new" ? list.filter((s) => s.status === "new") : list;
  const newCount = list.filter((s) => s.status === "new").length;

  const doImport = async (id: number) => { await intake.import(id); refreshLive(); };
  const doArchive = async (id: number) => { await intake.archive(id); refreshLive(); };
  const doApprove = async (id: number) => { await bookingSettings.approve(id); refreshLive(); };
  const doDecline = async (id: number) => { await bookingSettings.decline(id); refreshLive(); };

  const setField = <K extends keyof BookingSettings>(k: K, v: BookingSettings[K]) =>
    setSettings((s) => ({ ...s, [k]: v }));

  const saveSettings = async () => {
    setSaving(true);
    try {
      const r = await bookingSettings.update(settings);
      setSettings({ ...EMPTY_SETTINGS, ...r.data });
      setSavedFlash(true);
      window.setTimeout(() => setSavedFlash(false), 2200);
    } catch {
      alert("Failed to save booking settings.");
    } finally {
      setSaving(false);
    }
  };

  const channelIcon = (ch: string) => (ch === "sms" || ch === "phone" ? "📱" : "✉️");
  const statusClass = (st: string) => {
    switch (st) {
      case "sent": return "ns-sent";
      case "failed": return "ns-failed";
      case "skipped": return "ns-skipped";
      default: return "ns-pending";
    }
  };

  return (
    <div>
      <div className="header-row">
        <div>
          <h1 className="page-title">📥 Intake &amp; Bookings</h1>
          <p className="page-sub">{newCount} new · {list.length} total · from the <a href="/input" target="_blank">input page</a> · live-refreshing every 15s</p>
        </div>
        <div className="filter-tabs">
          <button className={filter === "new" ? "ftab on" : "ftab"} onClick={() => setFilter("new")}>New ({newCount})</button>
          <button className={filter === "all" ? "ftab on" : "ftab"} onClick={() => setFilter("all")}>All</button>
          {newCount > 0 && <button className="btn-primary" onClick={async () => { const r = await intake.autoImport(); alert(`Imported ${r.data.imported} of ${r.data.total_new} submissions`); refreshLive(); }}>⚡ Auto-import all ({newCount})</button>}
        </div>
      </div>

      {/* ---------- Settings panel ---------- */}
      <div className="card settings-panel">
        <button className="sp-toggle" onClick={() => setSettingsOpen((o) => !o)}>
          <span className="sp-chev">{settingsOpen ? "▾" : "▸"}</span>
          <span className="sp-title">⚙️ Booking Settings</span>
          <span className="sp-mode-pill">{settings.booking_mode === "automatic" ? "Automatic" : "Approval"}</span>
          <span className="sp-hint">{settingsOpen ? "" : "click to expand"}</span>
        </button>

        {settingsOpen && (
          <div className="sp-body">
            {/* Booking mode */}
            <div className="sp-section">
              <div className="sp-label">Booking Mode</div>
              <div className="mode-grid">
                <button
                  className={`mode-btn ${settings.booking_mode === "automatic" ? "on" : ""}`}
                  onClick={() => setField("booking_mode", "automatic")}
                >
                  <div className="mode-btn-title">⚡ Automatic</div>
                  <div className="mode-btn-sub">Auto-confirm if the slot is free</div>
                </button>
                <button
                  className={`mode-btn ${settings.booking_mode === "approval" ? "on" : ""}`}
                  onClick={() => setField("booking_mode", "approval")}
                >
                  <div className="mode-btn-title">✔️ Approval</div>
                  <div className="mode-btn-sub">Staff must approve each booking</div>
                </button>
              </div>
            </div>

            {/* Auto messages */}
            <div className="sp-section">
              <div className="sp-label">Automatic Messages</div>
              <label className="toggle-row">
                <input
                  type="checkbox"
                  checked={settings.auto_confirm_message}
                  onChange={(e) => setField("auto_confirm_message", e.target.checked)}
                />
                <span>Send confirmation when a booking is received</span>
              </label>
              <label className="toggle-row">
                <input
                  type="checkbox"
                  checked={settings.auto_reminder_message}
                  onChange={(e) => setField("auto_reminder_message", e.target.checked)}
                />
                <span>Send a reminder before the appointment</span>
                <span className="reminder-inline">
                  <input
                    type="number"
                    min={1}
                    max={168}
                    value={settings.reminder_hours_before}
                    onChange={(e) => setField("reminder_hours_before", Number(e.target.value) || 0)}
                    disabled={!settings.auto_reminder_message}
                  />
                  <span className="ri-unit">hours before</span>
                </span>
              </label>
            </div>

            {/* Providers */}
            <div className="sp-section">
              <div className="sp-label">Delivery Providers</div>
              <div className="prov-grid">
                <div className="prov-col">
                  <div className="prov-head">
                    ✉️ Email
                    <span className={`prov-status ${settings.email_provider && settings.email_from ? "ok" : "off"}`}>
                      {settings.email_provider && settings.email_from ? "configured" : "not configured"}
                    </span>
                  </div>
                  <label className="fld">
                    <span>Provider</span>
                    <select value={settings.email_provider} onChange={(e) => setField("email_provider", e.target.value)}>
                      {EMAIL_PROVIDERS.map((p) => (
                        <option key={p || "none"} value={p}>{p ? p : "— none —"}</option>
                      ))}
                    </select>
                  </label>
                  <label className="fld">
                    <span>From address</span>
                    <input
                      type="text"
                      placeholder="clinic@example.com"
                      value={settings.email_from}
                      onChange={(e) => setField("email_from", e.target.value)}
                    />
                  </label>
                </div>
                <div className="prov-col">
                  <div className="prov-head">
                    📱 SMS
                    <span className={`prov-status ${settings.sms_provider && settings.sms_sender ? "ok" : "off"}`}>
                      {settings.sms_provider && settings.sms_sender ? "configured" : "not configured"}
                    </span>
                  </div>
                  <label className="fld">
                    <span>Provider</span>
                    <select value={settings.sms_provider} onChange={(e) => setField("sms_provider", e.target.value)}>
                      {SMS_PROVIDERS.map((p) => (
                        <option key={p || "none"} value={p}>{p ? p : "— none —"}</option>
                      ))}
                    </select>
                  </label>
                  <label className="fld">
                    <span>Sender ID</span>
                    <input
                      type="text"
                      placeholder="OptiCore"
                      value={settings.sms_sender}
                      onChange={(e) => setField("sms_sender", e.target.value)}
                    />
                  </label>
                </div>
              </div>
            </div>

            {/* Templates */}
            <div className="sp-section">
              <div className="sp-label">Message Templates</div>
              <div className="tpl-note">
                Placeholders you can use: <code>{"{{name}}"}</code> <code>{"{{date}}"}</code> <code>{"{{time}}"}</code> <code>{"{{type}}"}</code>
              </div>
              <div className="tpl-grid">
                <label className="tpl">
                  <span>Booking received</span>
                  <textarea
                    rows={3}
                    value={settings.template_booking_received}
                    onChange={(e) => setField("template_booking_received", e.target.value)}
                    placeholder="Hi {{name}}, we received your booking request for {{date}} {{time}}…"
                  />
                </label>
                <label className="tpl">
                  <span>Booking confirmed</span>
                  <textarea
                    rows={3}
                    value={settings.template_booking_confirmed}
                    onChange={(e) => setField("template_booking_confirmed", e.target.value)}
                    placeholder="Hi {{name}}, your {{type}} on {{date}} at {{time}} is confirmed…"
                  />
                </label>
                <label className="tpl">
                  <span>Booking declined</span>
                  <textarea
                    rows={3}
                    value={settings.template_booking_declined}
                    onChange={(e) => setField("template_booking_declined", e.target.value)}
                    placeholder="Hi {{name}}, unfortunately we can't confirm {{date}} {{time}}…"
                  />
                </label>
                <label className="tpl">
                  <span>Reminder</span>
                  <textarea
                    rows={3}
                    value={settings.template_reminder}
                    onChange={(e) => setField("template_reminder", e.target.value)}
                    placeholder="Reminder: {{name}}, your {{type}} is on {{date}} at {{time}}…"
                  />
                </label>
              </div>
            </div>

            <div className="sp-footer">
              {!settingsLoaded && <span className="sp-warn">⚠️ Could not load saved settings — saving will create them.</span>}
              {savedFlash && <span className="sp-saved">✓ Saved</span>}
              <button className="btn-primary" onClick={saveSettings} disabled={saving}>
                {saving ? "Saving…" : "💾 Save settings"}
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="card info-banner">
        💡 The public input page is at <a href="/input" target="_blank"><strong>http://localhost:3000/input</strong></a>. Anyone can fill it in (no login). Submissions appear here for staff to review, approve, and import as patients.
      </div>

      {/* ---------- Submissions ---------- */}
      {loading ? <div className="card empty">Loading…</div> :
        shown.length === 0 ? <div className="card empty">No submissions.</div> :
        <div className="intake-list">
          {shown.map((s) => (
            <div key={s.id} className={`card intake-card ${s.status}`}>
              <div className="ic-head">
                <div className="ic-name">{s.first_name} {s.last_name}</div>
                {s.status === "new" && <span className="new-badge">New!</span>}
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
                  <button className="btn-approve" onClick={() => doApprove(s.id)}>✅ Approve</button>
                  <button className="btn-decline" onClick={() => doDecline(s.id)}>✕ Decline</button>
                  <button className="btn-primary" onClick={() => doImport(s.id)}>➕ Import as Patient</button>
                  <button className="btn-ghost" onClick={() => doArchive(s.id)}>Archive</button>
                </div>
              )}
              {s.status === "imported" && s.matched_patient_id && (
                <a className="ic-link" href={`#/patients/${s.matched_patient_id}`}>→ View patient record</a>
              )}
            </div>
          ))}
        </div>}

      {/* ---------- Live notification feed ---------- */}
      <div className="notif-section">
        <div className="notif-head">
          <h2 className="notif-title">📨 Notification Feed</h2>
          <span className="notif-sub">{notifications.length} recent · live</span>
        </div>
        {notifications.length === 0 ? (
          <div className="card empty">No notifications yet.</div>
        ) : (
          <div className="notif-list">
            {notifications.map((n) => (
              <div key={n.id} className="card notif-card">
                <div className="nc-icon">{channelIcon(n.channel)}</div>
                <div className="nc-body">
                  <div className="nc-top">
                    <span className="nc-recipient">{n.recipient}</span>
                    {n.template_used && <span className="nc-tpl">{n.template_used}</span>}
                    <span className={`ns ${statusClass(n.status)}`}>{n.status}</span>
                    <span className="nc-time">{new Date(n.sent_at || n.created_at).toLocaleString()}</span>
                  </div>
                  <div className="nc-message">{n.body}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <style>{`
        .header-row { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 20px; }
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; }
        .page-sub a { color: var(--accent); }
        .filter-tabs { display: flex; gap: 4px; align-items: center; }
        .ftab { background: var(--bg-elev); color: var(--text-dim); border: 1px solid var(--border); padding: 6px 14px; border-radius: 8px; font-size: 13px; }
        .ftab.on { background: var(--accent); color: white; border-color: var(--accent); }
        .info-banner { margin-bottom: 16px; font-size: 13px; color: var(--text-dim); }
        .info-banner a { color: var(--accent); }
        .empty { padding: 32px; text-align: center; color: var(--text-dim); }

        /* Settings panel */
        .settings-panel { margin-bottom: 16px; padding: 0; overflow: hidden; }
        .sp-toggle { display: flex; align-items: center; gap: 10px; width: 100%; background: transparent; border: none; color: var(--text); padding: 16px 20px; cursor: pointer; text-align: left; }
        .sp-chev { color: var(--text-dim); width: 14px; }
        .sp-title { font-weight: 700; font-size: 16px; }
        .sp-mode-pill { font-size: 11px; text-transform: uppercase; letter-spacing: 0.4px; background: var(--accent-soft); color: var(--accent); padding: 3px 10px; border-radius: 999px; }
        .sp-hint { margin-left: auto; color: var(--text-dim); font-size: 12px; }
        .sp-body { border-top: 1px solid var(--border); padding: 20px; display: flex; flex-direction: column; gap: 22px; }
        .sp-section { display: flex; flex-direction: column; gap: 10px; }
        .sp-label { font-size: 12px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-dim); font-weight: 700; }

        .mode-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
        .mode-btn { text-align: left; background: var(--bg-elev-2); border: 2px solid var(--border); border-radius: 12px; padding: 16px; cursor: pointer; color: var(--text); transition: border-color .12s, background .12s; }
        .mode-btn:hover { border-color: var(--accent); }
        .mode-btn.on { border-color: var(--accent); background: var(--accent-soft); }
        .mode-btn-title { font-weight: 700; font-size: 15px; }
        .mode-btn-sub { color: var(--text-dim); font-size: 12px; margin-top: 4px; }

        .toggle-row { display: flex; align-items: center; gap: 10px; font-size: 14px; }
        .toggle-row input[type=checkbox] { width: 16px; height: 16px; accent-color: var(--accent); }
        .reminder-inline { display: inline-flex; align-items: center; gap: 6px; margin-left: 6px; }
        .reminder-inline input { width: 64px; background: var(--bg); border: 1px solid var(--border); color: var(--text); border-radius: 6px; padding: 4px 8px; font-size: 13px; }
        .reminder-inline input:disabled { opacity: 0.4; }
        .ri-unit { color: var(--text-dim); font-size: 12px; }

        .prov-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
        .prov-col { background: var(--bg-elev-2); border: 1px solid var(--border); border-radius: 12px; padding: 14px; display: flex; flex-direction: column; gap: 10px; }
        .prov-head { display: flex; align-items: center; gap: 8px; font-weight: 700; font-size: 14px; }
        .prov-status { font-size: 11px; padding: 2px 8px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.4px; margin-left: auto; }
        .prov-status.ok { background: rgba(74,222,128,0.15); color: var(--green); }
        .prov-status.off { background: rgba(248,113,113,0.12); color: var(--red); }
        .fld { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--text-dim); }
        .fld input, .fld select { background: var(--bg); border: 1px solid var(--border); color: var(--text); border-radius: 6px; padding: 7px 10px; font-size: 13px; }

        .tpl-note { font-size: 12px; color: var(--text-dim); }
        .tpl-note code { background: var(--bg-elev-2); border: 1px solid var(--border); padding: 1px 6px; border-radius: 5px; color: var(--accent); font-size: 11px; }
        .tpl-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
        .tpl { display: flex; flex-direction: column; gap: 5px; font-size: 12px; color: var(--text-dim); font-weight: 600; }
        .tpl textarea { background: var(--bg); border: 1px solid var(--border); color: var(--text); border-radius: 8px; padding: 8px 10px; font-size: 13px; font-family: inherit; resize: vertical; line-height: 1.4; }

        .sp-footer { display: flex; align-items: center; gap: 12px; justify-content: flex-end; }
        .sp-warn { color: var(--amber); font-size: 12px; margin-right: auto; }
        .sp-saved { color: var(--green); font-size: 13px; font-weight: 600; }

        /* Submissions */
        .intake-list { display: flex; flex-direction: column; gap: 12px; }
        .intake-card { padding: 16px 20px; }
        .intake-card.imported, .intake-card.archived, .intake-card.declined { opacity: 0.7; }
        .ic-head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
        .ic-name { font-weight: 700; font-size: 16px; }
        .ic-date { color: var(--text-dim); font-size: 12px; margin-left: auto; }
        .ic-grid { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px 16px; font-size: 13px; }
        .ic-grid div span { color: var(--text-dim); display: block; font-size: 11px; text-transform: uppercase; }
        .ic-symptoms { font-size: 13px; margin-top: 10px; color: var(--text-dim); }
        .ic-symptoms span { color: var(--text); font-weight: 600; }
        .ic-actions { display: flex; gap: 8px; margin-top: 14px; flex-wrap: wrap; }
        .ic-link { display: inline-block; margin-top: 10px; color: var(--accent); font-size: 13px; }

        .new-badge { background: var(--green); color: #08130c; font-size: 11px; font-weight: 800; text-transform: uppercase; letter-spacing: 0.5px; padding: 3px 9px; border-radius: 999px; animation: pulse-badge 1.4s ease-in-out infinite; }
        @keyframes pulse-badge {
          0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(74,222,128,0.5); }
          50% { transform: scale(1.08); box-shadow: 0 0 0 6px rgba(74,222,128,0); }
        }

        .btn-approve { background: var(--green); color: #08130c; border: none; padding: 7px 14px; border-radius: 8px; font-size: 13px; font-weight: 700; cursor: pointer; }
        .btn-approve:hover { filter: brightness(1.08); }
        .btn-decline { background: transparent; color: var(--red); border: 1px solid var(--red); padding: 7px 14px; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; }
        .btn-decline:hover { background: rgba(248,113,113,0.12); }

        /* Notification feed */
        .notif-section { margin-top: 28px; }
        .notif-head { display: flex; align-items: baseline; gap: 12px; margin-bottom: 12px; }
        .notif-title { font-size: 20px; font-weight: 700; }
        .notif-sub { color: var(--text-dim); font-size: 13px; }
        .notif-list { display: flex; flex-direction: column; gap: 8px; }
        .notif-card { display: flex; gap: 12px; padding: 12px 16px; align-items: flex-start; }
        .nc-icon { font-size: 18px; line-height: 1.4; }
        .nc-body { flex: 1; min-width: 0; }
        .nc-top { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
        .nc-recipient { font-weight: 600; font-size: 14px; }
        .nc-tpl { font-size: 11px; color: var(--text-dim); background: var(--bg-elev-2); border: 1px solid var(--border); padding: 1px 7px; border-radius: 5px; }
        .nc-time { color: var(--text-dim); font-size: 12px; margin-left: auto; }
        .nc-message { font-size: 13px; color: var(--text-dim); margin-top: 5px; white-space: pre-wrap; word-break: break-word; }
        .ns { font-size: 11px; text-transform: uppercase; letter-spacing: 0.4px; font-weight: 700; padding: 2px 9px; border-radius: 999px; }
        .ns-pending { background: rgba(251,191,36,0.15); color: var(--amber); }
        .ns-sent { background: rgba(74,222,128,0.15); color: var(--green); }
        .ns-failed { background: rgba(248,113,113,0.15); color: var(--red); }
        .ns-skipped { background: var(--bg-elev-2); color: var(--text-dim); }

        @media (max-width: 720px) {
          .mode-grid, .prov-grid, .tpl-grid { grid-template-columns: 1fr; }
          .ic-grid { grid-template-columns: 1fr 1fr; }
        }
      `}</style>
    </div>
  );
}
