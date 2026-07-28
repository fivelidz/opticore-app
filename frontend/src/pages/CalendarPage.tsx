import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  addDays, addMonths, startOfWeek, startOfMonth, endOfMonth, eachDayOfInterval,
  format, isSameDay, isSameMonth, parseISO, addMinutes, differenceInMinutes,
  startOfDay, endOfDay, isToday,
} from "date-fns";
import {
  calendar, blockedTimes, appointments as apptApi, patients as patApi, patientDetail,
  type CalendarEvent, type Patient, type PatientDetail,
} from "../api";

type View = "month" | "week" | "day";

export function CalendarPage() {
  const nav = useNavigate();
  const [view, setView] = useState<View>("week");
  const [cursor, setCursor] = useState(new Date());
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [patients, setPatients] = useState<Patient[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<CalendarEvent | null>(null);
  const [editMode, setEditMode] = useState(false);
  const [showMass, setShowMass] = useState(false);
  const [showCreate, setShowCreate] = useState<{ start: Date; end: Date } | null>(null);

  // keyboard nav (Aria-style)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "ArrowLeft") step(-1);
      if (e.key === "ArrowRight") step(1);
      if (e.key === "t" || e.key === "T") setCursor(new Date());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, cursor]);

  const step = (dir: number) => {
    if (view === "month") setCursor(addMonths(cursor, dir));
    else if (view === "week") setCursor(addDays(cursor, 7 * dir));
    else setCursor(addDays(cursor, dir));
  };

  const load = () => {
    setLoading(true);
    let from: Date, to: Date;
    if (view === "month") {
      from = startOfWeek(startOfMonth(cursor), { weekStartsOn: 1 });
      to = addDays(endOfMonth(cursor), 7);
    } else if (view === "week") {
      from = startOfWeek(cursor, { weekStartsOn: 1 });
      to = addDays(from, 8);
    } else {
      from = startOfDay(cursor); to = addDays(startOfDay(cursor), 1);
    }
    Promise.all([
      calendar.range(format(from, "yyyy-MM-dd"), format(to, "yyyy-MM-dd")),
      patApi.list(),
    ]).then(([c, p]) => { setEvents(c.data); setPatients(p.data.patients); })
      .finally(() => setLoading(false));
  };
  useEffect(load, [view, cursor]);

  const title = view === "month" ? format(cursor, "MMMM yyyy")
    : view === "week" ? `${format(startOfWeek(cursor, { weekStartsOn: 1 }), "d MMM")} – ${format(addDays(startOfWeek(cursor, { weekStartsOn: 1 }), 6), "d MMM yyyy")}`
    : format(cursor, "EEEE d MMMM yyyy");

  const evByDay = (day: Date) => events.filter((e) => isSameDay(parseISO(e.start_at.replace(" ", "T")), day));

  // ---- event move (true drag) ----
  const moveEvent = async (ev: CalendarEvent, newStart: Date) => {
    // Preserve the ORIGINAL duration of the event when moving it.
    const dur = differenceInMinutes(parseISO(ev.end_at.replace(" ", "T")), parseISO(ev.start_at.replace(" ", "T")));
    const newEnd = addMinutes(newStart, dur);
    // Skip no-op moves (dropped back on the same start time).
    if (isSameDay(parseISO(ev.start_at.replace(" ", "T")), newStart) &&
        differenceInMinutes(newStart, parseISO(ev.start_at.replace(" ", "T"))) === 0) {
      setSelected(null); return;
    }
    try {
      if (ev.kind === "appointment") {
        await apptApi.update(ev.id, {
          appointment_type: ev.title,
          appointment_date: fmt(newStart),
          duration_minutes: dur,
          practitioner: ev.practitioner || undefined,
          status: ev.status || "scheduled",
          notes: ev.reason ?? undefined,
        });
      } else {
        // atomic update — no delete+create data-loss risk
        await blockedTimes.update(ev.id, { start_at: fmt(newStart), end_at: fmt(newEnd), reason: ev.title, practitioner: ev.practitioner || undefined });
      }
    } finally {
      setSelected(null);
      load(); // reload so the event renders in its new position (confirms the change)
    }
  };

  return (
    <div className="cal-page">
      <div className="cal-toolbar">
        <div className="cal-nav">
          <button className="cnav" onClick={() => step(-1)} title="Previous (←)">‹</button>
          <button className="cnav today-btn" onClick={() => setCursor(new Date())} title="Today (T)">⌂ Today</button>
          <button className="cnav" onClick={() => step(1)} title="Next (→)">›</button>
          <h1 className="cal-title">{title}</h1>
        </div>
        <div className="cal-right">
          <div className="vtabs">
            {(["month", "week", "day"] as View[]).map((v) => (
              <button key={v} className={`vtab ${view === v ? "on" : ""}`} onClick={() => setView(v)}>
                {v === "month" ? "Month" : v === "week" ? "Week" : "Day"}
              </button>
            ))}
          </div>
          <button className="btn-ghost" onClick={() => setShowMass(true)}>🗓 Mass Block</button>
          <button className={editMode ? "btn-primary" : "btn-ghost"} onClick={() => setEditMode(!editMode)}>
            {editMode ? "✏️ Editing" : "✏️ Edit"}
          </button>
        </div>
      </div>

      <div className={`cal-layout ${selected ? "with-panel" : ""}`}>
        <div className="cal-main">
          {view === "month" && <MonthView cursor={cursor} evByDay={evByDay} onDayClick={(d: Date) => { setCursor(d); setView("day"); }} onEventClick={setSelected} editMode={editMode} />}
          {view === "week" && <WeekView cursor={cursor} evByDay={evByDay} onEventClick={setSelected} onCreate={(s: Date, e: Date) => setShowCreate({ start: s, end: e })} editMode={editMode} onMove={moveEvent} />}
          {view === "day" && <DayView cursor={cursor} events={evByDay(cursor)} onEventClick={setSelected} onCreate={(s: Date, e: Date) => setShowCreate({ start: s, end: e })} editMode={editMode} onMove={moveEvent} />}
        </div>
        {selected && <SidePanel ev={selected} onClose={() => setSelected(null)} onOpenPatient={(pid) => nav(`/patients/${pid}`)} />}
      </div>

      <div className="legend">
        <span className="badge badge-scheduled">Appointment</span>
        <span className="badge badge-confirmed">Confirmed</span>
        <span className="badge badge-blocked">Blocked</span>
        <span className="legend-paid"><span className="dot-green" /> Paid</span>
        <span className="legend-paid"><span className="dot-amber" /> $ outstanding</span>
        <span className="cal-hint">{editMode ? "✏️ Drag appointments to reschedule · drag empty space to create" : "💡 Click an appointment for details · click ✏️ Edit to drag-reschedule · ← → keys navigate"}</span>
      </div>

      {showCreate && <CreateOrBlockModal start={showCreate.start} end={showCreate.end} patients={patients} onClose={() => setShowCreate(null)} onSaved={() => { setShowCreate(null); load(); }} />}
      {showMass && <MassBlockModal onClose={() => setShowMass(false)} onSaved={() => { setShowMass(false); load(); }} />}
      <style>{CAL_STYLE}</style>
    </div>
  );
}

// ============ MONTH VIEW ============
function MonthView({ cursor, evByDay, onDayClick, onEventClick, editMode }: any) {
  const monthStart = startOfMonth(cursor);
  const gridStart = startOfWeek(monthStart, { weekStartsOn: 1 });
  const days = eachDayOfInterval({ start: gridStart, end: addDays(gridStart, 41) });
  const dows = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

  return (
    <div className="month-view">
      <div className="month-dows">{dows.map((d) => <div key={d} className="dow-cell">{d}</div>)}</div>
      <div className="month-grid">
        {days.map((d) => {
          const evs = evByDay(d).slice(0, 3);
          const inMonth = isSameMonth(d, cursor);
          return (
            <div key={d.toISOString()} className={`mcell ${isToday(d) ? "today" : ""} ${!inMonth ? "other" : ""}`} onClick={() => onDayClick(d)}>
              <div className="mday">{format(d, "d")}</div>
              <div className="mevs">
                {evs.map((e: CalendarEvent) => (
                  <div key={e.id} className={`mpill ev-${e.kind} ${e.status || ""}`} onClick={(ev) => { ev.stopPropagation(); onEventClick(e); }}>
                    <span className="mpill-time">{format(parseISO(e.start_at.replace(" ", "T")), "HH:mm")}</span>
                    <span className="mpill-title">{e.title}</span>
                  </div>
                ))}
                {evByDay(d).length > 3 && <div className="mpill-more">+{evByDay(d).length - 3} more</div>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ============ WEEK VIEW (with drag-select + drag-move) ============
function WeekView({ cursor, evByDay, onEventClick, onCreate, editMode, onMove }: any) {
  const days = Array.from({ length: 7 }, (_, i) => addDays(startOfWeek(cursor, { weekStartsOn: 1 }), i));
  const HOURS = Array.from({ length: 12 }, (_, i) => i + 7); // 7am–7pm
  const PX_PER_MIN = 1.4; // 84px/hour — must match WeekRow/DayView for correct drag positioning

  // drag-select state
  const [drag, setDrag] = useState<{ dayIdx: number; startMin: number; endMin: number } | null>(null);
  // drag-move state
  const [moving, setMoving] = useState<{ ev: CalendarEvent; offsetMin: number } | null>(null);
  const [hoverDay, setHoverDay] = useState<number | null>(null);
  const [hoverMin, setHoverMin] = useState<number | null>(null);

  // Minutes-within-a-single-hour-cell (0–60). Each cell is one hour tall (60min * PX_PER_MIN).
  const minInCell = (y: number) => Math.max(0, Math.min(60, Math.round(y / PX_PER_MIN / 15) * 15));
  // Convert a cell's hour + in-cell offset into absolute minutes from 7am (grid origin), clamped 0..720.
  const absMin = (hour: number, y: number) => Math.max(0, Math.min(720, (hour - 7) * 60 + minInCell(y)));

  const onDownCell = (dayIdx: number, hour: number, e: React.MouseEvent) => {
    if (!editMode) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const startMin = absMin(hour, e.clientY - rect.top);
    setDrag({ dayIdx, startMin, endMin: startMin + 30 });
  };
  const onMoveCell = (dayIdx: number, hour: number, e: React.MouseEvent) => {
    if (drag && drag.dayIdx === dayIdx) {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      setDrag({ ...drag, endMin: Math.max(drag.startMin + 15, absMin(hour, e.clientY - rect.top)) });
    }
    if (moving) {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      setHoverDay(dayIdx);
      setHoverMin(absMin(hour, e.clientY - rect.top));
    }
  };
  const onUpCell = () => {
    if (drag) {
      const day = days[drag.dayIdx];
      const start = addMinutes(setHour(day, 7), drag.startMin);
      const end = addMinutes(setHour(day, 7), drag.endMin);
      onCreate(start, end);
    }
    if (moving && hoverDay !== null && hoverMin !== null) {
      const day = days[hoverDay];
      const evStart = parseISO(moving.ev.start_at.replace(" ", "T"));
      const newStart = addMinutes(setHour(day, 7), hoverMin - moving.offsetMin);
      onMove(moving.ev, newStart);
    }
    setDrag(null); setMoving(null); setHoverDay(null); setHoverMin(null);
  };

  const startMove = (ev: CalendarEvent, e: React.MouseEvent) => {
    if (!editMode) return;
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    // offset within the event where the user grabbed it (0..duration minutes)
    const offsetMin = minInCell(e.clientY - rect.top);
    setMoving({ ev, offsetMin });
  };

  return (
    <div className="week-view">
      <div className="week-grid">
        <div className="week-corner"></div>
        {days.map((d) => (
          <div key={d.toISOString()} className={`week-head ${isToday(d) ? "today" : ""}`}>
            <div className="wh-dow">{format(d, "EEE")}</div>
            <div className="wh-dom">{format(d, "d")}</div>
          </div>
        ))}
        {HOURS.map((h) => (
          <WeekRow key={h} hour={h} days={days} evByDay={evByDay}
            onDown={onDownCell} onMove={onMoveCell} onUp={onUpCell}
            drag={drag} onEventClick={onEventClick} editMode={editMode}
            moving={moving} hoverDay={hoverDay} hoverMin={hoverMin} startMove={startMove} />
        ))}
      </div>
    </div>
  );
}

function WeekRow({ hour, days, evByDay, onDown, onMove, onUp, drag, onEventClick, editMode, moving, hoverDay, hoverMin, startMove }: any) {
  const PX_PER_MIN = 1.4; // 84px/hour — durations clearly visible
  return (
    <>
      <div className="week-time">{format(new Date().setHours(hour, 0), "HH:mm")}</div>
      {days.map((d: Date, dayIdx: number) => {
        const evs = evByDay(d).filter((e: CalendarEvent) => parseISO(e.start_at.replace(" ", "T")).getHours() === hour);
        const showDrag = drag && drag.dayIdx === dayIdx && drag.startMin >= (hour - 7) * 60 && drag.startMin < (hour - 6) * 60;
        const showHover = moving && hoverDay === dayIdx && hoverMin !== null && hoverMin >= (hour - 7) * 60 && hoverMin < (hour - 6) * 60;
        return (
          <div key={dayIdx} className="week-cell"
            onMouseDown={(e) => onDown(dayIdx, hour, e)}
            onMouseMove={(e) => onMove(dayIdx, hour, e)}
            onMouseUp={onUp}
          >
            {evs.map((e: CalendarEvent) => {
              const start = parseISO(e.start_at.replace(" ", "T"));
              const end = parseISO(e.end_at.replace(" ", "T"));
              const top = start.getMinutes() * PX_PER_MIN;
              const dur = differenceInMinutes(end, start);
              const height = Math.max(16, dur * PX_PER_MIN);
              return (
                <div key={e.id} className={`ev ev-${e.kind} ${e.status || ""} ${e.paid ? "paid-" + e.paid : ""} ${editMode ? "draggable" : ""}`}
                  style={{ top, height }}
                  onMouseDown={(ev) => startMove(e, ev)}
                  onClick={(ev) => { if (!editMode) { ev.stopPropagation(); onEventClick(e); } }}
                  title={editMode ? "Drag to reschedule" : `${e.title} — ${format(start,"HH:mm")}–${format(end,"HH:mm")} (${dur}min)`}
                >
                  <div className="ev-time">{format(start, "HH:mm")}<span className="ev-dur"> · {dur}m</span></div>
                  <div className="ev-title">{e.title}</div>
                  {height > 44 && <div className="ev-end">→ {format(end, "HH:mm")}</div>}
                  {e.paid === "paid" && <span className="ev-paid ok" title="Paid in full">✓ Paid</span>}
                  {e.paid === "partial" && <span className="ev-paid part" title={`Outstanding: $${(e.balance||0).toFixed(2)}`}>⚠ $${(e.balance||0).toFixed(0)} due</span>}
                </div>
              );
            })}
            {showDrag && (
              <div className="ev ev-draft" style={{ top: (drag.startMin % 60) * PX_PER_MIN, height: (drag.endMin - drag.startMin) * PX_PER_MIN }}>
                <div className="ev-time">{format(addMinutes(setHour(d, 7), drag.startMin), "HH:mm")}</div>
                <div>+ New</div>
              </div>
            )}
            {showHover && moving && (
              <div className="ev ev-hover" style={{ top: ((hoverMin - moving.offsetMin) % 60) * PX_PER_MIN }}>
                <div className="ev-time">→ move</div>
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}

// ============ DAY VIEW ============
function DayView({ cursor, events, onEventClick, onCreate, editMode, onMove }: any) {
  const HOURS = Array.from({ length: 12 }, (_, i) => i + 7);
  const PX_PER_MIN = 1.4; // 84px/hour — durations clearly visible
  const [drag, setDrag] = useState<{ startMin: number; endMin: number } | null>(null);
  const [moving, setMoving] = useState<{ ev: CalendarEvent; offsetMin: number } | null>(null);
  const [hoverMin, setHoverMin] = useState<number | null>(null);

  // Minutes-within-a-single-hour-cell (0–60).
  const minInCell = (y: number) => Math.max(0, Math.min(60, Math.round(y / PX_PER_MIN / 15) * 15));
  // Absolute minutes from 7am (grid origin) for a given cell hour, clamped 0..720.
  const absMin = (hour: number, y: number) => Math.max(0, Math.min(720, (hour - 7) * 60 + minInCell(y)));

  return (
    <div className="day-view">
      <div className="day-grid">
        <div className="day-corner">{format(cursor, "EEE d")}</div>
        {HOURS.map((h) => {
          const evs = events.filter((e: CalendarEvent) => parseISO(e.start_at.replace(" ", "T")).getHours() === h);
          const showDrag = drag && drag.startMin >= (h - 7) * 60 && drag.startMin < (h - 6) * 60;
          const showHover = moving && hoverMin !== null && hoverMin >= (h - 7) * 60 && hoverMin < (h - 6) * 60;
          return (
            <div key={h} className="day-row">
              <div className="day-time">{format(new Date().setHours(h, 0), "HH:mm")}</div>
              <div className="day-cell"
                onMouseDown={(e) => { if (editMode) { const s = absMin(h, e.clientY - e.currentTarget.getBoundingClientRect().top); setDrag({ startMin: s, endMin: s + 30 }); } }}
                onMouseMove={(e) => {
                  if (drag) setDrag({ ...drag, endMin: Math.max(drag.startMin + 15, absMin(h, e.clientY - e.currentTarget.getBoundingClientRect().top)) });
                  if (moving) setHoverMin(absMin(h, e.clientY - e.currentTarget.getBoundingClientRect().top));
                }}
                onMouseUp={() => {
                  if (drag) onCreate(addMinutes(setHour(cursor, 7), drag.startMin), addMinutes(setHour(cursor, 7), drag.endMin));
                  if (moving && hoverMin !== null) onMove(moving.ev, addMinutes(setHour(cursor, 7), hoverMin - moving.offsetMin));
                  setDrag(null); setMoving(null); setHoverMin(null);
                }}
              >
                {evs.map((e: CalendarEvent) => {
                  const start = parseISO(e.start_at.replace(" ", "T"));
                  const end = parseISO(e.end_at.replace(" ", "T"));
                  const dur = differenceInMinutes(end, start);
                  const height = Math.max(16, dur * PX_PER_MIN);
                  return (
                    <div key={e.id} className={`ev ev-${e.kind} ${e.status || ""} ${e.paid ? "paid-" + e.paid : ""} ${editMode ? "draggable" : ""}`}
                      style={{ top: start.getMinutes() * PX_PER_MIN, height }}
                      onMouseDown={(ev) => { if (editMode) { ev.stopPropagation(); const r = ev.currentTarget.getBoundingClientRect(); setMoving({ ev: e, offsetMin: minInCell(ev.clientY - r.top) }); } }}
                      onClick={(ev) => { if (!editMode) { ev.stopPropagation(); onEventClick(e); } }}
                      title={`${e.title} — ${format(start,"HH:mm")}–${format(end,"HH:mm")} (${dur}min)`}
                    >
                      <div className="ev-time">{format(start, "HH:mm")}<span className="ev-dur"> · {dur}m</span></div>
                      <div className="ev-title">{e.title}</div>
                      {height > 44 && <div className="ev-end">→ {format(end, "HH:mm")}</div>}
                      {e.paid === "paid" && <span className="ev-paid ok" title="Paid in full">✓ Paid</span>}
                      {e.paid === "partial" && <span className="ev-paid part" title={`Outstanding: $${(e.balance||0).toFixed(2)}`}>⚠ $${(e.balance||0).toFixed(0)} due</span>}
                    </div>
                  );
                })}
                {showDrag && (
                  <div className="ev ev-draft" style={{ top: (drag.startMin % 60) * PX_PER_MIN, height: (drag.endMin - drag.startMin) * PX_PER_MIN }}>
                    <div className="ev-time">{format(addMinutes(setHour(cursor, 7), drag.startMin), "HH:mm")}</div>
                    <div>+ New</div>
                  </div>
                )}
                {showHover && moving && (
                  <div className="ev ev-hover" style={{ top: (hoverMin % 60) * PX_PER_MIN }}><div className="ev-time">→ move</div></div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ============ SIDE PANEL ============
function SidePanel({ ev, onClose, onOpenPatient }: { ev: CalendarEvent; onClose: () => void; onOpenPatient: (pid: number) => void }) {
  const [detail, setDetail] = useState<PatientDetail | null>(null);
  const [notes, setNotes] = useState("");
  const [notesOpen, setNotesOpen] = useState(false);
  const [savingNotes, setSavingNotes] = useState(false);
  useEffect(() => {
    if (ev.patient_id) patientDetail.get(ev.patient_id).then((r) => setDetail(r.data));
  }, [ev.id]);
  const start = parseISO(ev.start_at.replace(" ", "T"));
  const end = parseISO(ev.end_at.replace(" ", "T"));

  const saveNotes = async () => {
    setSavingNotes(true);
    await apptApi.update(ev.id, {
      appointment_type: ev.title, appointment_date: ev.start_at.replace(" ", " "),
      duration_minutes: differenceInMinutes(end, start), practitioner: ev.practitioner || undefined,
      status: ev.status || "scheduled", notes,
    });
    setSavingNotes(false); setNotesOpen(false);
  };

  const cancel = async () => {
    if (ev.kind === "appointment") {
      await apptApi.update(ev.id, { appointment_type: ev.title, appointment_date: ev.start_at.replace(" ", " "), duration_minutes: differenceInMinutes(end, start), practitioner: ev.practitioner || undefined, status: "cancelled", notes: undefined });
    } else {
      await blockedTimes.remove(ev.id);
    }
    onClose();
  };
  const del = async () => {
    if (ev.kind === "appointment") await apptApi.remove(ev.id);
    else await blockedTimes.remove(ev.id);
    onClose();
  };

  return (
    <aside className="side-panel">
      <div className="sp-head">
        <h3>{ev.kind === "blocked" ? "🚫 Blocked Time" : "📅 Appointment"}</h3>
        <button className="sp-close" onClick={onClose}>×</button>
      </div>
      <div className="sp-section">
        <div className="sp-row"><span>When</span><strong>{format(start, "EEE d MMM, HH:mm")} – {format(end, "HH:mm")}</strong></div>
        <div className="sp-row"><span>Duration</span><strong>{differenceInMinutes(end, start)} min</strong></div>
        {ev.practitioner && <div className="sp-row"><span>Practitioner</span><strong>{ev.practitioner}</strong></div>}
        {ev.status && <div className="sp-row"><span>Status</span><span className={`badge badge-${ev.status}`}>{ev.status}</span></div>}
        {ev.paid === "paid" && <div className="sp-row"><span>Payment</span><span className="badge badge-confirmed">✓ Paid in full</span></div>}
        {ev.paid === "partial" && <div className="sp-row"><span>Payment</span><span className="badge badge-scheduled">⚠ ${ev.balance?.toFixed(2)} outstanding</span></div>}
        {ev.kind === "appointment" && !ev.paid && <div className="sp-row"><span>Payment</span><span className="muted">Not yet invoiced</span></div>}
        {ev.reason && <div className="sp-row"><span>Reason</span><strong>{ev.reason}</strong></div>}
      </div>

      {ev.kind === "appointment" && detail && (
        <>
          <div className="sp-divider">Patient</div>
          <div className="sp-patient" onClick={() => onOpenPatient(ev.patient_id!)}>
            <div className="sp-avatar">{detail.patient.first_name[0]}{detail.patient.last_name[0]}</div>
            <div className="sp-pinfo">
              <div className="sp-pname">{detail.patient.first_name} {detail.patient.last_name}</div>
              <div className="sp-pmeta">{detail.patient.phone || "no phone"} · {detail.patient.mrn}</div>
              <div className="sp-pstats">
                {detail.stats.total_visits} visits · ${detail.stats.total_spent.toFixed(0)}
                {detail.allergies.length > 0 && <span className="sp-allergy">⚠️ {detail.allergies.length} allergy</span>}
              </div>
            </div>
          </div>
          <button className="btn-primary sp-open" onClick={() => onOpenPatient(ev.patient_id!)}>Open Patient File →</button>
          {detail.allergies.length > 0 && (
            <div className="sp-allergies">
              <div className="sp-sub">Allergies</div>
              {detail.allergies.map((a) => <div key={a.id} className={`sp-allergy-row sev-${a.severity}`}>{a.substance} · {a.severity}</div>)}
            </div>
          )}
          {detail.notes.length > 0 && (
            <div className="sp-notes">
              <div className="sp-sub">Latest Note</div>
              <div className="sp-note">{detail.notes[0].note}</div>
            </div>
          )}
        </>
      )}

      {ev.kind === "appointment" && (
        <div className="sp-appt-notes">
          <div className="sp-sub" onClick={() => { setNotesOpen(!notesOpen); setNotes(""); }}>
            📝 Appointment Notes {notesOpen ? "▲" : "▼"}
          </div>
          {notesOpen ? (
            <>
              <textarea className="sp-notes-area" rows={4} placeholder="Pre-visit reason, post-visit findings, instructions…" value={notes} onChange={(e) => setNotes(e.target.value)} autoFocus />
              <button className="btn-primary sp-save-notes" onClick={saveNotes} disabled={savingNotes}>{savingNotes ? "Saving…" : "Save Notes"}</button>
            </>
          ) : (
            <div className="sp-note-hint muted">Click to add pre/post-visit notes</div>
          )}
        </div>
      )}
      <div className="sp-bottom-actions">
        {ev.kind === "appointment" && ev.status !== "cancelled" && <button className="btn-ghost sp-act" onClick={cancel}>✕ Cancel {ev.kind}</button>}
        <button className="btn-danger sp-act" onClick={del}>🗑 Delete</button>
      </div>
      <style>{`
        .side-panel { width: 320px; background: var(--bg-elev); border-left: 1px solid var(--border); padding: 20px; overflow-y: auto; flex-shrink: 0; display: flex; flex-direction: column; }
        .sp-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }
        .sp-head h3 { font-size: 16px; }
        .sp-close { background: none; color: var(--text-dim); font-size: 24px; padding: 0 8px; }
        .sp-close:hover { color: var(--text); }
        .sp-section { display: flex; flex-direction: column; gap: 10px; }
        .sp-row { display: flex; justify-content: space-between; align-items: center; font-size: 13px; }
        .sp-row span { color: var(--text-dim); }
        .sp-divider { font-size: 11px; font-weight: 700; color: var(--text-dim); text-transform: uppercase; letter-spacing: 1px; margin: 20px 0 12px; }
        .sp-patient { display: flex; gap: 12px; padding: 12px; background: var(--bg-elev-2); border-radius: 10px; cursor: pointer; }
        .sp-patient:hover { border: 1px solid var(--accent); }
        .sp-avatar { width: 40px; height: 40px; border-radius: 50%; background: var(--accent); color: white; display: flex; align-items: center; justify-content: center; font-weight: 700; flex-shrink: 0; }
        .sp-pname { font-weight: 600; font-size: 14px; }
        .sp-pmeta { font-size: 12px; color: var(--text-dim); margin-top: 2px; }
        .sp-pstats { font-size: 11px; color: var(--text-dim); margin-top: 4px; }
        .sp-allergy { color: var(--amber); margin-left: 6px; }
        .sp-open { width: 100%; margin-top: 12px; }
        .sp-sub { font-size: 11px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; margin: 16px 0 6px; }
        .sp-allergy-row { font-size: 13px; padding: 4px 0; }
        .sp-allergy-row.sev-severe { color: var(--red); }
        .sp-allergy-row.sev-moderate { color: var(--amber); }
        .sp-note { font-size: 13px; color: var(--text-dim); line-height: 1.4; padding: 8px; background: var(--bg-elev-2); border-radius: 6px; }
        .sp-appt-notes { margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--border); }
        .sp-appt-notes .sp-sub { cursor: pointer; font-size: 12px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; margin-bottom: 8px; }
        .sp-notes-area { width: 100%; font-family: inherit; font-size: 13px; padding: 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-elev-2); color: var(--text); resize: vertical; }
        .sp-notes-area:focus { border-color: var(--accent); outline: none; }
        .sp-save-notes { width: 100%; margin-top: 8px; padding: 8px; font-size: 13px; }
        .sp-note-hint { font-size: 12px; padding: 6px 0; }
        .sp-bottom-actions { margin-top: auto; padding-top: 20px; display: flex; gap: 8px; }
        .sp-act { flex: 1; font-size: 13px; padding: 8px; }
      `}</style>
    </aside>
  );
}

// ============ MODALS (reused) ============
function CreateOrBlockModal({ start, end, patients, onClose, onSaved }: any) {
  const [mode, setMode] = useState<"appt" | "block">("appt");
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 440 }}>
        <div className="cob-tabs">
          <button className={mode === "appt" ? "cob-tab on" : "cob-tab"} onClick={() => setMode("appt")}>📅 Book Appointment</button>
          <button className={mode === "block" ? "cob-tab on" : "cob-tab"} onClick={() => setMode("block")}>🚫 Block Time</button>
        </div>
        {mode === "appt"
          ? <CreateApptBody start={start} end={end} patients={patients} onClose={onClose} onSaved={onSaved} />
          : <BlockBody start={start} end={end} onClose={onClose} onSaved={onSaved} />}
        <style>{`
          .cob-tabs { display: flex; gap: 4px; margin-bottom: 18px; background: var(--bg-elev-2); border-radius: 8px; padding: 3px; }
          .cob-tab { flex: 1; padding: 8px; border-radius: 6px; font-size: 13px; font-weight: 600; color: var(--text-dim); background: transparent; }
          .cob-tab.on { background: var(--accent); color: white; }
        `}</style>
      </div>
    </div>
  );
}

function BlockBody({ start, end, onClose, onSaved }: any) {
  const [reason, setReason] = useState("Lunch");
  const [practitioner, setPractitioner] = useState("");
  const [startT, setStartT] = useState(format(start, "HH:mm"));
  const [endT, setEndT] = useState(format(end, "HH:mm"));
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    const s = new Date(start); s.setHours(+startT.slice(0, 2), +startT.slice(3, 5), 0, 0);
    const e = new Date(start); e.setHours(+endT.slice(0, 2), +endT.slice(3, 5), 0, 0);
    await blockedTimes.create({ start_at: fmt(s), end_at: fmt(e), reason, practitioner: practitioner || undefined });
    setSaving(false); onSaved();
  };
  return (
    <div>
      <p className="muted">{format(start, "EEE d MMM")}</p>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        <div><label style={LAB}>Start</label><input type="time" value={startT} onChange={(e) => setStartT(e.target.value)} /></div>
        <div><label style={LAB}>End</label><input type="time" value={endT} onChange={(e) => setEndT(e.target.value)} /></div>
      </div>
      <label style={{ ...LAB, marginTop: 12 }}>Reason</label>
      <input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Lunch, Meeting, Leave" autoFocus />
      <label style={{ ...LAB, marginTop: 12 }}>Practitioner (optional)</label>
      <input value={practitioner} onChange={(e) => setPractitioner(e.target.value)} placeholder="e.g. Dr. Chapman-Davies" />
      <div className="modal-actions">
        <button className="btn-ghost" onClick={onClose}>Cancel</button>
        <button className="btn-primary" onClick={save} disabled={saving || !reason}>{saving ? "Blocking…" : "🚫 Block Time"}</button>
      </div>
    </div>
  );
}

// Default duration (minutes) per appointment type.
const TYPE_DURATIONS: Record<string, number> = {
  "Dry Eye Consultation": 60,
  "Follow-up": 30,
  "IPL Treatment": 45,
  "Imaging": 30,
  "Telehealth": 20,
};

function CreateApptBody({ start, end, patients, onClose, onSaved }: any) {
  const [pid, setPid] = useState<number | "">("");
  const [type, setType] = useState("Dry Eye Consultation");
  const [search, setSearch] = useState("");
  // The user's dragged length (rounded to a 15-min slot). If they only clicked
  // (default 30-min stub created on mouse-down) treat the type default as the source of truth.
  const draggedMin = Math.max(15, differenceInMinutes(end, start));
  const userDragged = draggedMin > 30; // > the default single-click stub → an intentional drag
  // Duration state: seeded from the drag if intentional, else the type default.
  const [duration, setDuration] = useState<number>(userDragged ? draggedMin : (TYPE_DURATIONS[type] ?? 30));
  // Track whether the user manually overrode the duration; if not, changing the
  // type refreshes the duration to that type's default.
  const [durTouched, setDurTouched] = useState(userDragged);
  const filtered = patients.filter((p: Patient) => `${p.first_name} ${p.last_name} ${p.mrn}`.toLowerCase().includes(search.toLowerCase())).slice(0, 8);
  const [saving, setSaving] = useState(false);

  const onTypeChange = (t: string) => {
    setType(t);
    if (!durTouched) setDuration(TYPE_DURATIONS[t] ?? 30);
  };

  const save = async () => {
    if (!pid) return;
    setSaving(true);
    await apptApi.create({
      patient_id: pid,
      appointment_type: type,
      appointment_date: fmt(start),            // exact dragged/selected start time
      duration_minutes: Math.max(15, duration), // type default or the length the user chose
    });
    setSaving(false); onSaved();
  };

  const apptEnd = addMinutes(start, Math.max(15, duration));
  return (
    <div>
      <p className="muted">{format(start, "EEE d MMM, HH:mm")} – {format(apptEnd, "HH:mm")} ({Math.max(15, duration)} min)</p>
      <label style={LAB}>Patient</label>
      <input placeholder="🔍 Search patient…" value={search} onChange={(e) => setSearch(e.target.value)} style={{ marginBottom: 8 }} autoFocus />
      {search && filtered.map((p: Patient) => (
        <div key={p.id} className={`pat-pick ${pid === p.id ? "sel" : ""}`} onClick={() => { setPid(p.id); setSearch(`${p.first_name} ${p.last_name}`); }}>
          {p.first_name} {p.last_name} <span className="mono">{p.mrn}</span>
        </div>
      ))}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        <div>
          <label style={{ ...LAB, marginTop: 12 }}>Type</label>
          <select value={type} onChange={(e) => onTypeChange(e.target.value)}>
            <option>Dry Eye Consultation</option><option>Follow-up</option><option>IPL Treatment</option>
            <option>Imaging</option><option>Telehealth</option>
          </select>
        </div>
        <div>
          <label style={{ ...LAB, marginTop: 12 }}>Duration</label>
          <select value={Math.max(15, duration)} onChange={(e) => { setDuration(+e.target.value); setDurTouched(true); }}>
            {[15, 20, 30, 45, 60, 90, 120].map((m) => <option key={m} value={m}>{m} min</option>)}
          </select>
        </div>
      </div>
      <div className="modal-actions">
        <button className="btn-ghost" onClick={onClose}>Cancel</button>
        <button className="btn-primary" onClick={save} disabled={saving || !pid}>{saving ? "Saving…" : "Book"}</button>
      </div>
    </div>
  );
}

function MassBlockModal({ onClose, onSaved }: any) {
  const [start, setStart] = useState(format(new Date(), "yyyy-MM-dd"));
  const [end, setEnd] = useState(format(addDays(new Date(), 6), "yyyy-MM-dd"));
  const [dailyStart, setDailyStart] = useState("12:00");
  const [dailyEnd, setDailyEnd] = useState("13:00");
  const [reason, setReason] = useState("Lunch");
  const [days, setDays] = useState({ mon: true, tue: true, wed: true, thu: true, fri: true, sat: false, sun: false });
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    const dowMap: Record<string, number> = { sun: 0, mon: 1, tue: 2, wed: 3, thu: 4, fri: 5, sat: 6 };
    const s = new Date(start), e = new Date(end);
    const tasks: Promise<any>[] = [];
    for (let d = new Date(s); d <= e; d = addDays(d, 1)) {
      if (Object.entries(days).find(([k, v]) => v && dowMap[k] === d.getDay())) {
        tasks.push(blockedTimes.create({ start_at: `${format(d, "yyyy-MM-dd")} ${dailyStart}:00`, end_at: `${format(d, "yyyy-MM-dd")} ${dailyEnd}:00`, reason }));
      }
    }
    await Promise.all(tasks);
    setSaving(false); onSaved();
  };
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="card modal" onClick={(e) => e.stopPropagation()} style={{ width: 440 }}>
        <h2>🗓 Mass Block Time</h2>
        <p className="muted">Block the same time range across multiple days.</p>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
          <div><label style={LAB}>From date</label><input type="date" value={start} onChange={(e) => setStart(e.target.value)} /></div>
          <div><label style={LAB}>To date</label><input type="date" value={end} onChange={(e) => setEnd(e.target.value)} /></div>
          <div><label style={LAB}>Daily start</label><input type="time" value={dailyStart} onChange={(e) => setDailyStart(e.target.value)} /></div>
          <div><label style={LAB}>Daily end</label><input type="time" value={dailyEnd} onChange={(e) => setDailyEnd(e.target.value)} /></div>
        </div>
        <label style={{ ...LAB, marginTop: 12 }}>Reason</label>
        <input value={reason} onChange={(e) => setReason(e.target.value)} style={{ marginBottom: 12 }} />
        <label style={LAB}>Days of week</label>
        <div className="dow-row">
          {(["mon","tue","wed","thu","fri","sat","sun"] as const).map((d) => (
            <button key={d} className={days[d] ? "dow on" : "dow"} onClick={() => setDays({ ...days, [d]: !days[d] })}>{d.toUpperCase()}</button>
          ))}
        </div>
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn-primary" onClick={save} disabled={saving}>{saving ? "Blocking…" : "Block All"}</button>
        </div>
      </div>
    </div>
  );
}

// helpers
function setHour(d: Date, h: number) { const x = new Date(d); x.setHours(h, 0, 0, 0); return x; }
function fmt(d: Date) { return format(d, "yyyy-MM-dd HH:mm:ss"); }
const LAB: React.CSSProperties = { display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-dim)", marginBottom: 5, textTransform: "uppercase", letterSpacing: 0.5 };

const CAL_STYLE = `
.cal-page { display: flex; flex-direction: column; height: 100%; }
.cal-toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; gap: 12px; flex-wrap: wrap; }
.cal-nav { display: flex; align-items: center; gap: 8px; }
.cnav { background: var(--bg-elev); color: var(--text-dim); border: 1px solid var(--border); padding: 8px 14px; border-radius: 8px; font-size: 16px; }
.cnav:hover { color: var(--text); border-color: var(--accent); }
.cnav.today-btn { font-size: 13px; font-weight: 600; }
.cal-title { font-size: 20px; font-weight: 700; margin-left: 8px; }
.cal-right { display: flex; gap: 8px; align-items: center; }
.vtabs { display: flex; gap: 2px; background: var(--bg-elev); border: 1px solid var(--border); border-radius: 8px; padding: 2px; }
.vtab { padding: 6px 14px; border-radius: 6px; font-size: 13px; font-weight: 600; color: var(--text-dim); background: transparent; }
.vtab.on { background: var(--accent); color: white; }
.cal-layout { display: flex; flex: 1; min-height: 0; gap: 16px; }
.cal-main { flex: 1; min-width: 0; overflow: auto; }
.legend { display: flex; gap: 10px; align-items: center; margin-top: 12px; flex-wrap: wrap; }
.legend-paid { display: flex; align-items: center; gap: 5px; font-size: 12px; color: var(--text-dim); }
.dot-green { width: 8px; height: 8px; border-radius: 50%; background: var(--green); display: inline-block; }
.dot-amber { width: 8px; height: 8px; border-radius: 50%; background: var(--amber); display: inline-block; }
.cal-hint { color: var(--text-dim); font-size: 12px; margin-left: auto; }

/* Month */
.month-view { background: var(--bg-elev); border: 1px solid var(--border); border-radius: 12px; overflow: hidden; }
.month-dows { display: grid; grid-template-columns: repeat(7, 1fr); border-bottom: 1px solid var(--border); }
.dow-cell { padding: 10px; text-align: center; font-size: 11px; font-weight: 700; color: var(--text-dim); text-transform: uppercase; letter-spacing: 1px; }
.month-grid { display: grid; grid-template-columns: repeat(7, 1fr); grid-auto-rows: 1fr; min-height: 500px; }
.mcell { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); padding: 6px; cursor: pointer; min-height: 80px; display: flex; flex-direction: column; gap: 3px; }
.mcell:hover { background: var(--bg-elev-2); }
.mcell.other { opacity: 0.4; }
.mcell.today { background: rgba(79,156,249,0.08); }
.mcell.today .mday { color: var(--accent); font-weight: 800; }
.mday { font-size: 13px; font-weight: 600; }
.mevs { display: flex; flex-direction: column; gap: 2px; overflow: hidden; }
.mpill { font-size: 10px; padding: 2px 5px; border-radius: 3px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
.mpill.ev-appointment { background: var(--accent-soft); color: var(--accent); }
.mpill.ev-appointment.confirmed { background: rgba(74,222,128,0.15); color: var(--green); }
.mpill.ev-blocked { background: rgba(251,191,36,0.15); color: var(--amber); }
.mpill-time { font-weight: 700; margin-right: 4px; }
.mpill-more { font-size: 10px; color: var(--text-dim); padding: 0 4px; }

/* Week */
.week-view { background: var(--bg-elev); border: 1px solid var(--border); border-radius: 12px; overflow: auto; }
.week-grid { display: grid; grid-template-columns: 60px repeat(7, 1fr); min-width: 700px; user-select: none; }
.week-corner { border-bottom: 1px solid var(--border); }
.week-head { text-align: center; padding: 10px 4px; border-bottom: 1px solid var(--border); }
.week-head.today .wh-dom { background: var(--accent); color: white; border-radius: 50%; width: 26px; height: 26px; line-height: 26px; margin: 0 auto; }
.wh-dow { font-size: 11px; text-transform: uppercase; color: var(--text-dim); }
.wh-dom { font-size: 16px; font-weight: 600; margin-top: 2px; }
.week-time { font-size: 11px; color: var(--text-dim); padding: 6px 8px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); text-align: right; }
.week-cell { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); height: 84px; position: relative; cursor: cell; }

/* Day */
.day-view { background: var(--bg-elev); border: 1px solid var(--border); border-radius: 12px; overflow: auto; }
.day-grid { display: grid; grid-template-columns: 60px 1fr; user-select: none; max-width: 600px; margin: 0 auto; }
.day-corner { grid-column: 1 / -1; padding: 12px; text-align: center; font-weight: 700; border-bottom: 1px solid var(--border); }
.day-row { display: contents; }
.day-time { font-size: 11px; color: var(--text-dim); padding: 6px 8px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); text-align: right; }
.day-cell { border-bottom: 1px solid var(--border); height: 56px; position: relative; cursor: cell; }

/* Events */
.ev { position: absolute; left: 2px; right: 2px; padding: 3px 6px; border-radius: 5px; font-size: 11px; overflow: hidden; z-index: 2; }
.ev.draggable { cursor: grab; }
.ev.draggable:active { cursor: grabbing; }
.ev-appointment { background: var(--accent-soft); color: var(--accent); border-left: 3px solid var(--accent); }
.ev-appointment.confirmed { background: rgba(74,222,128,0.15); color: var(--green); border-left-color: var(--green); }
.ev-appointment.paid-paid { border-left-color: var(--green); }
.ev-appointment.paid-partial { border-left-color: var(--amber); }
.ev-blocked { background: rgba(251,191,36,0.15); color: var(--amber); border-left: 3px solid var(--amber); }
.ev-draft { background: var(--accent-soft); color: var(--accent); border: 2px dashed var(--accent); opacity: 0.7; }
.ev-hover { background: rgba(79,156,249,0.1); border: 2px dashed var(--accent); opacity: 0.6; }
.ev-time { font-weight: 700; }
.ev-dur { font-weight: 400; opacity: 0.8; }
.ev-title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ev-end { font-size: 10px; opacity: 0.7; margin-top: 1px; }
.ev-paid { position: absolute; top: 3px; right: 4px; font-size: 9px; font-weight: 700; padding: 1px 5px; border-radius: 3px; }
.ev-paid.ok { background: var(--green); color: #000; }
.ev-paid.part { background: var(--amber); color: #000; }

/* Modals */
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 100; }
.modal { width: 460px; max-height: 90vh; overflow-y: auto; }
.modal h2 { margin-bottom: 8px; }
.muted { color: var(--text-dim); font-size: 13px; margin-bottom: 16px; }
.pat-pick { padding: 8px 10px; border: 1px solid var(--border); border-radius: 6px; margin-bottom: 4px; cursor: pointer; font-size: 14px; }
.pat-pick:hover { background: var(--bg-elev-2); }
.pat-pick.sel { border-color: var(--accent); background: var(--accent-soft); }
.mono { font-family: ui-monospace, monospace; font-size: 11px; color: var(--text-dim); }
.modal-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 16px; }
.dow-row { display: flex; gap: 6px; margin-top: 6px; }
.dow { padding: 6px 10px; border-radius: 6px; background: var(--bg-elev-2); color: var(--text-dim); border: 1px solid var(--border); font-size: 12px; }
.dow.on { background: var(--accent); color: white; border-color: var(--accent); }
`;