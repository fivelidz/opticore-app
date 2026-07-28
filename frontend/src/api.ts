import axios from "axios";

// API base URL:
// - VITE_API_URL override (explicit, used by the web demo mock)
// - In the Tauri desktop app, the embedded server is on localhost:3000.
//   The Tauri webview serves pages via tauri:// or https://tauri.localhost,
//   so we detect "not a normal http dev server" and point at localhost:3000.
// - In Vite dev (http://localhost:5173), use empty baseURL (proxy handles it).
const isTauriApp = typeof window !== "undefined" &&
  (window.location.protocol.startsWith("tauri") ||
   window.location.hostname === "tauri.localhost" ||
   (window.location.protocol !== "http:" && window.location.protocol !== "https:"));

const isDevServer = typeof window !== "undefined" &&
  window.location.hostname === "localhost" && window.location.port === "5173";

const baseURL =
  (import.meta as any).env?.VITE_API_URL ||
  (isTauriApp ? "http://localhost:3000" : isDevServer ? "" : "http://localhost:3000");

export const api = axios.create({ baseURL: baseURL + "/api", timeout: 15000 });

// Attach JWT to every request if present.
api.interceptors.request.use((cfg) => {
  const token = localStorage.getItem("pms_token");
  if (token) cfg.headers.Authorization = `Bearer ${token}`;
  return cfg;
});

// On 401, clear session and bounce to login.
api.interceptors.response.use(
  (r) => r,
  (err) => {
    if (err?.response?.status === 401) {
      localStorage.removeItem("pms_token");
      localStorage.removeItem("pms_user");
      // Use hash routing (works in both browser and Tauri webview)
      if (!window.location.hash.includes("/login")) {
        window.location.hash = "#/login";
      }
    }
    return Promise.reject(err);
  }
);

export interface User {
  id: number;
  username: string;
  email: string;
  role: string;
  first_name: string;
  last_name: string;
}

export interface Patient {
  id: number;
  mrn: string;
  first_name: string;
  last_name: string;
  date_of_birth: string;
  gender?: string | null;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  medicare_number?: string | null;
  created_at: string;
  updated_at: string;
}

export interface Appointment {
  id: number;
  patient_id: number;
  appointment_type: string;
  appointment_date: string;
  duration_minutes: number;
  practitioner?: string | null;
  status: string;
  notes?: string | null;
  first_name?: string;
  last_name?: string;
  phone?: string;
  mrn?: string;
}

export interface CalendarEvent {
  id: number;
  kind: "appointment" | "blocked";
  start_at: string;
  end_at: string;
  title: string;
  patient_id?: number;
  practitioner?: string;
  status?: string;
  reason?: string;
  paid?: "paid" | "partial" | null;
  balance?: number;
}

export const auth = {
  login: (username: string, password: string) =>
    api.post<{ token: string; refresh_token: string; user: User }>("/auth/login", {
      username,
      password,
    }),
  me: () => api.get<User>("/auth/me"),
  changePassword: (currentPassword: string, newPassword: string) =>
    api.post<{ message: string }>("/auth/change-password", {
      current_password: currentPassword,
      new_password: newPassword,
    }),
};

export const patients = {
  list: (search?: string) =>
    api.get<{ patients: Patient[]; count: number }>("/patients", {
      params: search ? { search } : {},
    }),
  listEnriched: (search?: string) =>
    api.get<any[]>("/patients/enriched/list", { params: search ? { search } : {} }),
  get: (id: number) => api.get<Patient>(`/patients/${id}`),
  create: (p: Partial<Patient>) => api.post<Patient>("/patients", p),
  update: (id: number, p: Partial<Patient>) => api.put<Patient>(`/patients/${id}`, p),
  remove: (id: number) => api.delete(`/patients/${id}`),
};

export const appointments = {
  list: (params?: { date?: string; patient_id?: number; from?: string; to?: string }) =>
    api.get<{ appointments: Appointment[]; count: number }>("/appointments", { params }),
  today: () => api.get<{ appointments: Appointment[]; count: number }>("/appointments/today"),
  create: (a: Partial<Appointment>) => api.post<Appointment>("/appointments", a),
  update: (id: number, a: Partial<Appointment>) => api.put<Appointment>(`/appointments/${id}`, a),
  remove: (id: number) => api.delete(`/appointments/${id}`),
};

export const blockedTimes = {
  list: () => api.get<any[]>("/blocked-times"),
  create: (b: { start_at: string; end_at: string; reason?: string; practitioner?: string }) =>
    api.post("/blocked-times", b),
  update: (id: number, b: { start_at: string; end_at: string; reason?: string; practitioner?: string }) =>
    api.put(`/blocked-times/${id}`, b),
  remove: (id: number) => api.delete(`/blocked-times/${id}`),
};

export const calendar = {
  range: (from: string, to: string) =>
    api.get<CalendarEvent[]>(`/calendar/${from}/${to}`),
};

// ---------- Patient detail aggregate ----------

export interface ClinicalNote {
  id: number; patient_id: number; author?: string | null;
  category: string; note: string; created_at: string;
}
export interface Allergy {
  id: number; patient_id: number; substance: string; severity: string; noted_at: string;
}
export interface OsdiScore {
  id: number; patient_id: number; score_date: string; total_score: number;
  ocular_symptoms?: number | null; vision_function?: number | null; environmental_triggers?: number | null;
  created_at: string;
}
export interface IplTreatment {
  id: number; patient_id: number; treatment_date: string; session_number: number;
  fluence_j_cm2?: number | null; number_of_pulses?: number | null;
  operator_name?: string | null; clinical_notes?: string | null; created_at: string;
}
export interface InvoiceItem {
  id: number; invoice_id: number; item_type: string; description: string;
  quantity: number; unit_price: number; discount_percent: number; tax_rate: number; total: number;
}
export interface Invoice {
  id: number; invoice_number: string; patient_id: number; appointment_id?: number | null;
  invoice_date: string; due_date?: string | null; subtotal: number; tax_amount: number;
  discount_amount: number; total_amount: number; amount_paid: number; balance_due: number;
  status: string; payment_method?: string | null; notes?: string | null; created_at: string;
  items?: InvoiceItem[];
}
export interface PatientStats {
  total_visits: number; last_visit?: string | null; total_spent: number;
  outstanding: number; first_visit?: string | null;
}
export interface PatientDetail {
  patient: Patient;
  appointments: Appointment[];
  notes: ClinicalNote[];
  allergies: Allergy[];
  osdi_scores: OsdiScore[];
  ipl_treatments: IplTreatment[];
  invoices: Invoice[];
  stats: PatientStats;
}

export const patientDetail = {
  get: (id: number) => api.get<PatientDetail>(`/patients/${id}/detail`),
};

export const clinical = {
  addNote: (n: { patient_id: number; author?: string; category?: string; note: string }) =>
    api.post<ClinicalNote>("/patients/" + n.patient_id + "/notes", n),
  delNote: (id: number) => api.delete(`/patients/0/notes/${id}`),
  addAllergy: (a: { patient_id: number; substance: string; severity?: string }) =>
    api.post<Allergy>("/allergies", a),
  delAllergy: (id: number) => api.delete(`/allergies/${id}`),
  addOsdi: (o: any) => api.post<OsdiScore>(`/patients/${o.patient_id}/osdi`, o),
  addIpl: (i: any) => api.post<IplTreatment>(`/patients/${i.patient_id}/ipl`, i),
};

// ---------- Billing ----------

export interface ConsultationType {
  id: number; type_code: string; type_name: string; description?: string | null;
  default_price: number; default_duration_minutes: number; medicare_item_number?: string | null; active: boolean;
}
export interface ServiceItem {
  id: number; service_code: string; service_name: string; category: string;
  description?: string | null; unit_price: number; unit_type: string; tax_rate: number; active: boolean;
}
export interface Payment {
  id: number; invoice_id: number; payment_date: string; amount: number;
  payment_method: string; reference_number?: string | null; notes?: string | null; created_at: string;
}

export const billing = {
  consultationTypes: () => api.get<ConsultationType[]>("/billing/consultation-types"),
  services: (category?: string) => api.get<ServiceItem[]>("/billing/services", { params: category ? { category } : {} }),
  serviceCategories: () => api.get<string[]>("/billing/service-categories"),
  invoicesByPatient: (pid: number) => api.get<Invoice[]>(`/billing/invoices/patient/${pid}`),
  createInvoice: (inv: any) => api.post<Invoice>("/billing/invoices", inv),
  paymentsByInvoice: (inv: number) => api.get<Payment[]>(`/billing/payments/invoice/${inv}`),
  addPayment: (p: { invoice_id: number; amount: number; payment_method: string; reference_number?: string; notes?: string }) =>
    api.post<Payment>("/billing/payments", p),
};

// ---------- Analytics ----------

export interface AnalyticsOverview {
  total_patients: number; total_appointments: number; total_revenue: number;
  outstanding_balance: number; appointments_this_month: number; revenue_this_month: number; avg_appt_value: number;
}
export interface TimeSeriesPoint { date: string; value: number; }
export interface WebsiteTrafficPoint { date: string; visitors: number; page_views: number; bookings: number; source: string; }
export interface SourceBreakdown { source: string; visitors: number; bookings: number; }
export interface RevenueByType { appointment_type: string; revenue: number; count: number; }
export interface NoShowRate {
  total: number; no_show: number; cancelled: number; completed: number;
  no_show_rate: number; cancellation_rate: number;
}
export interface HourCount { hour: number; count: number; }
export interface AgeBracket { bracket: string; count: number; }
export interface OutstandingPatient {
  patient_id: number; name: string; mrn: string; outstanding: number; invoice_count: number;
}

export const analytics = {
  overview: () => api.get<AnalyticsOverview>("/analytics/overview"),
  revenue: (days: number) => api.get<TimeSeriesPoint[]>(`/analytics/revenue/${days}`),
  appointments: (days: number) => api.get<TimeSeriesPoint[]>(`/analytics/appointments/${days}`),
  traffic: (days: number) => api.get<WebsiteTrafficPoint[]>(`/analytics/traffic/${days}`),
  trafficBySource: () => api.get<SourceBreakdown[]>("/analytics/traffic-by-source"),
  patientGrowth: (days: number) => api.get<TimeSeriesPoint[]>(`/analytics/patient-growth/${days}`),
  revenueByType: () => api.get<RevenueByType[]>("/analytics/revenue-by-type"),
  noShowRate: () => api.get<NoShowRate>("/analytics/no-show-rate"),
  hourDistribution: () => api.get<HourCount[]>("/analytics/hour-distribution"),
  ageDemographics: () => api.get<AgeBracket[]>("/analytics/age-demographics"),
  outstandingByPatient: () => api.get<OutstandingPatient[]>("/analytics/outstanding-by-patient"),
};

// ---------- Intake (public input page submissions) ----------

export interface IntakeSubmission {
  id: number; submitted_at: string; first_name: string; last_name: string;
  date_of_birth?: string | null; phone?: string | null; email?: string | null;
  address?: string | null; medicare_number?: string | null;
  preferred_date?: string | null; preferred_time?: string | null;
  appointment_type?: string | null; symptoms?: string | null;
  source: string; status: string; matched_patient_id?: number | null;
}

export const intake = {
  list: () => api.get<IntakeSubmission[]>("/intake"),
  import: (id: number) => api.post<{ message: string }>(`/intake/${id}/import`),
  archive: (id: number) => api.post<{ message: string }>(`/intake/${id}/archive`),
  autoImport: () => api.post<{ imported: number; total_new: number }>(`/intake/auto-import`),
};

// ---------- Messages (unified inbox) ----------

export interface Message {
  id: number; received_at: string; channel: string;
  from_name?: string | null; from_contact?: string | null;
  subject?: string | null; body: string; status: string;
  linked_patient_id?: number | null; thread_id?: string | null; created_at: string;
}

export const messages = {
  list: (params?: { channel?: string; status?: string }) => api.get<Message[]>("/messages", { params }),
  receive: (m: any) => api.post<Message>("/messages/receive", m),
  markRead: (id: number) => api.post(`/messages/${id}/read`),
  archive: (id: number) => api.post(`/messages/${id}/archive`),
  linkPatient: (id: number, pid: number) => api.post(`/messages/${id}/link/${pid}`),
};

// ---------- Users (staff management) ----------

export interface StaffUser {
  id: number; username: string; email: string; role: string;
  first_name: string; last_name: string; is_active: boolean;
  created_at: string; updated_at: string;
}

export const users = {
  list: () => api.get<StaffUser[]>("/users"),
  create: (u: { username: string; email: string; password: string; role: string; first_name: string; last_name: string }) =>
    api.post<StaffUser>("/users", u),
  update: (id: number, u: any) => api.put<StaffUser>(`/users/${id}`, u),
  toggle: (id: number) => api.post(`/users/${id}/toggle`),
  remove: (id: number) => api.delete(`/users/${id}`),
};

// ---------- Patient photos / files ----------

export interface PatientPhoto {
  id: number; patient_id: number; appointment_id?: number | null; category: string; filename: string;
  mime_type: string; caption?: string | null; file_size?: number | null;
  captured_at: string; created_at: string;
}

export const photos = {
  list: (pid: number) => api.get<PatientPhoto[]>(`/patients/${pid}/photos`),
  getData: (pid: number, photoId: number) => api.get<{ data: string; mime: string }>(`/patients/${pid}/photos/${photoId}`),
  upload: (pid: number, body: { category: string; filename: string; mime_type: string; caption?: string; data_base64: string }) =>
    api.post<PatientPhoto>(`/patients/${pid}/photos`, { patient_id: pid, ...body }),
  remove: (pid: number, photoId: number) => api.delete(`/patients/${pid}/photos/${photoId}`),
  makeProfile: (pid: number, photoId: number) => api.post(`/patients/${pid}/photos/${photoId}/make-profile`),
};

// ---------- Appointment attachments (documents & photos on a visit's notes) ----------
export const attachments = {
  list: (apptId: number) => api.get<PatientPhoto[]>(`/appointments/${apptId}/attachments`),
  getData: (apptId: number, photoId: number) => api.get<{ data: string; mime: string }>(`/appointments/${apptId}/attachments/${photoId}`),
  upload: (apptId: number, body: { category?: string; filename: string; mime_type: string; caption?: string; data_base64: string }) =>
    api.post<PatientPhoto>(`/appointments/${apptId}/attachments`, { patient_id: 0, category: "document", ...body }),
  remove: (apptId: number, photoId: number) => api.delete(`/appointments/${apptId}/attachments/${photoId}`),
};

// ---------- Booking settings & notifications ----------

export interface BookingSettings {
  booking_mode: string; auto_confirm_message: boolean; auto_reminder_message: boolean;
  reminder_hours_before: number; email_provider: string; email_from: string;
  sms_provider: string; sms_sender: string;
  template_booking_received: string; template_booking_confirmed: string;
  template_booking_declined: string; template_reminder: string;
}
export interface BookingNotification {
  id: number; booking_id?: number|null; intake_submission_id?: number|null;
  channel: string; recipient: string; template_used?: string|null;
  body: string; status: string; sent_at?: string|null; created_at: string;
}
export const bookingSettings = {
  get: () => api.get<BookingSettings>("/booking-settings"),
  update: (s: Partial<BookingSettings>) => api.put<BookingSettings>("/booking-settings", s),
  notifications: () => api.get<BookingNotification[]>("/booking-notifications"),
  approve: (id: number) => api.post(`/intake/${id}/approve`),
  decline: (id: number) => api.post(`/intake/${id}/decline`),
};
