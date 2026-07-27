/**
 * OptiCore Booking Worker — the public booking gateway.
 *
 * This sits between the public website intake form and the clinic's desktop app.
 * Patients submit bookings here; the desktop app syncs (pulls bookings, pushes
 * availability) every ~30 seconds.
 *
 * Endpoints:
 *   PUBLIC (no auth):
 *     GET  /api/public/appointment-types  — bookable types + prices
 *     GET  /api/public/availability       — published slot availability
 *     POST /api/public/book               — submit a booking request
 *
 *   SYNC (shared-secret auth):
 *     POST /api/sync/pull                 — desktop pulls pending bookings
 *     POST /api/sync/push                 — desktop pushes availability/types
 *     GET  /api/sync/status               — sync health + counts
 */

const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json', ...CORS },
  });
}

async function checkAuth(request, env) {
  const auth = request.headers.get('Authorization');
  if (!auth || !auth.startsWith('Bearer ')) return false;
  const token = auth.slice(7);
  // In production this is a per-clinic shared secret set via wrangler secret
  return token === (env.SYNC_SECRET || 'dev-sync-secret');
}

// ---- D1 schema initialization ----
const SCHEMA_SQL = `
CREATE TABLE IF NOT EXISTS bookings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  submitted_at TEXT DEFAULT (datetime('now')),
  first_name TEXT NOT NULL,
  last_name TEXT NOT NULL,
  date_of_birth TEXT,
  phone TEXT,
  email TEXT,
  address TEXT,
  preferred_date TEXT,
  preferred_time TEXT,
  appointment_type TEXT,
  symptoms TEXT,
  is_returning INTEGER DEFAULT 0,
  status TEXT DEFAULT 'pending',
  synced INTEGER DEFAULT 0,
  clinic_response TEXT
);
CREATE TABLE IF NOT EXISTS published_slots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date TEXT NOT NULL,
  time TEXT NOT NULL,
  available INTEGER DEFAULT 1,
  updated_at TEXT DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS appointment_types (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  code TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  price REAL DEFAULT 0,
  duration_minutes INTEGER DEFAULT 30,
  description TEXT
);
CREATE TABLE IF NOT EXISTS sync_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event TEXT NOT NULL,
  detail TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);
`;

async function initDB(env) {
  const stmts = SCHEMA_SQL.split(';').filter(s => s.trim()).map(s => ({ sql: s + ';' }));
  await env.DB.batch(stmts.map(s => env.DB.prepare(s.sql)));
}

// ---- Seed default appointment types ----
const SEED_TYPES = [
  { code: 'DRY-EYE', name: 'Dry Eye Consultation', price: 350, duration: 60, desc: 'Comprehensive dry eye assessment' },
  { code: 'FOLLOWUP', name: 'Follow-up', price: 150, duration: 30, desc: 'Review consultation' },
  { code: 'IPL', name: 'IPL Treatment', price: 300, duration: 45, desc: 'Intense Pulsed Light therapy' },
  { code: 'IMAGING', name: 'Imaging', price: 250, duration: 30, desc: 'Diagnostic imaging session' },
  { code: 'TELEHEALTH', name: 'Telehealth', price: 120, duration: 20, desc: 'Remote consultation' },
];

async function seedTypes(env) {
  for (const t of SEED_TYPES) {
    await env.DB.prepare(
      'INSERT OR IGNORE INTO appointment_types (code, name, price, duration_minutes, description) VALUES (?, ?, ?, ?, ?)'
    ).bind(t.code, t.name, t.price, t.duration, t.desc).run();
  }
}

// ---- Main handler ----
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const method = request.method;
    const path = url.pathname;

    // CORS preflight
    if (method === 'OPTIONS') return new Response(null, { headers: CORS });

    // Ensure DB exists (idempotent)
    try { await initDB(env); await seedTypes(env); } catch (e) { /* already exists */ }

    // ---- PUBLIC ENDPOINTS ----

    // GET /api/public/appointment-types
    if (path === '/api/public/appointment-types' && method === 'GET') {
      const { results } = await env.DB.prepare(
        'SELECT code, name, price, duration_minutes, description FROM appointment_types ORDER BY price'
      ).all();
      return json(results.map(t => ({
        code: t.code, name: t.name, price: t.price, duration: t.duration_minutes, description: t.description,
      })));
    }

    // GET /api/public/availability?days=14
    if (path === '/api/public/availability' && method === 'GET') {
      const days = parseInt(url.searchParams.get('days') || '14');
      const { results } = await env.DB.prepare(
        'SELECT date, time, available FROM published_slots WHERE date >= date("now") AND date <= date("now", ?) ORDER BY date, time'
      ).bind(`+${days} days`).all();

      // If no published slots, generate defaults (weekdays 9-5)
      if (!results.length) {
        const slots = [];
        for (let d = 1; d <= days; d++) {
          const date = new Date(); date.setDate(date.getDate() + d);
          const dow = date.getDay();
          if (dow === 0 || dow === 6) continue; // skip weekends
          const ds = date.toISOString().slice(0, 10);
          for (let h = 9; h <= 16; h++) {
            slots.push({ date: ds, time: `${String(h).padStart(2, '0')}:00`, available: true });
          }
        }
        return json(slots);
      }
      return json(results);
    }

    // POST /api/public/book
    if (path === '/api/public/book' && method === 'POST') {
      let body;
      try { body = await request.json(); } catch { return json({ error: 'Invalid JSON' }, 400); }

      if (!body.first_name || !body.last_name) {
        return json({ error: 'First name and last name are required' }, 400);
      }

      const result = await env.DB.prepare(
        `INSERT INTO bookings (first_name, last_name, date_of_birth, phone, email, address, preferred_date, preferred_time, appointment_type, symptoms, is_returning)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      ).bind(
        body.first_name, body.last_name, body.date_of_birth || null, body.phone || null,
        body.email || null, body.address || null, body.preferred_date || null,
        body.preferred_time || null, body.appointment_type || null, body.symptoms || null,
        body.is_returning ? 1 : 0
      ).run();

      // Mark the slot as provisionally booked
      if (body.preferred_date && body.preferred_time) {
        await env.DB.prepare(
          'UPDATE published_slots SET available = 0, updated_at = datetime("now") WHERE date = ? AND time = ?'
        ).bind(body.preferred_date, body.preferred_time).run();
      }

      return json({
        ok: true,
        booking_id: result.meta.last_row_id,
        message: 'Booking request received. The clinic will confirm shortly.',
      }, 201);
    }

    // ---- SYNC ENDPOINTS (shared-secret auth) ----

    if (path.startsWith('/api/sync/')) {
      if (!(await checkAuth(request, env))) {
        return json({ error: 'Unauthorized' }, 401);
      }

      // POST /api/sync/pull — desktop pulls pending bookings
      if (path === '/api/sync/pull' && method === 'POST') {
        const { results } = await env.DB.prepare(
          'SELECT * FROM bookings WHERE synced = 0 AND status = "pending" ORDER BY submitted_at'
        ).all();

        // Mark as synced
        if (results.length) {
          const ids = results.map(r => r.id);
          await env.DB.prepare(
            `UPDATE bookings SET synced = 1 WHERE id IN (${ids.map(() => '?').join(',')})`
          ).bind(...ids).run();
        }

        await env.DB.prepare('INSERT INTO sync_log (event, detail) VALUES ("pull", ?)')
          .bind(`${results.length} bookings pulled`).run();

        return json({ bookings: results, count: results.length });
      }

      // POST /api/sync/push — desktop pushes availability + appointment types
      if (path === '/api/sync/push' && method === 'POST') {
        const body = await request.json();

        // Push slots
        if (body.slots) {
          await env.DB.prepare('DELETE FROM published_slots').run();
          for (const s of body.slots) {
            await env.DB.prepare(
              'INSERT INTO published_slots (date, time, available) VALUES (?, ?, ?)'
            ).bind(s.date, s.time, s.available ? 1 : 0).run();
          }
        }

        // Push appointment types
        if (body.appointment_types) {
          await env.DB.prepare('DELETE FROM appointment_types').run();
          for (const t of body.appointment_types) {
            await env.DB.prepare(
              'INSERT INTO appointment_types (code, name, price, duration_minutes, description) VALUES (?, ?, ?, ?, ?)'
            ).bind(t.code, t.name, t.price, t.duration, t.description).run();
          }
        }

        // Confirm/cancel bookings
        if (body.confirmations) {
          for (const c of body.confirmations) {
            await env.DB.prepare(
              'UPDATE bookings SET status = ?, clinic_response = ? WHERE id = ?'
            ).bind(c.status, c.response || null, c.id).run();
          }
        }

        await env.DB.prepare('INSERT INTO sync_log (event, detail) VALUES ("push", ?)')
          .bind(`slots:${body.slots?.length || 0} types:${body.appointment_types?.length || 0}`).run();

        return json({ ok: true, pushed_at: new Date().toISOString() });
      }

      // GET /api/sync/status
      if (path === '/api/sync/status' && method === 'GET') {
        const pending = await env.DB.prepare('SELECT COUNT(*) as c FROM bookings WHERE synced = 0').first();
        const total = await env.DB.prepare('SELECT COUNT(*) as c FROM bookings').first();
        const lastSync = await env.DB.prepare('SELECT created_at FROM sync_log ORDER BY id DESC LIMIT 1').first();
        return json({
          pending_bookings: pending.c,
          total_bookings: total.c,
          last_sync: lastSync?.created_at || null,
        });
      }
    }

    // ---- Health ----
    if (path === '/' || path === '/health') {
      return json({ ok: true, service: 'opticore-booking-worker', version: '0.1.0' });
    }

    return json({ error: 'Not found' }, 404);
  },

  // ---- Scheduled handler (cron) — sends day-before reminders ----
  async scheduled(event, env, ctx) {
    ctx.waitUntil(sendReminders(env));
  },
};

/**
 * Send reminder SMS/email for appointments happening tomorrow.
 * Called hourly by Cloudflare cron — checks if any confirmed bookings are
 * within the next 24h and sends a reminder if one hasn't been sent yet.
 */
async function sendReminders(env) {
  // Get bookings confirmed for tomorrow that haven't had a reminder sent
  const { results } = await env.DB.prepare(
    `SELECT * FROM bookings
     WHERE status = 'confirmed'
       AND preferred_date = date('now', '+1 day')
       AND clinic_response NOT LIKE '%reminder_sent%'`
  ).all();

  if (!results.length) return;

  const settings = await env.DB.prepare('SELECT * FROM booking_settings WHERE id = 1').first().catch(() => null);

  for (const booking of results) {
    const name = booking.first_name;
    const time = booking.preferred_time || '';
    const type = booking.appointment_type || 'appointment';
    const tmpl = settings?.template_reminder || 'Reminder: {{name}}, appointment tomorrow at {{time}} ({{type}}). Reply STOP to opt out. — OptiCore';
    const body = tmpl.replace(/\{\{name\}\}/g, name).replace(/\{\{time\}\}/g, time).replace(/\{\{type\}\}/g, type).replace(/\{\{date\}\}/g, booking.preferred_date || '');

    // Send via SMS if phone available, else email
    if (booking.phone && settings?.sms_api_key) {
      try {
        await sendClickSendSMS(env, booking.phone, body, settings);
      } catch (e) { /* log but continue */ }
    } else if (booking.email && settings?.email_api_key) {
      try {
        await sendPostmarkEmail(env, booking.email, 'Appointment Reminder', body, settings);
      } catch (e) { /* log but continue */ }
    }

    // Mark reminder as sent
    await env.DB.prepare('UPDATE bookings SET clinic_response = COALESCE(clinic_response, "") || "reminder_sent;" WHERE id = ?')
      .bind(booking.id).run();
  }
}

async function sendClickSendSMS(env, phone, body, settings) {
  const username = settings.sms_username || '';
  const apiKey = settings.sms_api_key;
  const normalised = phone.replace(/\s|-/g, '').replace(/^0/, '+61');
  await fetch('https://rest.clicksend.com/v3/sms/send', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': 'Basic ' + btoa(username + ':' + apiKey),
    },
    body: JSON.stringify({ messages: [{ source: 'opticore', from: settings.sms_sender || 'OptiCore', to: normalised, body }] }),
  });
}

async function sendPostmarkEmail(env, to, subject, body, settings) {
  await fetch('https://api.postmarkapp.com/email', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Postmark-Server-Token': settings.email_api_key,
    },
    body: JSON.stringify({ From: settings.email_from || 'bookings@clinic.local', To: to, Subject: subject, TextBody: body }),
  });
}
