import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  appointments, analytics, type Appointment, type AnalyticsOverview,
} from "../api";

export function Dashboard() {
  const nav = useNavigate();
  const [today, setToday] = useState<Appointment[]>([]);
  const [ov, setOv] = useState<AnalyticsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    Promise.all([appointments.today(), analytics.overview()])
      .then(([a, o]) => { setToday(a.data.appointments); setOv(o.data); })
      .finally(() => setLoading(false));
  }, []);

  const sorted = [...today].sort((a, b) => a.appointment_date.localeCompare(b.appointment_date));
  const next = sorted.find((a) => new Date(a.appointment_date).getTime() > now && a.status !== "cancelled");

  const stats = [
    { label: "Today's Appointments", value: today.length, icon: "📅", color: "var(--accent)", to: "/calendar" },
    { label: "Total Patients", value: ov?.total_patients ?? "—", icon: "👥", color: "var(--green)", to: "/patients" },
    { label: "Revenue (all time)", value: ov ? `$${ov.total_revenue.toLocaleString()}` : "—", icon: "💰", color: "var(--amber)", to: "/analytics" },
    { label: "Outstanding", value: ov ? `$${ov.outstanding_balance.toLocaleString()}` : "—", icon: "⏳", color: "var(--red)", to: "/analytics" },
  ];

  return (
    <div>
      <h1 className="page-title">Dashboard</h1>
      <p className="page-sub">Welcome back — here's what's happening today.</p>

      {/* Next patient countdown */}
      {next && <NextPatient appt={next} now={now} onClick={() => nav(`/patients/${next.patient_id}`)} />}

      <div className="stat-grid">
        {stats.map((s) => (
          <div key={s.label} className="card stat-card clickable" onClick={() => nav(s.to)}>
            <div className="stat-icon" style={{ background: s.color + "22", color: s.color }}>{s.icon}</div>
            <div>
              <div className="stat-value">{loading ? "—" : s.value}</div>
              <div className="stat-label">{s.label}</div>
            </div>
            <div className="stat-go">›</div>
          </div>
        ))}
      </div>

      <div className="card clickable" style={{ marginTop: 20 }} onClick={() => nav("/calendar")}>
        <div className="section-head">
          <h2 className="section-title">Today's Schedule</h2>
          <span className="see-all">View calendar ›</span>
        </div>
        {loading ? <div className="empty">Loading…</div> :
         today.length === 0 ? <div className="empty">No appointments scheduled for today 🎉</div> :
          <div className="appt-list">
            {sorted.map((a) => {
              const t = new Date(a.appointment_date).getTime();
              const isNext = next && a.id === next.id;
              const past = t < now;
              return (
                <div key={a.id} className={`appt-row ${isNext ? "is-next" : ""} ${past ? "past" : ""}`} onClick={(e) => { e.stopPropagation(); nav(`/patients/${a.patient_id}`); }}>
                  <div className="appt-time">{new Date(a.appointment_date).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })}</div>
                  <div className="appt-main">
                    <div className="appt-name">{a.first_name} {a.last_name}</div>
                    <div className="appt-type">{a.appointment_type} · {a.duration_minutes}min {a.practitioner ? `· ${a.practitioner}` : ""}</div>
                  </div>
                  {isNext && <span className="next-tag">NEXT</span>}
                  <span className={`badge badge-${a.status}`}>{a.status}</span>
                </div>
              );
            })}
          </div>}
      </div>

      <div className="quick-grid">
        <div className="card quick clickable" onClick={() => nav("/analytics")}>
          <div className="quick-icon">📊</div>
          <div className="quick-label">View Analytics</div>
          <div className="quick-sub">Revenue, traffic, trends</div>
        </div>
        <div className="card quick clickable" onClick={() => nav("/calendar")}>
          <div className="quick-icon">📅</div>
          <div className="quick-label">Open Calendar</div>
          <div className="quick-sub">Book & block times</div>
        </div>
        <div className="card quick clickable" onClick={() => nav("/patients")}>
          <div className="quick-icon">👥</div>
          <div className="quick-label">Find Patient</div>
          <div className="quick-sub">Records & history</div>
        </div>
      </div>

      <style>{`
        .page-title { font-size: 28px; font-weight: 700; }
        .page-sub { color: var(--text-dim); margin-top: 4px; margin-bottom: 24px; }
        .stat-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; }
        .stat-card { display: flex; align-items: center; gap: 16px; position: relative; }
        .stat-card.clickable:hover { border-color: var(--accent); transform: translateY(-1px); }
        .stat-icon { width: 48px; height: 48px; border-radius: 12px; display: flex; align-items: center; justify-content: center; font-size: 22px; }
        .stat-value { font-size: 26px; font-weight: 700; }
        .stat-label { font-size: 13px; color: var(--text-dim); }
        .stat-go { position: absolute; right: 16px; color: var(--text-dim); font-size: 22px; }
        .section-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; }
        .section-title { font-size: 17px; font-weight: 600; }
        .see-all { color: var(--accent); font-size: 13px; }
        .empty { padding: 32px; text-align: center; color: var(--text-dim); }
        .appt-list { display: flex; flex-direction: column; }
        .appt-row { display: flex; align-items: center; gap: 16px; padding: 12px 8px; border-bottom: 1px solid var(--border); border-radius: 6px; cursor: pointer; }
        .appt-row:hover { background: var(--bg-elev-2); }
        .appt-row.is-next { background: var(--accent-soft); }
        .appt-row.past { opacity: 0.5; }
        .appt-time { font-weight: 700; font-size: 15px; width: 70px; color: var(--accent); }
        .appt-main { flex: 1; }
        .appt-name { font-weight: 600; }
        .appt-type { font-size: 13px; color: var(--text-dim); margin-top: 2px; }
        .next-tag { background: var(--accent); color: white; padding: 2px 8px; border-radius: 999px; font-size: 10px; font-weight: 700; }
        .quick-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-top: 20px; }
        .quick { text-align: center; padding: 24px; }
        .quick.clickable:hover { border-color: var(--accent); }
        .quick-icon { font-size: 32px; }
        .quick-label { font-weight: 600; margin-top: 8px; }
        .quick-sub { font-size: 12px; color: var(--text-dim); margin-top: 2px; }
      `}</style>
    </div>
  );
}

function NextPatient({ appt, now, onClick }: { appt: Appointment; now: number; onClick: () => void }) {
  const target = new Date(appt.appointment_date).getTime();
  const diff = Math.max(0, target - now);
  const h = Math.floor(diff / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  const s = Math.floor((diff % 60000) / 1000);
  const soon = diff < 600000; // < 10 min

  return (
    <div className={`next-card ${soon ? "soon" : ""}`} onClick={onClick}>
      <div className="next-left">
        <div className="next-eyebrow">⏰ NEXT PATIENT {soon ? "— SOON" : ""}</div>
        <div className="next-name">{appt.first_name} {appt.last_name}</div>
        <div className="next-type">{appt.appointment_type} · {new Date(appt.appointment_date).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })}</div>
      </div>
      <div className="next-clock">
        <div className="clock-time">{String(h).padStart(2, "0")}:{String(m).padStart(2, "0")}:{String(s).padStart(2, "0")}</div>
        <div className="clock-label">until arrival</div>
      </div>
      <style>{`
        .next-card { display: flex; align-items: center; justify-content: space-between; background: linear-gradient(135deg, var(--accent-soft), transparent); border: 1px solid var(--accent); border-radius: 14px; padding: 20px 24px; margin-bottom: 24px; cursor: pointer; transition: transform 0.15s; }
        .next-card:hover { transform: translateY(-2px); }
        .next-card.soon { border-color: var(--amber); background: linear-gradient(135deg, rgba(251,191,36,0.15), transparent); animation: pulse 2s infinite; }
        @keyframes pulse { 0%,100% { box-shadow: 0 0 0 0 rgba(251,191,36,0.3); } 50% { box-shadow: 0 0 0 8px rgba(251,191,36,0); } }
        .next-eyebrow { font-size: 11px; font-weight: 700; color: var(--accent); letter-spacing: 1px; }
        .next-card.soon .next-eyebrow { color: var(--amber); }
        .next-name { font-size: 22px; font-weight: 700; margin-top: 4px; }
        .next-type { font-size: 14px; color: var(--text-dim); margin-top: 2px; }
        .next-clock { text-align: right; }
        .clock-time { font-size: 32px; font-weight: 700; font-variant-numeric: tabular-nums; color: var(--accent); font-family: ui-monospace, monospace; }
        .next-card.soon .clock-time { color: var(--amber); }
        .clock-label { font-size: 11px; color: var(--text-dim); }
      `}</style>
    </div>
  );
}
