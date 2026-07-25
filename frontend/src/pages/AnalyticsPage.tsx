import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  ResponsiveContainer, AreaChart, Area, BarChart, Bar, PieChart, Pie, Cell,
  XAxis, YAxis, Tooltip, CartesianGrid, Legend,
} from "recharts";
import {
  analytics, type AnalyticsOverview, type TimeSeriesPoint, type WebsiteTrafficPoint, type SourceBreakdown,
  type RevenueByType, type NoShowRate, type HourCount, type AgeBracket, type OutstandingPatient,
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
  const [growth, setGrowth] = useState<TimeSeriesPoint[]>([]);
  const [revByType, setRevByType] = useState<RevenueByType[]>([]);
  const [noShow, setNoShow] = useState<NoShowRate | null>(null);
  const [hours, setHours] = useState<HourCount[]>([]);
  const [ages, setAges] = useState<AgeBracket[]>([]);
  const [outstanding, setOutstanding] = useState<OutstandingPatient[]>([]);
  const [range, setRange] = useState(30);

  useEffect(() => {
    analytics.overview().then((r) => setOv(r.data));
    analytics.revenue(range).then((r) => setRev(r.data));
    analytics.appointments(range).then((r) => setAppt(r.data));
    analytics.traffic(range).then((r) => setTraffic(r.data));
    analytics.trafficBySource().then((r) => setSources(r.data));
    analytics.patientGrowth(range).then((r) => setGrowth(r.data));
  }, [range]);

  // These aggregates aren't range-dependent — fetch once.
  useEffect(() => {
    analytics.revenueByType().then((r) => setRevByType(r.data));
    analytics.noShowRate().then((r) => setNoShow(r.data));
    analytics.hourDistribution().then((r) => setHours(r.data));
    analytics.ageDemographics().then((r) => setAges(r.data));
    analytics.outstandingByPatient().then((r) => setOutstanding(r.data));
  }, []);

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

      {tab === "overview" && <OverviewTab ov={ov} nav={nav} hours={hours} noShow={noShow} />}
      {tab === "financial" && <FinancialTab ov={ov} rev={rev} fmtDate={fmtDate} revByType={revByType} />}
      {tab === "patients" && <PatientsTab ov={ov} appt={appt} fmtDate={fmtDate} nav={nav} growth={growth} ages={ages} />}
      {tab === "website" && <WebsiteTab trafficAgg={trafficAgg} sources={sources} />}
      {tab === "reports" && <ReportsTab ov={ov} revByType={revByType} noShow={noShow} ages={ages} outstanding={outstanding} nav={nav} />}

      <style>{AN_STYLE}</style>
    </div>
  );
}

function OverviewTab({ ov, nav, hours, noShow }: any) {
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

      <div className="chart-row">
        <div className="card chart-card">
          <h3 className="card-h">Busiest Hours</h3>
          <ResponsiveContainer width="100%" height={220}>
            <BarChart data={(hours || []).map((h: HourCount) => ({ hour: fmtHour(h.hour), Appointments: h.count }))}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
              <XAxis dataKey="hour" stroke="var(--text-dim)" fontSize={10} interval={1} />
              <YAxis allowDecimals={false} stroke="var(--text-dim)" fontSize={11} />
              <Tooltip contentStyle={tooltipStyle} />
              <Bar dataKey="Appointments" fill="#a78bfa" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
        <div className="card chart-card">
          <h3 className="card-h">Appointment Reliability</h3>
          {noShow ? (
            <div className="rel-wrap">
              <div className="rel-row">
                <span className="rel-lab">Completed</span>
                <div className="rel-bar"><div className="rel-fill" style={{ width: `${pct(noShow.completed, noShow.total)}%`, background: "var(--green)" }} /></div>
                <span className="rel-num">{noShow.completed}</span>
              </div>
              <div className="rel-row">
                <span className="rel-lab">No-shows</span>
                <div className="rel-bar"><div className="rel-fill" style={{ width: `${pct(noShow.no_show, noShow.total)}%`, background: "var(--red)" }} /></div>
                <span className="rel-num">{noShow.no_show}</span>
              </div>
              <div className="rel-row">
                <span className="rel-lab">Cancelled</span>
                <div className="rel-bar"><div className="rel-fill" style={{ width: `${pct(noShow.cancelled, noShow.total)}%`, background: "var(--amber)" }} /></div>
                <span className="rel-num">{noShow.cancelled}</span>
              </div>
              <div className="rel-summary">
                <div><span className="rel-big" style={{ color: "var(--red)" }}>{noShow.no_show_rate.toFixed(1)}%</span><span className="rel-cap">No-show rate</span></div>
                <div><span className="rel-big" style={{ color: "var(--amber)" }}>{noShow.cancellation_rate.toFixed(1)}%</span><span className="rel-cap">Cancellation rate</span></div>
              </div>
            </div>
          ) : <div className="empty-note">No appointment data yet.</div>}
        </div>
      </div>
    </>
  );
}

function FinancialTab({ ov, rev, fmtDate, revByType }: any) {
  const rbt: RevenueByType[] = revByType || [];
  const totalRbt = rbt.reduce((s, r) => s + r.revenue, 0);
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

      <div className="chart-row">
        <div className="card chart-card">
          <h3 className="card-h">Revenue by Appointment Type</h3>
          {rbt.length ? (
            <ResponsiveContainer width="100%" height={260}>
              <BarChart layout="vertical" data={rbt.map((r) => ({ type: cap(r.appointment_type), Revenue: r.revenue }))} margin={{ left: 20 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis type="number" stroke="var(--text-dim)" fontSize={11} tickFormatter={(v: number) => `$${v.toLocaleString()}`} />
                <YAxis type="category" dataKey="type" stroke="var(--text-dim)" fontSize={11} width={110} />
                <Tooltip contentStyle={tooltipStyle} formatter={(v: any) => `$${Number(v).toLocaleString()}`} />
                <Bar dataKey="Revenue" radius={[0, 4, 4, 0]}>
                  {rbt.map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          ) : <div className="empty-note">No invoiced revenue yet.</div>}
        </div>
        <div className="card chart-card">
          <h3 className="card-h">Breakdown</h3>
          <table className="dt">
            <thead><tr><th>Type</th><th>Invoices</th><th>Revenue</th><th>Share</th></tr></thead>
            <tbody>
              {rbt.map((r, i) => (
                <tr key={r.appointment_type}>
                  <td><span className="dot" style={{ background: PIE_COLORS[i % PIE_COLORS.length] }} />{cap(r.appointment_type)}</td>
                  <td>{r.count}</td>
                  <td>${r.revenue.toLocaleString()}</td>
                  <td>{totalRbt > 0 ? ((r.revenue / totalRbt) * 100).toFixed(1) : "0"}%</td>
                </tr>
              ))}
              {!rbt.length && <tr><td colSpan={4} className="empty-note">No data.</td></tr>}
            </tbody>
          </table>
        </div>
      </div>
    </>
  );
}

function PatientsTab({ ov, appt, fmtDate, nav, growth, ages }: any) {
  const gr: TimeSeriesPoint[] = growth || [];
  const ag: AgeBracket[] = ages || [];
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

      <div className="chart-row">
        <div className="card chart-card">
          <h3 className="card-h">Patient Growth (new per week)</h3>
          {gr.length ? (
            <ResponsiveContainer width="100%" height={260}>
              <AreaChart data={gr.map((p) => ({ week: fmtDate(p.date), "New patients": p.value }))}>
                <defs><linearGradient id="grow" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#4f9cf9" stopOpacity={0.4} /><stop offset="95%" stopColor="#4f9cf9" stopOpacity={0} /></linearGradient></defs>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis dataKey="week" stroke="var(--text-dim)" fontSize={11} />
                <YAxis allowDecimals={false} stroke="var(--text-dim)" fontSize={11} />
                <Tooltip contentStyle={tooltipStyle} />
                <Area type="monotone" dataKey="New patients" stroke="#4f9cf9" strokeWidth={2} fill="url(#grow)" />
              </AreaChart>
            </ResponsiveContainer>
          ) : <div className="empty-note">No new patients in this range.</div>}
        </div>
        <div className="card chart-card">
          <h3 className="card-h">Age Demographics</h3>
          {ag.some((a) => a.count > 0) ? (
            <ResponsiveContainer width="100%" height={260}>
              <PieChart>
                <Pie data={ag.filter((a) => a.count > 0)} dataKey="count" nameKey="bracket" cx="50%" cy="50%" outerRadius={80} label={(e: any) => `${e.bracket} (${e.count})`}>
                  {ag.filter((a) => a.count > 0).map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
                </Pie>
                <Tooltip contentStyle={tooltipStyle} />
              </PieChart>
            </ResponsiveContainer>
          ) : <div className="empty-note">No patient age data yet.</div>}
        </div>
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

function ReportsTab({ ov, revByType, noShow, ages, outstanding, nav }: any) {
  const [active, setActive] = useState<string | null>(null);
  const reports: { name: string; desc: string; icon: string; live: boolean }[] = [
    { name: "Financial Summary", desc: "Revenue, outstanding balances, revenue by type", icon: "💰", live: true },
    { name: "Patient Demographics", desc: "Age bracket breakdown of your patient base", icon: "👥", live: true },
    { name: "Appointment Analytics", desc: "No-shows, cancellations, completion rate", icon: "📅", live: true },
    { name: "Outstanding Balances", desc: "Top patients owing money, by amount", icon: "🧾", live: true },
    { name: "Treatment Outcomes", desc: "OSDI score progression, IPL response rates", icon: "🔬", live: false },
    { name: "Inventory Report", desc: "Stock levels, usage, reordering", icon: "📦", live: false },
  ];

  return (
    <>
      <div className="card info-banner">📋 Click a <strong>live report</strong> to generate a summary from your current data. Exportable/print versions are planned for a later phase.</div>
      <div className="rep-grid">
        {reports.map((r) => (
          <div key={r.name} className={`card rep-card ${active === r.name ? "rep-on" : ""}`}>
            <div className="rep-icon">{r.icon}</div>
            <h3>{r.name}</h3>
            <p>{r.desc}</p>
            {r.live ? (
              <button className="btn-ghost rep-btn live" onClick={() => setActive(active === r.name ? null : r.name)}>
                {active === r.name ? "Hide" : "Generate"}
              </button>
            ) : (
              <button className="btn-ghost rep-btn" disabled>Coming soon</button>
            )}
          </div>
        ))}
      </div>

      {active && (
        <div className="card report-out">
          <h3 className="card-h">{active}</h3>
          {active === "Financial Summary" && <FinancialSummaryReport ov={ov} revByType={revByType} outstanding={outstanding} />}
          {active === "Patient Demographics" && <DemographicsReport ages={ages} ov={ov} />}
          {active === "Appointment Analytics" && <AppointmentReport noShow={noShow} />}
          {active === "Outstanding Balances" && <OutstandingReport outstanding={outstanding} nav={nav} />}
        </div>
      )}
    </>
  );
}

function FinancialSummaryReport({ ov, revByType, outstanding }: any) {
  const rbt: RevenueByType[] = revByType || [];
  const totalRbt = rbt.reduce((s, r) => s + r.revenue, 0);
  const totalOutstanding = (outstanding || []).reduce((s: number, r: OutstandingPatient) => s + r.outstanding, 0);
  return (
    <>
      <div className="rep-kpis">
        <div><span className="rep-kv" style={{ color: "var(--green)" }}>${ov?.total_revenue.toLocaleString() ?? "—"}</span><span className="rep-kl">Total revenue</span></div>
        <div><span className="rep-kv" style={{ color: "var(--green)" }}>${ov?.revenue_this_month.toLocaleString() ?? "—"}</span><span className="rep-kl">This month</span></div>
        <div><span className="rep-kv" style={{ color: "var(--red)" }}>${ov?.outstanding_balance.toLocaleString() ?? "—"}</span><span className="rep-kl">Outstanding</span></div>
        <div><span className="rep-kv">${ov?.avg_appt_value.toFixed(0) ?? "—"}</span><span className="rep-kl">Avg per visit</span></div>
      </div>
      <h4 className="rep-sub">Revenue by appointment type</h4>
      <table className="dt">
        <thead><tr><th>Type</th><th>Invoices</th><th>Revenue</th><th>Share</th></tr></thead>
        <tbody>
          {rbt.map((r) => (
            <tr key={r.appointment_type}>
              <td>{cap(r.appointment_type)}</td><td>{r.count}</td>
              <td>${r.revenue.toLocaleString()}</td>
              <td>{totalRbt > 0 ? ((r.revenue / totalRbt) * 100).toFixed(1) : "0"}%</td>
            </tr>
          ))}
          {rbt.length > 0 && (
            <tr className="rep-total"><td>Total</td><td>{rbt.reduce((s, r) => s + r.count, 0)}</td><td>${totalRbt.toLocaleString()}</td><td>100%</td></tr>
          )}
          {!rbt.length && <tr><td colSpan={4} className="empty-note">No invoiced revenue yet.</td></tr>}
        </tbody>
      </table>
      <p className="rep-foot">Outstanding across top patients: <strong style={{ color: "var(--red)" }}>${totalOutstanding.toLocaleString()}</strong></p>
    </>
  );
}

function DemographicsReport({ ages, ov }: any) {
  const ag: AgeBracket[] = ages || [];
  const total = ag.reduce((s, a) => s + a.count, 0);
  return (
    <>
      <p className="rep-foot">Total patients: <strong>{ov?.total_patients ?? total}</strong> · with recorded age: <strong>{total}</strong></p>
      <table className="dt">
        <thead><tr><th>Age bracket</th><th>Patients</th><th>Share</th></tr></thead>
        <tbody>
          {ag.map((a) => (
            <tr key={a.bracket}>
              <td>{a.bracket}</td><td>{a.count}</td>
              <td>{total > 0 ? ((a.count / total) * 100).toFixed(1) : "0"}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function AppointmentReport({ noShow }: any) {
  if (!noShow) return <div className="empty-note">No appointment data yet.</div>;
  const rows = [
    { label: "Completed", value: noShow.completed, color: "var(--green)" },
    { label: "No-shows", value: noShow.no_show, color: "var(--red)" },
    { label: "Cancelled", value: noShow.cancelled, color: "var(--amber)" },
  ];
  return (
    <>
      <div className="rep-kpis">
        <div><span className="rep-kv">{noShow.total}</span><span className="rep-kl">Total appointments</span></div>
        <div><span className="rep-kv" style={{ color: "var(--red)" }}>{noShow.no_show_rate.toFixed(1)}%</span><span className="rep-kl">No-show rate</span></div>
        <div><span className="rep-kv" style={{ color: "var(--amber)" }}>{noShow.cancellation_rate.toFixed(1)}%</span><span className="rep-kl">Cancellation rate</span></div>
      </div>
      <table className="dt">
        <thead><tr><th>Status</th><th>Count</th><th>Share</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.label}>
              <td><span className="dot" style={{ background: r.color }} />{r.label}</td>
              <td>{r.value}</td>
              <td>{pct(r.value, noShow.total).toFixed(1)}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function OutstandingReport({ outstanding, nav }: any) {
  const rows: OutstandingPatient[] = outstanding || [];
  return rows.length ? (
    <table className="dt">
      <thead><tr><th>Patient</th><th>MRN</th><th>Invoices</th><th>Outstanding</th></tr></thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.patient_id} className="clickable-row" onClick={() => nav(`/patients/${r.patient_id}`)}>
            <td>{r.name}</td><td>{r.mrn}</td><td>{r.invoice_count}</td>
            <td style={{ color: "var(--red)", fontWeight: 600 }}>${r.outstanding.toLocaleString()}</td>
          </tr>
        ))}
      </tbody>
    </table>
  ) : <div className="empty-note">No outstanding balances. 🎉</div>;
}

const tooltipStyle = { background: "var(--bg-elev-2)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 };
const cap = (s: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : s);
const pct = (n: number, total: number) => (total > 0 ? (n / total) * 100 : 0);
const fmtHour = (h: number) => {
  const ampm = h < 12 ? "a" : "p";
  const hr = h % 12 === 0 ? 12 : h % 12;
  return `${hr}${ampm}`;
};
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
  .rep-btn.live { opacity: 1; cursor: pointer; border-color: var(--accent); color: var(--accent); }
  .rep-btn.live:hover { background: var(--accent); color: white; }
  .rep-card.rep-on { border-color: var(--accent); }
  .report-out { margin-top: 16px; }
  .rep-kpis { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-bottom: 18px; }
  .rep-kpis > div { display: flex; flex-direction: column; gap: 2px; }
  .rep-kv { font-size: 22px; font-weight: 700; }
  .rep-kl { font-size: 12px; color: var(--text-dim); }
  .rep-sub { font-size: 13px; font-weight: 600; margin: 8px 0 10px; color: var(--text); }
  .rep-total td { font-weight: 700; border-top: 2px solid var(--border); }
  .rep-foot { font-size: 13px; color: var(--text-dim); margin-top: 12px; }
  .empty-note { color: var(--text-dim); font-size: 13px; padding: 30px 10px; text-align: center; }
  .dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 8px; vertical-align: middle; }
  .clickable-row { cursor: pointer; }
  .clickable-row:hover { background: var(--bg-elev); }
  .rel-wrap { display: flex; flex-direction: column; gap: 14px; padding: 6px 0; }
  .rel-row { display: grid; grid-template-columns: 90px 1fr 40px; align-items: center; gap: 10px; }
  .rel-lab { font-size: 12px; color: var(--text-dim); }
  .rel-bar { height: 10px; background: var(--bg-elev); border-radius: 5px; overflow: hidden; }
  .rel-fill { height: 100%; border-radius: 5px; transition: width 0.3s; }
  .rel-num { font-size: 13px; font-weight: 600; text-align: right; }
  .rel-summary { display: flex; gap: 30px; margin-top: 8px; padding-top: 14px; border-top: 1px solid var(--border); }
  .rel-summary > div { display: flex; flex-direction: column; }
  .rel-big { font-size: 22px; font-weight: 700; }
  .rel-cap { font-size: 11px; color: var(--text-dim); }
  @media (max-width: 900px) { .kpi-grid { grid-template-columns: repeat(2, 1fr); } .fin-row { grid-template-columns: repeat(2, 1fr); } .chart-row { grid-template-columns: 1fr; } .rep-grid { grid-template-columns: 1fr; } .rep-kpis { grid-template-columns: repeat(2, 1fr); } }
`;
