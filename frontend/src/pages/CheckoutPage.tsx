import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  billing, patientDetail,
  type ConsultationType, type ServiceItem, type PatientDetail as PDetail,
} from "../api";

export function CheckoutPage() {
  const { id } = useParams();
  const nav = useNavigate();
  const pid = Number(id);
  const [data, setData] = useState<PDetail | null>(null);
  const [ctypes, setCtypes] = useState<ConsultationType[]>([]);
  const [services, setServices] = useState<ServiceItem[]>([]);
  const [cat, setCat] = useState("all");
  const [cart, setCart] = useState<CartItem[]>([]);
  const [method, setMethod] = useState("card");
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [loadErr, setLoadErr] = useState("");

  useEffect(() => {
    Promise.all([patientDetail.get(pid), billing.consultationTypes(), billing.services()])
      .then(([pd, ct, sv]) => { setData(pd.data); setCtypes(ct.data); setServices(sv.data); })
      .catch((e) => setLoadErr(e?.response?.data?.error || "Failed to load checkout data"))
      .finally(() => setSaving(false));
  }, [pid]);

  const cats = ["all", ...new Set(services.map((s) => s.category))];
  const shownServices = cat === "all" ? services : services.filter((s) => s.category === cat);

  const addCtype = (c: ConsultationType) => setCart([...cart, { kind: "consultation", desc: c.type_name, qty: 1, price: c.default_price, disc: 0, tax: 0.1 }]);
  const addSvc = (s: ServiceItem) => setCart([...cart, { kind: "service", desc: s.service_name, qty: 1, price: s.unit_price, disc: 0, tax: s.tax_rate }]);
  const removeItem = (i: number) => setCart(cart.filter((_, idx) => idx !== i));
  const setItem = (i: number, patch: Partial<CartItem>) => setCart(cart.map((it, idx) => idx === i ? { ...it, ...patch } : it));

  // Tax-exclusive: line net = price * qty * (1 - disc/100); tax computed on net.
  const lineNet = (it: CartItem) => it.price * it.qty * (1 - it.disc / 100);
  const subtotal = cart.reduce((s, it) => s + lineNet(it), 0);
  const tax = cart.reduce((s, it) => s + lineNet(it) * it.tax, 0);
  const total = subtotal + tax;

  const checkout = async () => {
    if (cart.length === 0) return;
    setSaving(true); setErr("");
    try {
      const inv = await billing.createInvoice({
        patient_id: pid,
        payment_method: method,
        notes,
        items: cart.map((it) => ({ item_type: it.kind, description: it.desc, quantity: it.qty, unit_price: it.price, discount_percent: it.disc, tax_rate: it.tax })),
      });
      if (method) {
        await billing.addPayment({ invoice_id: inv.data.id, amount: total, payment_method: method });
      }
      setDone(inv.data.invoice_number);
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Checkout failed. Please try again.");
    } finally {
      setSaving(false);
    }
  };

  if (done) {
    return (
      <div className="card done-card">
        <div className="done-check">✓</div>
        <h2>Payment Complete</h2>
        <p className="muted">Invoice {done} · ${total.toFixed(2)} paid via {method}</p>
        <div className="done-actions">
          <button className="btn-ghost" onClick={() => nav(`/patients/${pid}`)}>Back to Patient</button>
          <button className="btn-primary" onClick={() => nav("/")}>Dashboard</button>
        </div>
        <style>{`.done-card{text-align:center;max-width:420px;margin:60px auto}.done-check{font-size:64px;color:var(--green)}.muted{color:var(--text-dim);margin:8px 0 24px}.done-actions{display:flex;gap:10px;justify-content:center}`}</style>
      </div>
    );
  }

  if (loadErr) return <div className="card empty" style={{ maxWidth: 420, margin: "60px auto", color: "var(--red)" }}>{loadErr}</div>;
  if (!data) return <div className="empty">Loading…</div>;
  const p = data.patient;

  return (
    <div>
      <button className="btn-ghost back-btn" onClick={() => nav(`/patients/${pid}`)}>‹ Back to {p.last_name}</button>
      <h1 className="page-title">Checkout — {p.first_name} {p.last_name}</h1>

      <div className="co-grid">
        {/* Left: catalog */}
        <div className="co-left">
          <div className="card">
            <h3 className="card-h">Consultations</h3>
            <div className="cat-grid">
              {ctypes.map((c) => (
                <button key={c.id} className="cat-card" onClick={() => addCtype(c)}>
                  <div className="cat-name">{c.type_name}</div>
                  <div className="cat-meta">{c.default_duration_minutes}min · ${c.default_price}</div>
                </button>
              ))}
            </div>
          </div>
          <div className="card" style={{ marginTop: 16 }}>
            <div className="co-cat-head">
              <h3 className="card-h" style={{ marginBottom: 0 }}>Services & Products</h3>
              <select value={cat} onChange={(e) => setCat(e.target.value)} style={{ width: "auto" }}>
                {cats.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>
            <div className="cat-grid">
              {shownServices.map((s) => (
                <button key={s.id} className="cat-card" onClick={() => addSvc(s)}>
                  <div className="cat-name">{s.service_name}</div>
                  <div className="cat-meta">{s.category} · ${s.unit_price}</div>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Right: cart */}
        <div className="card co-cart">
          <h3 className="card-h">Invoice Items</h3>
          {cart.length === 0 ? <div className="empty-sm">Click items on the left to add them.</div> :
            cart.map((it, i) => (
              <div key={i} className="cart-row">
                <div className="cart-desc">{it.desc}</div>
                <input className="cart-num" type="number" min="1" value={it.qty} onChange={(e) => setItem(i, { qty: Number(e.target.value) })} />
                <span className="cart-x">×</span>
                <span className="cart-price">${it.price.toFixed(2)}</span>
                <input className="cart-num small" type="number" min="0" max="100" value={it.disc} onChange={(e) => setItem(i, { disc: Number(e.target.value) })} title="discount %" />
                <span className="cart-line">${lineNet(it).toFixed(2)}</span>
                <button className="mini danger" onClick={() => removeItem(i)}>×</button>
              </div>
            ))
          }
          <div className="cart-totals">
            <div className="tot-row"><span>Subtotal</span><span>${subtotal.toFixed(2)}</span></div>
            <div className="tot-row"><span>Tax (GST)</span><span>${tax.toFixed(2)}</span></div>
            <div className="tot-row total"><span>Total</span><span>${total.toFixed(2)}</span></div>
          </div>
          <label style={LAB}>Payment Method</label>
          <select value={method} onChange={(e) => setMethod(e.target.value)} style={{ marginBottom: 12 }}>
            <option value="card">Card</option><option value="cash">Cash</option>
            <option value="eftpos">EFTPOS</option><option value="medicare">Medicare</option><option value="insurance">Insurance</option>
          </select>
          <textarea rows={2} placeholder="Notes (optional)" value={notes} onChange={(e) => setNotes(e.target.value)} style={{ marginBottom: 12 }} />
          {err && <div style={{ color: "var(--red)", fontSize: 13, marginBottom: 10 }}>{err}</div>}
          <button className="btn-primary co-pay" disabled={saving || cart.length === 0} onClick={checkout}>
            {saving ? "Processing…" : `Process Payment · $${total.toFixed(2)}`}
          </button>
        </div>
      </div>

      <style>{`
        .back-btn { margin-bottom: 12px; }
        .page-title { font-size: 24px; margin-bottom: 20px; }
        .co-grid { display: grid; grid-template-columns: 1.4fr 1fr; gap: 16px; }
        .card-h { font-size: 14px; font-weight: 600; margin-bottom: 14px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; }
        .cat-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
        .cat-card { text-align: left; background: var(--bg-elev-2); border: 1px solid var(--border); border-radius: 8px; padding: 10px 12px; color: var(--text); }
        .cat-card:hover { border-color: var(--accent); background: var(--accent-soft); }
        .cat-name { font-weight: 600; font-size: 14px; }
        .cat-meta { font-size: 12px; color: var(--text-dim); margin-top: 2px; }
        .co-cat-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; }
        .empty-sm { padding: 16px; text-align: center; color: var(--text-dim); font-size: 13px; }
        .cart-row { display: flex; align-items: center; gap: 6px; padding: 8px 0; border-bottom: 1px solid var(--border); font-size: 13px; }
        .cart-desc { flex: 1; font-weight: 500; }
        .cart-num { width: 50px; padding: 4px 6px; font-size: 12px; }
        .cart-num.small { width: 42px; }
        .cart-x { color: var(--text-dim); }
        .cart-price { width: 56px; text-align: right; }
        .cart-line { width: 64px; text-align: right; font-weight: 600; }
        .mini.danger { color: var(--red); padding: 2px 8px; }
        .cart-totals { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border); }
        .tot-row { display: flex; justify-content: space-between; padding: 4px 0; font-size: 14px; color: var(--text-dim); }
        .tot-row.total { font-size: 18px; font-weight: 700; color: var(--text); padding-top: 10px; }
        .co-pay { width: 100%; padding: 12px; font-size: 16px; margin-top: 8px; }
      `}</style>
    </div>
  );
}

interface CartItem { kind: string; desc: string; qty: number; price: number; disc: number; tax: number; }
const LAB: React.CSSProperties = { display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-dim)", marginBottom: 5, textTransform: "uppercase", letterSpacing: 0.5 };
