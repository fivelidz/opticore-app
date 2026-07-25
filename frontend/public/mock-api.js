// Mock API layer for the static demo. Intercepts axios requests and returns
// in-memory data, so the REAL React components work without a backend.
// This is injected before the app's api.ts loads.

(function () {
  const DB = {
    users: [{
      id: 1, username: "admin", email: "admin@clinic.local", role: "admin",
      first_name: "System", last_name: "Administrator", is_active: true,
      created_at: "2026-07-01T00:00:00Z", updated_at: "2026-07-01T00:00:00Z",
      password_hash: "mock",
    }],
    patients: [
      { id: 1, mrn: "MOS-2026000001", first_name: "Sarah", last_name: "Johnson", date_of_birth: "1985-04-12", gender: "female", phone: "0412 345 678", email: "sarah.johnson@example.com", address: "12 Mosman St, Mosman NSW 2088", medicare_number: "2123 45678 1", created_at: "2026-06-01T10:00:00Z", updated_at: "2026-07-01T11:00:00Z" },
      { id: 2, mrn: "MOS-2026000002", first_name: "Michael", last_name: "Chen", date_of_birth: "1979-09-23", gender: "male", phone: "0423 456 789", email: "michael.chen@example.com", address: "45 Military Rd, Neutral Bay 2089", medicare_number: "2987 65432 1", created_at: "2026-06-15T14:00:00Z", updated_at: "2026-06-15T14:00:00Z" },
      { id: 3, mrn: "MOS-2026000003", first_name: "Emma", last_name: "Wilson", date_of_birth: "1992-12-03", gender: "female", phone: "0434 567 890", email: "emma.wilson@example.com", address: "8 Raglan St, Mosman NSW 2088", medicare_number: "3456 78901 2", created_at: "2026-07-10T09:30:00Z", updated_at: "2026-07-10T09:30:00Z" },
      { id: 4, mrn: "MOS-2026000004", first_name: "David", last_name: "Nguyen", date_of_birth: "1968-07-15", gender: "male", phone: "0445 678 901", email: "david.nguyen@example.com", address: "22 Spit Rd, Mosman NSW 2088", medicare_number: "4567 89012 3", created_at: "2026-06-12T15:00:00Z", updated_at: "2026-06-12T15:00:00Z" },
      { id: 5, mrn: "MOS-2026000005", first_name: "Olivia", last_name: "Brown", date_of_birth: "1995-02-28", gender: "female", phone: "0456 789 012", email: "olivia.brown@example.com", address: "3 Belmont Rd, Mosman NSW 2088", medicare_number: "5678 90123 4", created_at: "2026-07-20T11:00:00Z", updated_at: "2026-07-20T11:00:00Z" },
    ],
    appointments: [
      { id: 1, patient_id: 1, appointment_type: "Dry Eye Consultation", appointment_date: "2026-07-25 09:00:00", duration_minutes: 60, practitioner: "Dr. Chapman-Davies", status: "scheduled", notes: "Patient reports gritty eyes, worse in mornings. Started Restasis.", created_at: "2026-07-20T10:00:00Z" },
      { id: 2, patient_id: 3, appointment_type: "Follow-up", appointment_date: "2026-07-25 10:00:00", duration_minutes: 30, practitioner: "Regina Chapman-Davies", status: "scheduled", notes: "6-week review", created_at: "2026-07-20T10:00:00Z" },
      { id: 3, patient_id: 2, appointment_type: "IPL Treatment", appointment_date: "2026-07-25 11:00:00", duration_minutes: 45, practitioner: "Dr. Chapman-Davies", status: "confirmed", notes: "Session 2 of 4.", created_at: "2026-07-20T10:00:00Z" },
      { id: 4, patient_id: 4, appointment_type: "Dry Eye Consultation", appointment_date: "2026-07-25 14:00:00", duration_minutes: 60, practitioner: "Regina Chapman-Davies", status: "scheduled", notes: "New patient", created_at: "2026-07-20T10:00:00Z" },
      { id: 5, patient_id: 1, appointment_type: "Imaging", appointment_date: "2026-07-26 14:00:00", duration_minutes: 30, practitioner: "Dr. Chapman-Davies", status: "scheduled", notes: "Keratograph 5M", created_at: "2026-07-20T10:00:00Z" },
    ],
    blocked_times: [
      { id: 1, start_at: "2026-07-25 13:00:00", end_at: "2026-07-25 14:00:00", reason: "Lunch", practitioner: "Dr. Chapman-Davies", all_day: false, is_recurring: true, created_at: "2026-07-01T00:00:00Z" },
      { id: 2, start_at: "2026-07-26 12:00:00", end_at: "2026-07-26 13:30:00", reason: "Lunch", practitioner: "Regina Chapman-Davies", all_day: false, is_recurring: true, created_at: "2026-07-01T00:00:00Z" },
    ],
    clinical_notes: [
      { id: 1, patient_id: 1, author: "Dr. Chapman-Davies", category: "assessment", note: "Moderate dry eye disease. TBUT 5s. Started on Restasis + warm compresses.", created_at: "2026-06-01T10:30:00Z" },
      { id: 2, patient_id: 1, author: "Regina Chapman-Davies", category: "followup", note: "Improvement reported after 6 weeks. Continue current management.", created_at: "2026-07-01T11:00:00Z" },
      { id: 3, patient_id: 2, author: "Dr. Chapman-Davies", category: "treatment", note: "IPL session 2 of 4. Good response. Fluence 12 J/cm2, 15 pulses.", created_at: "2026-06-20T14:00:00Z" },
      { id: 4, patient_id: 3, author: "Regina Chapman-Davies", category: "general", note: "New patient. Mild symptoms. Baseline imaging completed.", created_at: "2026-07-10T09:30:00Z" },
    ],
    allergies: [
      { id: 1, patient_id: 2, substance: "Chloramphenicol", severity: "moderate", noted_at: "2026-06-15T00:00:00Z" },
      { id: 2, patient_id: 4, substance: "Sulfa drugs", severity: "severe", noted_at: "2026-06-12T00:00:00Z" },
    ],
    osdi_scores: [
      { id: 1, patient_id: 1, score_date: "2026-06-01", total_score: 28.5, ocular_symptoms: 30.0, vision_function: 25.0, environmental_triggers: 30.0, created_at: "2026-06-01T00:00:00Z" },
      { id: 2, patient_id: 1, score_date: "2026-07-01", total_score: 22.1, ocular_symptoms: 24.0, vision_function: 20.0, environmental_triggers: 22.5, created_at: "2026-07-01T00:00:00Z" },
      { id: 3, patient_id: 2, score_date: "2026-06-15", total_score: 35.2, ocular_symptoms: 38.0, vision_function: 32.0, environmental_triggers: 35.5, created_at: "2026-06-15T00:00:00Z" },
      { id: 4, patient_id: 3, score_date: "2026-07-10", total_score: 15.0, ocular_symptoms: 16.0, vision_function: 14.0, environmental_triggers: 15.0, created_at: "2026-07-10T00:00:00Z" },
    ],
    ipl_treatments: [
      { id: 1, patient_id: 2, treatment_date: "2026-06-20 14:00:00", session_number: 1, fluence_j_cm2: 10.0, number_of_pulses: 15, operator_name: "Dr. Chapman-Davies", clinical_notes: "First session. Tolerated well.", created_at: "2026-06-20T00:00:00Z" },
      { id: 2, patient_id: 2, treatment_date: "2026-07-04 14:00:00", session_number: 2, fluence_j_cm2: 12.0, number_of_pulses: 15, operator_name: "Dr. Chapman-Davies", clinical_notes: "Increased fluence. Good response.", created_at: "2026-07-04T00:00:00Z" },
      { id: 3, patient_id: 1, treatment_date: "2026-06-25 11:00:00", session_number: 1, fluence_j_cm2: 11.0, number_of_pulses: 14, operator_name: "Dr. Chapman-Davies", clinical_notes: "Session 1 of 4.", created_at: "2026-06-25T00:00:00Z" },
    ],
    invoices: [
      { id: 1, invoice_number: "INV-2026-0001", patient_id: 1, appointment_id: null, invoice_date: "2026-06-01 10:30:00", due_date: null, subtotal: 350, tax_amount: 35, discount_amount: 0, total_amount: 385, amount_paid: 385, balance_due: 0, status: "paid", payment_method: "card", notes: null, created_at: "2026-06-01T00:00:00Z", items: [{ id: 1, invoice_id: 1, item_type: "consultation", description: "Dry Eye Consultation", quantity: 1, unit_price: 350, discount_percent: 0, tax_rate: 0.1, total: 385 }] },
      { id: 2, invoice_number: "INV-2026-0002", patient_id: 1, appointment_id: null, invoice_date: "2026-07-01 11:00:00", due_date: null, subtotal: 150, tax_amount: 15, discount_amount: 0, total_amount: 165, amount_paid: 165, balance_due: 0, status: "paid", payment_method: "card", notes: null, created_at: "2026-07-01T00:00:00Z", items: [{ id: 2, invoice_id: 2, item_type: "consultation", description: "Follow-up", quantity: 1, unit_price: 150, discount_percent: 0, tax_rate: 0.1, total: 165 }] },
      { id: 3, invoice_number: "INV-2026-0003", patient_id: 2, appointment_id: null, invoice_date: "2026-06-20 14:00:00", due_date: null, subtotal: 580, tax_amount: 58, discount_amount: 0, total_amount: 638, amount_paid: 300, balance_due: 338, status: "partially_paid", payment_method: "eftpos", notes: null, created_at: "2026-06-20T00:00:00Z", items: [{ id: 3, invoice_id: 3, item_type: "consultation", description: "Dry Eye Consultation", quantity: 1, unit_price: 350, discount_percent: 0, tax_rate: 0.1, total: 385 }, { id: 4, invoice_id: 3, item_type: "service", description: "IPL Therapy Session", quantity: 1, unit_price: 300, discount_percent: 10, tax_rate: 0.1, total: 297 }] },
      { id: 4, invoice_number: "INV-2026-0004", patient_id: 2, appointment_id: null, invoice_date: "2026-07-04 14:00:00", due_date: null, subtotal: 300, tax_amount: 30, discount_amount: 0, total_amount: 330, amount_paid: 0, balance_due: 330, status: "issued", payment_method: null, notes: null, created_at: "2026-07-04T00:00:00Z", items: [{ id: 5, invoice_id: 4, item_type: "service", description: "IPL Therapy Session", quantity: 1, unit_price: 300, discount_percent: 0, tax_rate: 0.1, total: 330 }] },
      { id: 5, invoice_number: "INV-2026-0005", patient_id: 3, appointment_id: null, invoice_date: "2026-07-10 09:30:00", due_date: null, subtotal: 430, tax_amount: 43, discount_amount: 0, total_amount: 473, amount_paid: 473, balance_due: 0, status: "paid", payment_method: "medicare", notes: null, created_at: "2026-07-10T00:00:00Z", items: [{ id: 6, invoice_id: 5, item_type: "consultation", description: "Dry Eye Consultation", quantity: 1, unit_price: 350, discount_percent: 0, tax_rate: 0.1, total: 385 }, { id: 7, invoice_id: 5, item_type: "service", description: "Keratograph 5M Imaging", quantity: 1, unit_price: 180, discount_percent: 0, tax_rate: 0.1, total: 198 }] },
    ],
    payments: [
      { id: 1, invoice_id: 1, payment_date: "2026-06-01 10:35:00", amount: 385, payment_method: "card", reference_number: "TXN-001", notes: null, created_at: "2026-06-01T00:00:00Z" },
      { id: 2, invoice_id: 2, payment_date: "2026-07-01 11:05:00", amount: 165, payment_method: "card", reference_number: "TXN-002", notes: null, created_at: "2026-07-01T00:00:00Z" },
      { id: 3, invoice_id: 3, payment_date: "2026-06-20 14:10:00", amount: 300, payment_method: "eftpos", reference_number: "TXN-003", notes: null, created_at: "2026-06-20T00:00:00Z" },
    ],
    consultation_types: [
      { id: 1, type_code: "DRY-EYE", type_name: "Dry Eye Consultation", description: "Comprehensive dry eye assessment", default_price: 350, default_duration_minutes: 60, medicare_item_number: "10910", active: true },
      { id: 2, type_code: "FOLLOWUP", type_name: "Follow-up", description: "Review consultation", default_price: 150, default_duration_minutes: 30, medicare_item_number: "10912", active: true },
      { id: 3, type_code: "IPL", type_name: "IPL Treatment", description: "Intense Pulsed Light therapy session", default_price: 300, default_duration_minutes: 45, medicare_item_number: null, active: true },
      { id: 4, type_code: "IMAGING", type_name: "Imaging", description: "Diagnostic imaging session", default_price: 250, default_duration_minutes: 30, medicare_item_number: null, active: true },
      { id: 5, type_code: "TELEHEALTH", type_name: "Telehealth", description: "Remote consultation", default_price: 120, default_duration_minutes: 20, medicare_item_number: "91852", active: true },
    ],
    services: [
      { id: 1, service_code: "IPL-SESSION", service_name: "IPL Therapy Session", category: "treatment", description: "Single IPL treatment", unit_price: 300, unit_type: "each", tax_rate: 0.1, active: true },
      { id: 2, service_code: "KERATO-5M", service_name: "Keratograph 5M Imaging", category: "imaging", description: "Ocular surface imaging", unit_price: 180, unit_type: "each", tax_rate: 0.1, active: true },
      { id: 3, service_code: "RESTASIS", service_name: "Restasis 0.05%", category: "medication", description: "Cyclosporine emulsion", unit_price: 95, unit_type: "each", tax_rate: 0.1, active: true },
      { id: 4, service_code: "ARTIFICIAL", service_name: "Artificial Tears", category: "medication", description: "Lubricant drops", unit_price: 25, unit_type: "each", tax_rate: 0.1, active: true },
    ],
    intake_submissions: [
      { id: 1, submitted_at: "2026-07-25T08:12:00Z", first_name: "Jane", last_name: "Carter", date_of_birth: "1990-03-15", phone: "0499 111 222", email: "jane.carter@example.com", address: null, medicare_number: null, preferred_date: "2026-08-01", preferred_time: "10:00", appointment_type: "Dry Eye Consultation", symptoms: "Gritty eyes for a few months", source: "intake-form", status: "new", matched_patient_id: null },
      { id: 2, submitted_at: "2026-07-24T16:30:00Z", first_name: "Tom", last_name: "Bradley", date_of_birth: "1982-11-20", phone: "0499 333 444", email: "tom.bradley@example.com", address: null, medicare_number: null, preferred_date: "2026-08-05", preferred_time: "14:00", appointment_type: "Follow-up", symptoms: "IPL session 3 due", source: "intake-form", status: "new", matched_patient_id: null },
    ],
    messages: [
      { id: 1, received_at: "2026-07-25T08:12:00Z", channel: "website", from_name: "Jane Carter", from_contact: "jane.carter@example.com", subject: "Booking enquiry", body: "Hi, I saw on your website that you do dry eye consultations. Could I book one for next week?", status: "unread", linked_patient_id: null, thread_id: "t-web-001", created_at: "2026-07-25T08:12:00Z" },
      { id: 2, received_at: "2026-07-25T07:45:00Z", channel: "email", from_name: "Tom Bradley", from_contact: "tom.bradley@example.com", subject: "IPL treatment question", body: "Hello, I had IPL session 2 last month and was wondering when I should book session 3. Is 4 weeks apart still ok?", status: "unread", linked_patient_id: null, thread_id: "t-email-002", created_at: "2026-07-25T07:45:00Z" },
      { id: 3, received_at: "2026-07-24T16:30:00Z", channel: "whatsapp", from_name: "Priya Shah", from_contact: "+61411222333", subject: null, body: "Hi! Do you have any availability this Friday afternoon for a follow-up?", status: "unread", linked_patient_id: null, thread_id: "t-wa-003", created_at: "2026-07-24T16:30:00Z" },
      { id: 4, received_at: "2026-07-24T11:20:00Z", channel: "email", from_name: "Bupa Claims", from_contact: "claims@bupa.com.au", subject: "Claim reference 8841923", body: "Medicare claim processed for patient. Reference 8841923.", status: "read", linked_patient_id: null, thread_id: "t-email-004", created_at: "2026-07-24T11:20:00Z" },
    ],
    website_events: [],
    patient_photos: [],
    nextId: 100,
  };

  // Token for mock auth
  const MOCK_TOKEN = "demo-token-admin";
  localStorage.setItem("pms_token", MOCK_TOKEN);
  localStorage.setItem("pms_user", JSON.stringify(DB.users[0]));

  function nextId(table) { return ++DB.nextId; }
  function ok(data) { return Promise.resolve({ data, status: 200 }); }
  function created(data) { return Promise.resolve({ data, status: 201 }); }

  // Route handlers
  const routes = {
    "GET /api/health": () => ok({ status: "ok", version: "0.1.0", clinic: "OptiCore" }),
    "POST /api/auth/login": (body) => {
      if (body.username === "admin" && body.password === "admin") {
        return ok({ token: MOCK_TOKEN, refresh_token: MOCK_TOKEN, user: DB.users[0] });
      }
      return Promise.reject({ response: { status: 401, data: { error: "Invalid credentials" } } });
    },
    "GET /api/auth/me": () => ok(DB.users[0]),
    "POST /api/auth/change-password": () => ok({ message: "Password changed (demo)" }),

    "GET /api/patients": (params) => {
      let list = DB.patients;
      if (params?.search) {
        const s = params.search.toLowerCase();
        list = list.filter(p => `${p.first_name} ${p.last_name} ${p.mrn} ${p.phone} ${p.email}`.toLowerCase().includes(s));
      }
      return ok({ patients: list, count: list.length });
    },
    "GET /api/patients/enriched/list": (params) => {
      let list = DB.patients.map(p => {
        const appts = DB.appointments.filter(a => a.patient_id === p.id);
        const invoices = DB.invoices.filter(i => i.patient_id === p.id);
        return {
          ...p,
          next_appointment: appts.filter(a => a.appointment_date > new Date().toISOString().replace("T", " ").slice(0, 19)).sort((a, b) => a.appointment_date.localeCompare(b.appointment_date))[0]?.appointment_date || null,
          last_appointment: appts.filter(a => a.appointment_date <= new Date().toISOString().replace("T", " ").slice(0, 19)).sort((a, b) => b.appointment_date.localeCompare(a.appointment_date))[0]?.appointment_date || null,
          total_visits: appts.length,
          total_spent: invoices.reduce((s, i) => s + i.amount_paid, 0),
          outstanding: invoices.reduce((s, i) => s + i.balance_due, 0),
        };
      });
      if (params?.search) {
        const s = params.search.toLowerCase();
        list = list.filter(p => `${p.first_name} ${p.last_name} ${p.mrn} ${p.phone} ${p.email}`.toLowerCase().includes(s));
      }
      return ok(list);
    },
    "GET /api/patients/:id": (params, p) => ok(DB.patients.find(x => x.id === Number(p.id))),
    "GET /api/patients/:id/detail": (params, p) => {
      const id = Number(p.id);
      const patient = DB.patients.find(x => x.id === id);
      if (!patient) return Promise.reject({ response: { status: 404, data: { error: "Not found" } } });
      const appts = DB.appointments.filter(a => a.patient_id === id).map(a => ({ ...a, first_name: patient.first_name, last_name: patient.last_name, phone: patient.phone, mrn: patient.mrn }));
      const notes = DB.clinical_notes.filter(n => n.patient_id === id);
      const allergies = DB.allergies.filter(a => a.patient_id === id);
      const osdi = DB.osdi_scores.filter(o => o.patient_id === id);
      const ipl = DB.ipl_treatments.filter(t => t.patient_id === id);
      const invoices = DB.invoices.filter(i => i.patient_id === id);
      const totalSpent = invoices.reduce((s, i) => s + i.amount_paid, 0);
      const outstanding = invoices.reduce((s, i) => s + i.balance_due, 0);
      return ok({
        patient, appointments: appts, notes, allergies, osdi_scores: osdi, ipl_treatments: ipl, invoices,
        stats: { total_visits: appts.length, last_visit: appts[0]?.appointment_date, total_spent: totalSpent, outstanding, first_visit: appts[appts.length - 1]?.appointment_date },
      });
    },
    "POST /api/patients": (body) => {
      const id = nextId();
      const year = new Date().getFullYear();
      const p = { id, mrn: body.mrn || `MOS-${year}${String(id).padStart(7, "0")}`, created_at: new Date().toISOString(), updated_at: new Date().toISOString(), ...body };
      DB.patients.push(p);
      return created(p);
    },
    "PUT /api/patients/:id": (body, p) => {
      const id = Number(p.id);
      const idx = DB.patients.findIndex(x => x.id === id);
      if (idx < 0) return Promise.reject({ response: { status: 404 } });
      DB.patients[idx] = { ...DB.patients[idx], ...body, updated_at: new Date().toISOString() };
      return ok(DB.patients[idx]);
    },
    "DELETE /api/patients/:id": (body, p) => {
      const id = Number(p.id);
      DB.patients = DB.patients.filter(x => x.id !== id);
      return ok({ message: "Deleted (demo)" });
    },

    "GET /api/appointments": (params) => {
      let list = DB.appointments.map(a => {
        const p = DB.patients.find(x => x.id === a.patient_id);
        return { ...a, first_name: p?.first_name, last_name: p?.last_name, phone: p?.phone, mrn: p?.mrn };
      });
      if (params?.date) list = list.filter(a => a.appointment_date.startsWith(params.date));
      if (params?.patient_id) list = list.filter(a => a.patient_id === Number(params.patient_id));
      return ok({ appointments: list, count: list.length });
    },
    "GET /api/appointments/today": () => {
      const today = new Date().toISOString().slice(0, 10);
      const list = DB.appointments.filter(a => a.appointment_date.startsWith(today)).map(a => {
        const p = DB.patients.find(x => x.id === a.patient_id);
        return { ...a, first_name: p?.first_name, last_name: p?.last_name, phone: p?.phone, mrn: p?.mrn };
      });
      return ok({ appointments: list, count: list.length });
    },
    "POST /api/appointments": (body) => {
      const id = nextId();
      const a = { id, created_at: new Date().toISOString(), ...body };
      DB.appointments.push(a);
      const p = DB.patients.find(x => x.id === body.patient_id);
      return created({ ...a, first_name: p?.first_name, last_name: p?.last_name });
    },
    "PUT /api/appointments/:id": (body, p) => {
      const id = Number(p.id);
      const idx = DB.appointments.findIndex(a => a.id === id);
      if (idx < 0) return Promise.reject({ response: { status: 404 } });
      DB.appointments[idx] = { ...DB.appointments[idx], ...body };
      return ok(DB.appointments[idx]);
    },
    "DELETE /api/appointments/:id": (body, p) => {
      DB.appointments = DB.appointments.filter(a => a.id !== Number(p.id));
      return ok({ message: "Deleted (demo)" });
    },

    "GET /api/blocked-times": () => ok(DB.blocked_times),
    "POST /api/blocked-times": (body) => { const id = nextId(); const b = { id, created_at: new Date().toISOString(), all_day: false, is_recurring: false, ...body }; DB.blocked_times.push(b); return created(b); },
    "PUT /api/blocked-times/:id": (body, p) => { const id = Number(p.id); const idx = DB.blocked_times.findIndex(b => b.id === id); if (idx < 0) return Promise.reject({ response: { status: 404 } }); DB.blocked_times[idx] = { ...DB.blocked_times[idx], ...body }; return ok(DB.blocked_times[idx]); },
    "DELETE /api/blocked-times/:id": (body, p) => { DB.blocked_times = DB.blocked_times.filter(b => b.id !== Number(p.id)); return ok({ message: "Deleted (demo)" }); },

    "GET /api/calendar/:from/:to": (params, p) => {
      const from = p.from, to = p.to;
      const events = [];
      DB.appointments.filter(a => a.appointment_date >= from && a.appointment_date <= to).forEach(a => {
        const pat = DB.patients.find(x => x.id === a.patient_id);
        const end = new Date(new Date(a.appointment_date.replace(" ", "T")).getTime() + a.duration_minutes * 60000).toISOString().replace("T", " ").slice(0, 19);
        const inv = DB.invoices.find(i => i.patient_id === a.patient_id);
        events.push({ id: a.id, kind: "appointment", start_at: a.appointment_date, end_at: end, title: `${pat?.first_name} ${pat?.last_name}`, patient_id: a.patient_id, practitioner: a.practitioner, status: a.status, reason: null, paid: inv ? (inv.balance_due <= 0 ? "paid" : "partial") : null, balance: inv?.balance_due > 0 ? inv.balance_due : null });
      });
      DB.blocked_times.filter(b => b.start_at >= from && b.start_at <= to).forEach(b => {
        events.push({ id: b.id, kind: "blocked", start_at: b.start_at, end_at: b.end_at, title: b.reason || "Blocked", patient_id: null, practitioner: b.practitioner, status: null, reason: b.reason, paid: null, balance: null });
      });
      return ok(events.sort((a, b) => a.start_at.localeCompare(b.start_at)));
    },

    "GET /api/patients/:id/notes": (params, p) => ok(DB.clinical_notes.filter(n => n.patient_id === Number(p.id))),
    "POST /api/patients/:id/notes": (body) => { const id = nextId(); const n = { id, created_at: new Date().toISOString(), ...body }; DB.clinical_notes.push(n); return created(n); },
    "DELETE /api/patients/:id/notes/:nid": (body, p) => { DB.clinical_notes = DB.clinical_notes.filter(n => n.id !== Number(p.nid)); return ok({ message: "Deleted (demo)" }); },
    "GET /api/patients/:id/allergies": (params, p) => ok(DB.allergies.filter(a => a.patient_id === Number(p.id))),
    "POST /api/allergies": (body) => { const id = nextId(); const a = { id, noted_at: new Date().toISOString(), ...body }; DB.allergies.push(a); return ok(a); },
    "DELETE /api/allergies/:id": (body, p) => { DB.allergies = DB.allergies.filter(a => a.id !== Number(p.id)); return ok({ message: "Deleted (demo)" }); },
    "GET /api/patients/:id/osdi": (params, p) => ok(DB.osdi_scores.filter(o => o.patient_id === Number(p.id))),
    "POST /api/patients/:id/osdi": (body) => { const id = nextId(); const o = { id, created_at: new Date().toISOString(), ...body }; DB.osdi_scores.push(o); return ok(o); },
    "GET /api/patients/:id/ipl": (params, p) => ok(DB.ipl_treatments.filter(t => t.patient_id === Number(p.id))),
    "POST /api/patients/:id/ipl": (body) => { const id = nextId(); const t = { id, created_at: new Date().toISOString(), ...body }; DB.ipl_treatments.push(t); return ok(t); },

    "GET /api/billing/consultation-types": () => ok(DB.consultation_types),
    "GET /api/billing/services": (params) => params?.category ? ok(DB.services.filter(s => s.category === params.category)) : ok(DB.services),
    "GET /api/billing/service-categories": () => ok([...new Set(DB.services.map(s => s.category))]),
    "GET /api/billing/invoices/patient/:pid": (params, p) => ok(DB.invoices.filter(i => i.patient_id === Number(p.pid))),
    "POST /api/billing/invoices": (body) => { const id = nextId(); const inv = { id, invoice_number: `INV-2026-${String(id).padStart(4, "0")}`, invoice_date: new Date().toISOString().replace("T", " ").slice(0, 19), due_date: null, discount_amount: 0, amount_paid: 0, status: "issued", created_at: new Date().toISOString(), items: [], ...body }; DB.invoices.push(inv); return ok(inv); },
    "GET /api/billing/payments/invoice/:inv": (params, p) => ok(DB.payments.filter(pay => pay.invoice_id === Number(p.inv))),
    "POST /api/billing/payments": (body) => { const id = nextId(); const pay = { id, payment_date: new Date().toISOString().replace("T", " ").slice(0, 19), created_at: new Date().toISOString(), ...body }; DB.payments.push(pay); const inv = DB.invoices.find(i => i.id === body.invoice_id); if (inv) { inv.amount_paid += body.amount; inv.balance_due = Math.max(0, inv.total_amount - inv.amount_paid); inv.status = inv.balance_due <= 0 ? "paid" : "partially_paid"; } return ok(pay); },

    "GET /api/analytics/overview": () => {
      const revenue = DB.invoices.reduce((s, i) => s + i.amount_paid, 0);
      const outstanding = DB.invoices.filter(i => i.status !== "paid").reduce((s, i) => s + i.balance_due, 0);
      return ok({ total_patients: DB.patients.length, total_appointments: DB.appointments.length, total_revenue: revenue, outstanding_balance: outstanding, appointments_this_month: DB.appointments.length, revenue_this_month: revenue, avg_appt_value: DB.appointments.length ? revenue / DB.appointments.length : 0 });
    },
    "GET /api/analytics/revenue/:days": (params, p) => {
      const days = Number(p.days);
      const out = [];
      for (let i = days; i >= 0; i--) {
        const d = new Date(); d.setDate(d.getDate() - i);
        const ds = d.toISOString().slice(0, 10);
        out.push({ date: ds, value: Math.floor(50 + Math.random() * 400) });
      }
      return ok(out);
    },
    "GET /api/analytics/appointments/:days": (params, p) => {
      const days = Number(p.days); const out = [];
      for (let i = days; i >= 0; i--) { const d = new Date(); d.setDate(d.getDate() - i); out.push({ date: d.toISOString().slice(0, 10), value: Math.floor(Math.random() * 6) }); }
      return ok(out);
    },
    "GET /api/analytics/traffic/:days": (params, p) => {
      const days = Number(p.days); const out = [];
      const sources = ["website", "google", "facebook", "direct"];
      for (let i = days; i >= 0; i--) { const d = new Date(); d.setDate(d.getDate() - i); out.push({ date: d.toISOString().slice(0, 10), visitors: 20 + Math.floor(Math.random() * 40), page_views: 60 + Math.floor(Math.random() * 120), bookings: Math.floor(Math.random() * 4), source: sources[Math.floor(Math.random() * 4)] }); }
      return ok(out);
    },
    "GET /api/analytics/traffic-by-source": () => ok([
      { source: "google", visitors: 412, bookings: 14 },
      { source: "direct", visitors: 234, bookings: 8 },
      { source: "facebook", visitors: 156, bookings: 3 },
      { source: "referral", visitors: 89, bookings: 5 },
    ]),

    "GET /api/intake": () => ok(DB.intake_submissions),
    "POST /api/intake/:id/import": (body, p) => { const sub = DB.intake_submissions.find(s => s.id === Number(p.id)); if (sub) sub.status = "imported"; return ok({ message: "Imported (demo)" }); },
    "POST /api/intake/:id/archive": (body, p) => { const sub = DB.intake_submissions.find(s => s.id === Number(p.id)); if (sub) sub.status = "archived"; return ok({ message: "Archived (demo)" }); },
    "POST /api/intake/auto-import": () => { let n = 0; DB.intake_submissions.forEach(s => { if (s.status === "new") { s.status = "imported"; n++; } }); return ok({ imported: n, total_new: n }); },

    "GET /api/messages": (params) => params?.channel ? ok(DB.messages.filter(m => m.channel === params.channel)) : params?.status ? ok(DB.messages.filter(m => m.status === params.status)) : ok(DB.messages),
    "POST /api/messages/:id/read": (body, p) => { const m = DB.messages.find(x => x.id === Number(p.id)); if (m) m.status = "read"; return ok({ message: "Read" }); },
    "POST /api/messages/:id/archive": (body, p) => { const m = DB.messages.find(x => x.id === Number(p.id)); if (m) m.status = "archived"; return ok({ message: "Archived" }); },
    "POST /api/messages/:id/link/:pid": (body, p) => { const m = DB.messages.find(x => x.id === Number(p.id)); if (m) m.linked_patient_id = Number(p.pid); return ok({ message: "Linked" }); },

    "GET /api/users": () => ok(DB.users.map(u => { const { password_hash, ...rest } = u; return rest; })),
    "POST /api/users": (body) => { const id = nextId(); const u = { id, is_active: true, created_at: new Date().toISOString(), updated_at: new Date().toISOString(), ...body }; DB.users.push({ ...u, password_hash: "mock" }); return created(u); },
    "PUT /api/users/:id": (body, p) => { const id = Number(p.id); const idx = DB.users.findIndex(u => u.id === id); if (idx < 0) return Promise.reject({ response: { status: 404 } }); DB.users[idx] = { ...DB.users[idx], ...body, updated_at: new Date().toISOString() }; const { password_hash, ...rest } = DB.users[idx]; return ok(rest); },
    "POST /api/users/:id/toggle": (body, p) => { const u = DB.users.find(x => x.id === Number(p.id)); if (u) u.is_active = !u.is_active; return ok({ id: Number(p.id), is_active: u?.is_active }); },
    "DELETE /api/users/:id": (body, p) => { DB.users = DB.users.filter(u => u.id !== Number(p.id)); return ok({ message: "Deleted (demo)" }); },

    "GET /api/patients/:id/photos": () => ok(DB.patient_photos.filter(p => p.patient_id === Number(arguments[1]?.id))),
    "POST /api/data/version": () => ok({ app_version: "0.1.0", snapshot_version: 1, supported_min_snapshot: 1 }),
  };

  // Match a URL+method to a route, extracting path params
  function matchRoute(method, url) {
    // strip query string
    const [path, query] = url.split("?");
    const params = {};
    if (query) query.split("&").forEach(p => { const [k, v] = p.split("="); params[k] = decodeURIComponent(v); });

    // try exact match first
    const key = `${method} ${path}`;
    if (routes[key]) return { handler: routes[key], params, pathParams: {} };

    // try parametric match
    for (const route of Object.keys(routes)) {
      const [rm, rp] = route.split(" ");
      if (rm !== method) continue;
      const rParts = rp.split("/");
      const pParts = path.split("/");
      if (rParts.length !== pParts.length) continue;
      const pParams = {};
      let match = true;
      for (let i = 0; i < rParts.length; i++) {
        if (rParts[i].startsWith(":")) pParams[rParts[i].slice(1)] = pParts[i];
        else if (rParts[i] !== pParts[i]) { match = false; break; }
      }
      if (match) return { handler: routes[route], params, pathParams: pParams };
    }
    return null;
  }

  // Override fetch for the static demo
  const origFetch = window.fetch;
  window.fetch = async function (url, opts = {}) {
    const method = (opts.method || "GET").toUpperCase();
    const match = matchRoute(method, url);
    if (match) {
      const body = opts.body ? JSON.parse(opts.body) : {};
      try {
        const data = await match.handler(body, match.pathParams);
        return { ok: data.status < 400, status: data.status, json: () => Promise.resolve(data.data), text: () => Promise.resolve(JSON.stringify(data.data)) };
      } catch (e) {
        throw e;
      }
    }
    // fall through to real fetch for non-API requests (static assets)
    return origFetch.call(this, url, opts);
  };

  // Also intercept XMLHttpRequest (axios uses XHR by default)
  const origOpen = XMLHttpRequest.prototype.open;
  const origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    this._mockMethod = method.toUpperCase();
    this._mockUrl = url;
    return origOpen.call(this, method, url, ...rest);
  };
  XMLHttpRequest.prototype.send = function (body) {
    const match = matchRoute(this._mockMethod, this._mockUrl);
    if (match) {
      const parsed = body ? JSON.parse(body) : {};
      match.handler(parsed, match.pathParams).then(data => {
        Object.defineProperty(this, "readyState", { value: 4, configurable: true });
        Object.defineProperty(this, "status", { value: data.status, configurable: true });
        Object.defineProperty(this, "response", { value: JSON.stringify(data.data), configurable: true });
        Object.defineProperty(this, "responseText", { value: JSON.stringify(data.data), configurable: true });
        if (this.onreadystatechange) this.onreadystatechange();
        if (this.onload) this.onload();
      }).catch(err => {
        const status = err?.response?.status || 500;
        const data = err?.response?.data || { error: "Mock error" };
        Object.defineProperty(this, "readyState", { value: 4, configurable: true });
        Object.defineProperty(this, "status", { value: status, configurable: true });
        Object.defineProperty(this, "response", { value: JSON.stringify(data), configurable: true });
        Object.defineProperty(this, "responseText", { value: JSON.stringify(data), configurable: true });
        if (this.onreadystatechange) this.onreadystatechange();
        if (this.onload) this.onload();
      });
      return;
    }
    return origSend.call(this, body);
  };

  console.log("✅ OptiCore demo mock API loaded — all endpoints intercepted");
})();
