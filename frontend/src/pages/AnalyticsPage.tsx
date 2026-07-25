import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  ResponsiveContainer, AreaChart, Area, BarChart, Bar, PieChart, Pie, Cell,
  XAxis, YAxis, Tooltip, CartesianGrid, Legend,
} from "recharts";
import {
  analytics, type AnalyticsOverview, type TimeSeriesPoint, type WebsiteTrafficPoint, type SourceBreakdown,
} from "../api";

const PIE_COLORS = ["#4f9cf9", "#4ade80", "#fbbf24", "#f87171", "#a78bfa"];
type Tab = "overview" | "financial" | "patients" | "website" | "reports";

export function AnalyticsPage() {
  const nav = useNavigate();
  const [tab, setTab] = useState<Tab>("overview");
  const [ov, setOv] = useState<AnalyticsOverview | null>(null);
  const [rev, setRev] = useState<TimeSeriesPoint[]>([]);
  const [appt, setAppt] = useState<TimeSeriesPoint[]>([]);
  const [traffic, setTraffic] = useState<WebsiteTrafficPoint[]>([]);
  const [sources, setSources] = useState<SourceBreakdown[]>([]);
  const [range, setRange] = useState(30);

  useEffect(() => {
    analytics.overview().then((r) => setOv(r.data));
    analytics.revenue(range).then((r) => setRev(r.data));
    analytics.appointments(range).then((r) => setAppt(r.data));
    analytics.traffic(range).then((r) => setTraffic(r.data));
    analytics.trafficBySource().then((r) => setSources(r.data));
  }, [range]);

  const trafficAgg = aggregateTraffic(traffic);
  const fmtDate = (s: string) => s.slice(5);
  const tabs: [Tab, string, string][] = [
    ["overview", "Overview", "📊"],
    ["financial", "Financial", "💰"],
    ["patients", "Patients", "👥"],
    ["website", "Website", "🌐"],
    ["reports", "Reports", "📋"],
  ];

  return (
    <div>
      <div className="header-row">
        <div>
          <h1 className="page-title">Analytics</h1>
          <p className="page-sub">Practice performance & insights</p>
        </div>
        <div className="range-tabs">
          {[7, 30, 90].map((d) => (
            <button key={d} className={range === d ? "range-tab active" : "range-tab"} onClick={() => setRange(d)}>{d}d</button>
          ))}
        </div>
      </div>

      <div className="an-tabs">
        {tabs.map(([t, label, icon]) => (
          <button key={t} className={tab === t ? "an-tab on" : "an-tab"} onClick={() => setTab(t)}>
            {icon} {label}
          </button>
        ))}
      </div>

      {tab === "overview" && <OverviewTab ov={ov} nav={nav} />}
      {tab === "financial" && <FinancialTab ov={ov} rev={rev} fmtDate={fmtDate} />}
      {tab === "patients" && <PatientsTab ov={ov} appt={appt} fmtDate={fmtDate} nav={nav} />}
      {tab === "website" && <WebsiteTab trafficAgg={trafficAgg} sources={sources} />}
      {tab === "reports" && <ReportsTab />}

      <style>{AN_STYLE}</style>
    </div>
  );
}

function OverviewTab({ ov, nav }: any) {
  const stats = [
    { label: "Total Revenue", value: ov ? `$${ov.total_revenue.toLocaleString()}` : "—", sub: ov ? `+${ov.revenue_this_month.toLocaleString()} this month` : "", color: "var(--green)", icon: "$", to: "/analytics" },
    { label: "Outstanding", value: ov ? `$${ov.outstanding_balance.toLocaleString()}` : "—", sub: "awaiting payment", color: "var(--red)", icon: "!" },
    { label: "Patients", value: ov?.total_patients ?? "—", sub: "total records", color: "var(--accent)", icon: "👥", to: "/patients" },
    { label: "Appointments", value: ov?.total_appointments ?? "—", sub: ov ? `${ov.appointments_this_month} this month` : "", color: "var(--amber)", icon: "📅", to: "/calendar" },
    { label: "Avg Appt Value", value: ov ? `$${ov.avg_appt_value.toFixed(0)}` : "—", sub: "per visit", color: "var(--text-dim)", icon: "📊" },
  ];
  return (
    <>
      <div className="kpi-grid">
        {stats.map((s) => (
          <div key={s.label} className={`card kpi ${s.to ? "clickable" : ""}`} onClick={() => s.to && nav(s.to)} style={s.to ? { cursor: "pointer" } : {}}>
            <div className="kpi-icon" style={{ background: s.color + "22", color: s.color }}>{s.icon}</div>
            <div>
              <div className="kpi-val" style={{ color: s.color }}>{s.value}</div>
              <div className="kpi-lab">{s.label}</div>
              <div className="kpi-sub">{s.sub}</div>
            </div>
          </div>
        ))}
      </div>
    </>
  );
}

function FinancialTab({ ov, rev, fmtDate }: any) {
  return (
    <>
      <div className="fin-row">
        <div className="card fin-stat"><div className="fin-v">${ov?.total_revenue.toLocaleString() || "—"}</div><div className="fin-l">Total Revenue</div></div>
        <div className="card fin-stat"><div className="fin-v" style={{ color: "var(--green)" }}>${ov?.revenue_this_month.toLocaleString() || "—"}</div><div className="fin-l">This Month</div></div>
        <div className="card fin-stat"><div className="fin-v" style={{ color: "var(--red)" }}>${ov?.outstanding_balance.toLocaleString() || "—"}</div><div className="fin-l">Outstanding</div></div>
        <div className="card fin-stat"><div className="fin-v">${ov?.avg_appt_value.toFixed(0) || "—"}</div><div className="fin-l">Avg per Visit</div></div>
      </div>
      <div className="card chart-card">
        <h3 className="card-h">Revenue Over Time</h3>
        <ResponsiveContainer width="100%" height={280}>
          <AreaChart data={rev.map((p: TimeSeriesPoint) => ({ date: fmtDate(p.date), Revenue: p.value }))}>
            <defs><linearGradient id="rev" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#4ade80" stopOpacity={0.4} /><stop offset="95%" stopColor="#4ade80" stopOpacity={0} /></linearGradient></defs>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
            <XAxis dataKey="date" stroke="var(--text-dim)" fontSize={11} />
            <YAxis stroke="var(--text-dim)" fontSize={11} />
            <Tooltip contentStyle={tooltipStyle} />
            <Area type="monotone" dataKey="Revenue" stroke="#4ade80" strokeWidth={2} fill="url(#rev)" />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </>
  );
}

function PatientsTab({ ov, appt, fmtDate, nav }: any) {
  return (
    <>
      <div className="fin-row">
        <div className="card fin-stat"><div className="fin-v">{ov?.total_patients ?? "—"}</div><div className="fin-l">Total Patients</div></div>
        <div className="card fin-stat clickable" onClick={() => nav("/patients")}><div className="fin-v" style={{ color: "var(--accent)" }}>View All →</div><div className="fin-l">Patient List</div></div>
      </div>
      <div className="card chart-card">
        <h3 className="card-h">Appointments Over Time</h3>
        <ResponsiveContainer width="100%" height={260}>
          <BarChart data={appt.map((p: TimeSeriesPoint) => ({ date: fmtDate(p.date), Appointments: p.value }))}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
            <XAxis dataKey="date" stroke="var(--text-dim)" fontSize={11} />
            <YAxis allowDecimals={false} stroke="var(--text-dim)" fontSize={11} />
            <Tooltip contentStyle={tooltipStyle} />
            <Bar dataKey="Appointments" fill="#4f9cf9" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </>
  );
}

function WebsiteTab({ trafficAgg, sources }: any) {
  return (
    <>
      <div className="chart-row">
        <div className="card chart-card">
          <h3 className="card-h">Website Visitors & Bookings</h3>
          <ResponsiveContainer width="100%" height={260}>
            <AreaChart data={trafficAgg}>
              <defs>
                <linearGradient id="vis" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#4f9cf9" stopOpacity={0.4} /><stop offset="95%" stopColor="#4f9cf9" stopOpacity={0} /></linearGradient>
                <linearGradient id="book" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#fbbf24" stopOpacity={0.4} /><stop offset="95%" stopColor="#fbbf24" stopOpacity={0} /></linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="date" stroke="var(--text-dim)" fontSize={11} />
              <YAxis allowDecimals={false} stroke="var(--text-dim)" fontSize={11} />
              <Tooltip contentStyle={tooltipStyle} />
              <Legend />
              <Area type="monotone" dataKey="Visitors" stroke="#4f9cf9" strokeWidth={2} fill="url(#vis)" />
              <Area type="monotone" dataKey="Bookings" stroke="#fbbf24" strokeWidth={2} fill="url(#book)" />
            </AreaChart>
          </ResponsiveContainer>
        </div>
        <div className="card chart-card">
          <h3 className="card-h">Traffic by Source</h3>
          <ResponsiveContainer width="100%" height={260}>
            <PieChart>
              <Pie data={sources} dataKey="visitors" nameKey="source" cx="50%" cy="50%" outerRadius={80} label={(e: any) => e.source}>
                {sources.map((_: any, i: number) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
              </Pie>
              <Tooltip contentStyle={tooltipStyle} />
            </PieChart>
          </ResponsiveContainer>
        </div>
      </div>
      <div className="card">
        <h3 className="card-h">Source Breakdown</h3>
        <table className="dt">
          <thead><tr><th>Source</th><th>Visitors</th><th>Bookings</th><th>Conversion</th></tr></thead>
          <tbody>
            {sources.map((s: SourceBreakdown) => (
              <tr key={s.source}><td className="cap">{s.source}</td><td>{s.visitors}</td><td>{s.bookings}</td>
                <td>{s.visitors > 0 ? ((s.bookings / s.visitors) * 100).toFixed(1) : "0"}%</td></tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

function ReportsTab() {
  const reports = [
    { name: "Patient Demographics", desc: "Age, gender, location breakdown of your patient base", icon: "👥" },
    { name: "Financial Summary", desc: "Revenue, tax, outstanding balances, payment methods", icon: "💰" },
    { name: "Treatment Outcomes", desc: "OSDI score progression, IPL response rates", icon: "🔬" },
    { name: "Appointment Analytics", desc: "No-shows, cancellations, peak times, duration", icon: "📅" },
    { name: "Practitioner Productivity", desc: "Per-doctor revenue, volume, outcomes", icon: "🧑‍⚕️" },
    { name: "Inventory Report", desc: "Stock levels, usage, reordering", icon: "📦" },
  ];
  return (
    <>
      <div className="card info-banner">📋 <strong>Reports</strong> are part of Phase 6 (comes last). These will be fully generated, exportable reports. Below is the planned catalog.</div>
      <div className="rep-grid">
        {reports.map((r) => (
          <div key={r.name} className="card rep-card">
            <div className="rep-icon">{r.icon}</div>
            <h3>{r.name}</h3>
            <p>{r.desc}</p>
            <button className="btn-ghost rep-btn" disabled>Coming in Phase 6</button>
          </div>
        ))}
      </div>
    </>
  );
}

const tooltipStyle = { background: "var(--bg-elev-2)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 };
function aggregateTraffic(points: WebsiteTrafficPoint[]) {
  const byDate: Record<string, { visitors: number; bookings: number }> = {};
  for (const p of points) {
    if (!byDate[p.date]) byDate[p.date] = { visitors: 0, bookings: 0 };
    byDate[p.date].visitors += p.visitors; byDate[p.date].bookings += p.bookings;
  }
  return Object.entries(byDate).map(([date, v]) => ({ date: date.slice(5), ...v }));
}

const AN_STYLE = `
  .header-row { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px; gap: 12px; flex-wrap: wrap; }
  .page-title { font-size: 28px; font-weight: 700; }
  .page-sub { color: var(--text-dim); margin-top: 4px; }
  .range-tabs { display: flex; gap: 4px; }
  .range-tab { background: var(--bg-elev); color: var(--text-dim); border: 1px solid var(--border); padding: 6px 14px; border-radius: 8px; font-size: 13px; }
  .range-tab.active { background: var(--accent); color: white; border-color: var(--accent); }
  .an-tabs { display: flex; gap: 4px; margin-bottom: 20px; border-bottom: 1px solid var(--border); }
  .an-tab { background: transparent; color: var(--text-dim); padding: 10px 16px; border-radius: 8px 8px 0 0; font-size: 14px; font-weight: 600; border-bottom: 2px solid transparent; }
  .an-tab:hover { color: var(--text); }
  .an-tab.on { color: var(--accent); border-bottom-color: var(--accent); }
  .kpi-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 14px; margin-bottom: 20px; }
  .kpi { display: flex; align-items: center; gap: 12px; }
  .kpi.clickable:hover { border-color: var(--accent); }
  .kpi-icon { width: 40px; height: 40px; border-radius: 10px; display: flex; align-items: center; justify-content: center; font-size: 18px; font-weight: 700; flex-shrink: 0; }
  .kpi-val { font-size: 22px; font-weight: 700; }
  .kpi-lab { font-size: 12px; color: var(--text-dim); }
  .kpi-sub { font-size: 11px; color: var(--text-dim); margin-top: 2px; }
  .fin-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-bottom: 20px; }
  .fin-stat { padding: 18px 20px; }
  .fin-stat.clickable:hover { border-color: var(--accent); }
  .fin-v { font-size: 24px; font-weight: 700; }
  .fin-l { font-size: 12px; color: var(--text-dim); margin-top: 4px; }
  .chart-card { margin-bottom: 16px; }
  .chart-row { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 16px; }
  .card-h { font-size: 14px; font-weight: 600; margin-bottom: 14px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; }
  .dt { width: 100%; border-collapse: collapse; }
  .dt th { text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-dim); padding: 8px 10px; border-bottom: 1px solid var(--border); }
  .dt td { padding: 10px; border-bottom: 1px solid var(--border); font-size: 14px; }
  .cap { text-transform: capitalize; }
  .info-banner { margin-bottom: 16px; font-size: 13px; color: var(--text-dim); }
  .rep-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  .rep-card { text-align: center; padding: 24px; }
  .rep-icon { font-size: 36px; margin-bottom: 10px; }
  .rep-card h3 { font-size: 15px; margin-bottom: 6px; }
  .rep-card p { font-size: 13px; color: var(--text-dim); margin-bottom: 14px; }
  .rep-btn { font-size: 12px; opacity: 0.6; }
  @media (max-width: 900px) { .kpi-grid { grid-template-columns: repeat(2, 1fr); } .fin-row { grid-template-columns: repeat(2, 1fr); } .chart-row { grid-template-columns: 1fr; } .rep-grid { grid-template-columns: 1fr; } }
`;
