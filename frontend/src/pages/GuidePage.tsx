import { useState } from "react";

type Section = "overview" | "getting-started" | "patients" | "calendar" | "intake" | "billing" | "messages" | "analytics" | "users" | "backup" | "architecture" | "sync" | "lan" | "faq";

const SECTIONS: [Section, string, string][] = [
  ["overview", "📋 Overview", "What OptiCore is and how it fits together"],
  ["getting-started", "🚀 Getting Started", "First steps after install"],
  ["patients", "👥 Patients", "Managing patient records"],
  ["calendar", "📅 Calendar", "Booking, blocking, and rescheduling"],
  ["intake", "📥 Online Intake", "How online bookings reach the clinic"],
  ["billing", "💳 Billing & Checkout", "Invoices, payments, GST"],
  ["messages", "📬 Messages", "Email, WhatsApp, website messages"],
  ["analytics", "📈 Analytics", "Understanding your practice data"],
  ["users", "🧑‍⚕️ Users & Roles", "Staff access control"],
  ["backup", "💾 Backup & Data", "Export, import, version safety"],
  ["architecture", "🏗️ Architecture", "How the system works under the hood"],
  ["sync", "🔄 Cloudflare Sync", "How online bookings sync to the app"],
  ["lan", "🌐 Multi-Device (LAN)", "Use OptiCore from tablets & other PCs"],
  ["faq", "❓ FAQ", "Common questions"],
];

export function GuidePage() {
  const [section, setSection] = useState<Section>("overview");

  return (
    <div className="guide-layout">
      <aside className="guide-sidebar">
        <h2 className="guide-title">📖 Guide</h2>
        <p className="guide-sub">How everything works</p>
        <nav className="guide-nav">
          {SECTIONS.map(([s, label, desc]) => (
            <button key={s} className={section === s ? "guide-nav-item active" : "guide-nav-item"} onClick={() => setSection(s)}>
              <span className="guide-nav-label">{label}</span>
              <span className="guide-nav-desc">{desc}</span>
            </button>
          ))}
        </nav>
      </aside>

      <main className="guide-content">
        {section === "overview" && <Overview />}
        {section === "getting-started" && <GettingStarted />}
        {section === "patients" && <Patients />}
        {section === "calendar" && <Calendar />}
        {section === "intake" && <Intake />}
        {section === "billing" && <Billing />}
        {section === "messages" && <Messages />}
        {section === "analytics" && <Analytics />}
        {section === "users" && <Users />}
        {section === "backup" && <Backup />}
        {section === "architecture" && <Architecture />}
        {section === "sync" && <Sync />}
        {section === "lan" && <LAN />}
        {section === "faq" && <FAQ />}
      </main>

      <style>{GUIDE_STYLE}</style>
    </div>
  );
}

function H({ children }: any) { return <h2 className="g-h2">{children}</h2>; }
function P({ children }: any) { return <p className="g-p">{children}</p>; }
function Step({ n, title, children }: any) {
  return <div className="g-step"><div className="g-step-num">{n}</div><div><strong>{title}</strong><div className="g-step-body">{children}</div></div></div>;
}
function Callout({ type, children }: any) {
  const icons: any = { info: "💡", warn: "⚠️", tip: "✨", security: "🔒" };
  return <div className={`g-callout g-callout-${type}`}><span className="g-callout-icon">{icons[type] || "💡"}</span>{children}</div>;
}

function Overview() {
  return <div>
    <H>What is OptiCore?</H>
    <P>OptiCore is a complete practice management system for optometry and eye-care clinics. It runs as a desktop application on your clinic computers — no cloud subscriptions, no monthly fees, and your patient data never leaves your premises.</P>
    <P>Everything lives in one app: patient records, appointment calendar, billing and checkout, clinical notes, online booking intake, messages, and analytics.</P>
    <H>What makes it different?</H>
    <Callout type="security">Your data is stored locally on your own computer in an encrypted SQLite database. It is never sent to any cloud server unless you explicitly enable the Cloudflare sync for online bookings.</Callout>
    <P><strong>Built in Rust</strong> — the backend is written in Rust, a language used for mission-critical systems. This means it is fast, reliable, and the database queries are checked at compile time to prevent the bugs that plague other systems.</P>
    <P><strong>Works offline</strong> — the app runs entirely on your local network. If the internet goes down, you can still see patients, book appointments, and take payments. Online bookings queue on the Cloudflare Worker and sync when the internet returns.</P>
    <P><strong>Multi-computer</strong> — one computer acts as the server (stays on during clinic hours), and every other computer in the clinic connects to it over your local network. All staff see the same data in real time.</P>
  </div>;
}

function GettingStarted() {
  return <div>
    <H>Getting Started</H>
    <P>After installing OptiCore, here is how to get up and running:</P>
    <Step n="1" title="Log in">The first time you launch the app, use the admin credentials shown in the console window. For demo mode, the login is <code>admin</code> / <code>admin</code>. Change this immediately in Settings.</Step>
    <Step n="2" title="Add your staff">Go to <strong>Users</strong> and create accounts for each staff member. Choose their role: Admin (full access), Doctor (clinical + billing), Nurse (clinical), Receptionist (bookings + patients), or Read-only.</Step>
    <Step n="3" title="Add patients">Go to <strong>Patients</strong> and click "+ Add Patient" to start building your patient database. You can also import patients from online intake submissions.</Step>
    <Step n="4" title="Set up the calendar">Go to <strong>Calendar</strong> and block out times when you are not available (lunch, days off) using the Mass Block feature. Book appointments by dragging on empty time slots.</Step>
    <Step n="5" title="Configure online intake">Go to <strong>Intake</strong> → Settings to choose your booking mode (Automatic or Approval) and set up confirmation messages. The public booking form is at <code>http://localhost:3000/input</code>.</Step>
    <Step n="6" title="Back up your data">Go to <strong>Settings</strong> → Data Export to create an encrypted snapshot of your database. Do this regularly.</Step>
    <Callout type="tip">The patient intake form (the public booking page) can be opened on any device — phone, tablet, or computer — at the address shown above. Patients fill it in and their booking appears in your Intake tab.</Callout>
  </div>;
}

function Patients() {
  return <div>
    <H>Managing Patients</H>
    <P>The <strong>Patients</strong> tab is your complete patient database. Every patient has a deep record accessible by clicking their name.</P>
    <H>The patient record</H>
    <P>Each patient file contains:</P>
    <ul className="g-list">
      <li><strong>Contact & demographics</strong> — name, DOB, phone, email, address, Medicare number (editable)</li>
      <li><strong>Allergies</strong> — with severity levels (mild/moderate/severe). These show as warnings throughout the app.</li>
      <li><strong>OSDI scores</strong> — dry eye questionnaire scores over time, colour-coded by severity</li>
      <li><strong>IPL treatments</strong> — session history with fluence, pulses, and clinical notes</li>
      <li><strong>Photos & documents</strong> — profile pictures, medical images (eye scans), and documents (referrals, consent forms)</li>
      <li><strong>Appointment history</strong> — every visit, expandable to show per-appointment notes</li>
      <li><strong>Clinical notes</strong> — categorized (assessment, treatment, follow-up, general), with author and timestamp</li>
      <li><strong>Invoices & payments</strong> — full billing history with outstanding balances</li>
    </ul>
    <H>Sorting and searching</H>
    <P>Click any column header to sort (name, DOB, next appointment, amount spent, etc.). Use the search bar to find patients by name, MRN, phone, or email. Export the list to CSV at any time.</P>
    <Callout type="tip">The 📝 icon on an appointment in the history means it has notes attached. Click to expand and read them.</Callout>
  </div>;
}

function Calendar() {
  return <div>
    <H>Calendar</H>
    <P>The calendar has three views: <strong>Month</strong>, <strong>Week</strong>, and <strong>Day</strong>. Switch between them with the tabs at the top. Use the arrow buttons (or keyboard ← →) to navigate, and the Today button to jump back.</P>
    <H>Creating appointments</H>
    <P>In <strong>Week</strong> or <strong>Day</strong> view, turn on <strong>Edit mode</strong> (the ✏️ button), then click and drag across empty space to create an appointment. A modal appears where you select the patient and appointment type.</P>
    <H>Rescheduling</H>
    <P>In Edit mode, drag any existing appointment to a new time slot. The appointment moves instantly. Blocked times can also be dragged.</P>
    <H>Blocking time</H>
    <P>Drag across empty space and choose "Block Time" instead of booking an appointment. Use <strong>Mass Block</strong> to block the same time (e.g. lunch) across multiple days or weekdays at once.</P>
    <H>Appointment details</H>
    <P>Click any appointment (when not in Edit mode) to see a side panel with the patient summary, slot details, payment status, and a link to open the full patient file. You can also add pre/post-visit notes from here.</P>
    <Callout type="info">The coloured badges on appointments show payment status: green ✓ Paid, amber $X due.</Callout>
  </div>;
}

function Intake() {
  return <div>
    <H>Online Intake & Bookings</H>
    <P>The intake system lets patients book appointments online through a public web form. Their bookings appear in your app for approval or automatic confirmation.</P>
    <H>The public booking form</H>
    <P>Located at <code>http://localhost:3000/input</code> (or your Cloudflare Worker URL when deployed). Patients can open it on any device. The form:</P>
    <ul className="g-list">
      <li>Asks if they are a new or returning patient</li>
      <li>Shows available appointment types with prices</li>
      <li>Displays a month calendar — they click an available day (highlighted) to see time slots</li>
      <li>Detects returning patients by matching name, DOB, phone, or email</li>
      <li>Submits the booking request to the system</li>
    </ul>
    <H>Booking modes</H>
    <P>In the <strong>Intake</strong> tab → Settings, you choose:</P>
    <ul className="g-list">
      <li><strong>Automatic</strong> — if the requested slot is free, the booking is confirmed instantly and the patient gets a confirmation message</li>
      <li><strong>Approval</strong> — every booking waits for staff approval. You see it in the Intake tab and click Approve or Decline</li>
    </ul>
    <H>Auto-messages</H>
    <P>When a booking is received or confirmed, the system can automatically send the patient an email or SMS. You edit the message templates in Settings. Placeholders <code>{`{{name}}`}</code>, <code>{`{{date}}`}</code>, <code>{`{{time}}`}</code>, <code>{`{{type}}`}</code> are filled in automatically.</P>
    <Callout type="warn">To actually send emails/SMS, you need to add your Postmark (email) or ClickSend (SMS) API key in Settings. Without keys, messages are queued but marked as "skipped".</Callout>
  </div>;
}

function Billing() {
  return <div>
    <H>Billing & Checkout</H>
    <P>OptiCore handles the full billing flow from consultation to payment.</P>
    <H>Checkout</H>
    <P>From any patient file, click <strong>💳 Checkout</strong>. The checkout screen has two columns:</P>
    <ul className="g-list">
      <li><strong>Left:</strong> consultation types and services/products catalog — click to add to the invoice</li>
      <li><strong>Right:</strong> live invoice cart with quantity, discount, and automatic GST calculation</li>
    </ul>
    <P>Choose a payment method (card, cash, EFTPOS, Medicare, insurance) and click Process Payment. The invoice is created and the payment recorded automatically.</P>
    <H>Invoices</H>
    <P>Each patient file shows their full invoice history with totals, amounts paid, and outstanding balances. You can take additional payments on any unpaid invoice.</P>
    <Callout type="info">GST (10%) is calculated automatically on each line item. Discounts are per-line as a percentage.</Callout>
  </div>;
}

function Messages() {
  return <div>
    <H>Messages</H>
    <P>The <strong>Messages</strong> tab is a unified inbox for all your patient communications — email, WhatsApp, and website messages — in one place.</P>
    <H>How it works</H>
    <ul className="g-list">
      <li>Messages are categorized by channel (✉️ email, 💬 WhatsApp, 🌐 website)</li>
      <li>Filter by channel using the tabs at the top</li>
      <li>Click any message to read it in the side panel</li>
      <li>Link a message to a patient (search by name) so you can see it in their file</li>
      <li>Archive messages you have dealt with</li>
    </ul>
    <Callout type="info">In the full version, the Cloudflare Worker receives website contact-form submissions and WhatsApp webhooks, feeding them into this inbox automatically.</Callout>
  </div>;
}

function Analytics() {
  return <div>
    <H>Analytics</H>
    <P>The <strong>Analytics</strong> tab gives you insight into your practice performance across five tabs:</P>
    <ul className="g-list">
      <li><strong>Overview</strong> — KPI cards (revenue, patients, appointments, outstanding), busiest hours, appointment reliability</li>
      <li><strong>Financial</strong> — revenue over time chart, revenue by appointment type, financial summary report</li>
      <li><strong>Patients</strong> — patient growth chart, age demographics, appointment volume</li>
      <li><strong>Website</strong> — visitor and booking traffic, source breakdown (Google/Facebook/direct), conversion rates</li>
      <li><strong>Reports</strong> — interactive reports you can generate on demand (Financial Summary, Patient Demographics, Appointment Analytics, Outstanding Balances)</li>
    </ul>
    <P>Use the 7/30/90-day range buttons to adjust the time window.</P>
  </div>;
}

function Users() {
  return <div>
    <H>Users & Roles</H>
    <P>The <strong>Users</strong> tab (admin only) lets you manage staff accounts. Each user has a role that determines what they can access:</P>
    <ul className="g-list">
      <li><strong>Admin</strong> — full access to everything including user management and settings</li>
      <li><strong>Doctor</strong> — clinical features + billing</li>
      <li><strong>Nurse</strong> — clinical features</li>
      <li><strong>Receptionist</strong> — bookings + patient management</li>
      <li><strong>Read-only</strong> — can view everything but change nothing</li>
    </ul>
    <P>You can create, edit, disable, or delete users. Disabled users cannot log in but their history is preserved. You cannot delete the last active admin account.</P>
    <Callout type="security">Passwords are hashed with Argon2 — they are never stored in plain text. All routes require JWT authentication.</Callout>
  </div>;
}

function Backup() {
  return <div>
    <H>Backup & Data Safety</H>
    <P>Your patient data is critical. OptiCore has multiple layers of protection:</P>
    <H>Data export</H>
    <P>In Settings, you can export the entire database as a versioned JSON snapshot. This can be optionally encrypted with a passphrase. The snapshot includes a version number so future updates can always read older formats.</P>
    <H>Data import</H>
    <P>Import a snapshot to restore data — useful when moving to a new computer or recovering from a backup. The system rejects snapshots from newer app versions (it tells you to update first).</P>
    <H>The 5-layer backup strategy (production)</H>
    <ul className="g-list">
      <li><strong>L1 — WAL</strong> — SQLite write-ahead log (continuous, built-in)</li>
      <li><strong>L2 — Hourly snapshot</strong> — automatic local copy every hour</li>
      <li><strong>L3 — LAN replica</strong> — nightly copy to a second clinic computer</li>
      <li><strong>L4 — Offsite</strong> — encrypted nightly upload to cloud storage</li>
      <li><strong>L5 — Edge replay</strong> — the Cloudflare Worker keeps every inbound booking as a durable ledger</li>
    </ul>
    <Callout type="warn">A backup you have not tested restoring is not a backup. Always test the restore procedure after setting up backups.</Callout>
  </div>;
}

function Architecture() {
  return <div>
    <H>How OptiCore Works Under the Hood</H>
    <P>OptiCore is built with three layers:</P>
    <H>1. Rust backend (the engine)</H>
    <P>The core is a Rust server binary using <strong>axum</strong> (web framework) and <strong>sqlx</strong> (database driver with compile-time SQL checking). It stores all data in a local <strong>SQLite</strong> database file. This runs as a background process when you launch the app.</P>
    <H>2. Tauri desktop shell (the window)</H>
    <P><strong>Tauri</strong> wraps the React frontend in a native desktop window. It is a small, fast, secure shell — much lighter than Electron. The app binary is under 10MB.</P>
    <H>3. React frontend (the interface)</H>
    <P>The user interface is built with <strong>React + TypeScript</strong>. It runs inside the Tauri webview and communicates with the Rust backend via HTTP on localhost:3000.</P>
    <Callout type="security">Every API route that touches patient data requires a valid JWT token. Authentication uses Argon2 password hashing. There are zero unauthenticated endpoints that expose patient information.</Callout>
  </div>;
}

function Sync() {
  return <div>
    <H>How Online Bookings Sync to the App</H>
    <P>This is the key to understanding how the website intake form connects to the desktop app. They do NOT talk directly — they go through a <strong>Cloudflare Worker</strong> as a middleman.</P>
    <H>The flow</H>
    <Step n="1" title="Patient books online">{"The patient fills in the intake form on the website. The form submits to the Cloudflare Worker (which is always online, even if the clinic is closed)."}</Step>
    <Step n="2" title="Worker stores the booking">{"The Worker saves the booking in its D1 database and marks the requested slot as provisionally booked (so no one else can take it)."}</Step>
    <Step n="3" title="Desktop app syncs">{"Every 30 seconds, the desktop app (when running) connects to the Worker and pulls any new bookings. It also pushes its current availability up, so the website always shows accurate free slots."}</Step>
    <Step n="4" title="Booking appears in the app">{"The synced booking appears in the Intake tab. Depending on your booking mode setting, it is either auto-confirmed or waits for your approval."}</Step>
    <Step n="5" title="Confirmation sent">{"If auto-messages are enabled, the patient receives an email or SMS confirmation automatically."}</Step>
    <H>What if the clinic computer is off?</H>
    <Callout type="info">Bookings are safely queued on the Cloudflare Worker. When the clinic computer starts and the app launches, it pulls all pending bookings. Nothing is lost. The website shows the slots as provisionally booked in the meantime.</Callout>
    <H>Setting up sync</H>
    <P>To enable online sync, set these environment variables before launching the app:</P>
    <pre className="g-code">WORKER_URL=https://your-worker.workers.dev
SYNC_SECRET=your-shared-secret</pre>
    <P>The <code>SYNC_SECRET</code> must match the secret configured on the Worker. You can check sync status at <code>/api/sync/status</code> and trigger an immediate sync at <code>/api/sync/now</code>.</P>
  </div>;
}

function LAN() {
  return <div>
    <H>Use OptiCore From Any Device on Your Network</H>
    <P>OptiCore isn't limited to the computer it runs on. Any tablet, laptop, or desktop on the same network can open the full PMS in a web browser — no app install needed. Every device reads and writes the <strong>same database</strong>, so everyone sees the same data in real time.</P>

    <Callout type="tip">Reception desk on a desktop, optometrist on a tablet, practice manager on a laptop — all hitting the same shared database. No sync buttons, no "refresh to see changes" — it's live.</Callout>

    <H>How It Works</H>
    <P>The OptiCore server listens on <strong>all network interfaces</strong> (not just localhost), so any device that can reach the server computer over WiFi or Ethernet can connect. The database uses <strong>WAL mode</strong> (Write-Ahead Logging), which lets multiple devices read and write simultaneously without "database is locked" errors.</P>

    <H>Step-by-Step Setup</H>
    <Step n="1" title="Start the server on the main computer">Launch OptiCore on the computer that will act as the server (the one that stays on during clinic hours). The server prints its address in the console: <code>🩺 OptiCore server listening on http://0.0.0.0:3000</code></Step>
    <Step n="2" title="Find the server computer's IP address">On the server computer, open a terminal and run:<br /><pre className="g-code">ip addr | grep "inet " | grep -v 127.0.0.1</pre>You'll see something like <code>192.168.0.11</code>. That's the address other devices use. (On Windows, run <code>ipconfig</code> in Command Prompt and look for the IPv4 Address.)</Step>
    <Step n="3" title="Open the browser on another device">On your tablet, laptop, or phone — anything connected to the <strong>same WiFi network</strong> — open a web browser and navigate to:<br /><pre className="g-code">http://192.168.0.11:3000</pre>(replace with your actual IP from step 2). The full OptiCore interface loads — login, patients, calendar, billing, everything.</Step>
    <Step n="4" title="Log in">Use the same credentials as the desktop app. The default first-login is <code>admin</code> / <code>admin</code>. Each staff member can log in from any device with their own account.</Step>

    <H>Accessing From Outside the Clinic (Remote)</H>
    <P>If you need to access OptiCore from outside the clinic WiFi — from home, or from a different building — use <strong>Tailscale</strong> (free, recommended) or any VPN:</P>
    <Step n="1" title="Install Tailscale">Install Tailscale on the server computer and on the remote device. Both join the same Tailnet.</Step>
    <Step n="2" title="Use the Tailscale IP">The server computer gets a Tailscale IP (e.g. <code>100.73.134.20</code>). From the remote device, navigate to <code>http://100.73.134.20:3000</code>. It works exactly like being on the clinic WiFi.</Step>
    <Callout type="security">Tailscale creates an encrypted WireGuard tunnel between your devices. The traffic is end-to-end encrypted — safer than port-forwarding or exposing the server to the public internet. Never port-forward port 3000 to the public internet without additional security layers.</Callout>

    <H>Firewall</H>
    <P>If another device can't connect (browser says "connection refused" or times out), the server computer's firewall may be blocking port 3000. On Linux, open it with:</P>
    <pre className="g-code">sudo firewall-cmd --add-port=3000/tcp --permanent && sudo firewall-cmd --reload
# or with ufw:
sudo ufw allow 3000/tcp</pre>
    <P>On Windows, a firewall prompt may appear the first time you start the server — click "Allow access". On macOS, the firewall is usually permissive for local network services.</P>

    <H>Troubleshooting</H>
    <div className="g-faq">
      <div className="g-faq-item">
        <strong>"This site can't be reached" / connection refused</strong>
        <P>The server isn't running, or the firewall is blocking port 3000. First check the server is up: on the server computer, open <code>http://localhost:3000/api/health</code> in a browser — you should see a JSON status response. If that works but other devices can't connect, it's the firewall.</P>
      </div>
      <div className="g-faq-item">
        <strong>UI loads but it's blank / API errors in console</strong>
        <P>This shouldn't happen — the frontend uses relative URLs so it automatically targets whatever address you loaded it from. Try a hard refresh (Ctrl+Shift+R or Cmd+Shift+R) to clear cached files.</P>
      </div>
      <div className="g-faq-item">
        <strong>"Database is locked" error</strong>
        <P>Extremely rare with WAL mode (which allows concurrent readers + one writer). If it persists, it means sustained heavy write contention from many devices at once. For a typical clinic (a few devices), this never happens.</P>
      </div>
      <div className="g-faq-item">
        <strong>Changes not appearing on another device</strong>
        <P>All devices share the same database file, so writes are immediately visible. If something looks stale, hard-refresh the browser (Ctrl+Shift+R).</P>
      </div>
    </div>

    <Callout type="info">The server computer must stay on and awake while other devices are using it. If it sleeps or shuts down, all other devices lose connection until it's back. Set the server computer's power settings to never sleep during clinic hours.</Callout>
  </div>;
}

function FAQ() {
  return <div>
    <H>Frequently Asked Questions</H>
    <div className="g-faq">
      <div className="g-faq-item">
        <strong>Q: Where is my data stored?</strong>
        <P>In a local SQLite database file (<code>pms.db</code>) on the computer running the app. It never leaves your machine unless you enable Cloudflare sync or manually export.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: What happens if the computer crashes?</strong>
        <P>Restore from your latest data export (Settings → Import). In production, the hourly snapshots and LAN replica mean you lose at most one hour of data.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: Can I use it on multiple computers?</strong>
        <P>Yes — tablets, laptops, and desktops can all access the same data. One computer runs as the server, and every other device opens <code>http://&lt;server-IP&gt;:3000</code> in a browser. See the <strong>🌐 Multi-Device (LAN)</strong> section above for the full guide.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: Does it work without internet?</strong>
        <P>Yes. The app runs entirely locally. Only the online intake form needs internet (via the Cloudflare Worker). If internet is down, bookings queue on the Worker.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: How do I update the app?</strong>
        <P>The app checks for updates automatically on launch. When a new version is available, it downloads and installs it. Your data is preserved through version-safe snapshots.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: Is it secure?</strong>
        <P>Yes. Passwords are Argon2-hashed. Every patient-data endpoint requires JWT authentication. The database can be encrypted at rest. There are no unauthenticated backdoors.</P>
      </div>
      <div className="g-faq-item">
        <strong>Q: Can I sell this to other clinics?</strong>
        <P>Yes — OptiCore is designed multi-tenant. Each clinic gets its own database, its own Worker configuration, and a license key. The architecture supports selling it as a product.</P>
      </div>
    </div>
  </div>;
}

const GUIDE_STYLE = `
.guide-layout { display: flex; height: 100%; overflow: hidden; }
.guide-sidebar { width: 280px; flex-shrink: 0; border-right: 1px solid var(--border); padding: 24px 16px; overflow-y: auto; }
.guide-title { font-size: 22px; font-weight: 700; }
.guide-sub { color: var(--text-dim); font-size: 13px; margin-top: 2px; margin-bottom: 20px; }
.guide-nav { display: flex; flex-direction: column; gap: 4px; }
.guide-nav-item { display: flex; flex-direction: column; gap: 1px; padding: 10px 12px; border-radius: 8px; text-align: left; background: transparent; color: var(--text-dim); border: none; cursor: pointer; transition: all 0.12s; }
.guide-nav-item:hover { background: var(--bg-elev-2); color: var(--text); }
.guide-nav-item.active { background: var(--accent-soft); color: var(--accent); }
.guide-nav-label { font-size: 14px; font-weight: 600; }
.guide-nav-desc { font-size: 11px; opacity: 0.7; }
.guide-content { flex: 1; overflow-y: auto; padding: 32px 40px; max-width: 760px; }
.g-h2 { font-size: 22px; font-weight: 700; margin: 28px 0 12px; }
.g-h2:first-child { margin-top: 0; }
.g-p { font-size: 15px; line-height: 1.7; color: var(--text); margin-bottom: 14px; }
.g-list { margin: 8px 0 16px 20px; font-size: 15px; line-height: 1.8; }
.g-list li { margin-bottom: 4px; }
.g-step { display: flex; gap: 14px; margin-bottom: 16px; }
.g-step-num { width: 28px; height: 28px; border-radius: 50%; background: var(--accent); color: white; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 14px; flex-shrink: 0; }
.g-step-body { font-size: 14px; color: var(--text-dim); margin-top: 4px; line-height: 1.6; }
.g-callout { display: flex; gap: 10px; padding: 14px 16px; border-radius: 10px; margin: 16px 0; font-size: 14px; line-height: 1.6; }
.g-callout-icon { font-size: 18px; flex-shrink: 0; }
.g-callout-info { background: rgba(79,156,249,0.1); border: 1px solid rgba(79,156,249,0.3); }
.g-callout-warn { background: rgba(251,191,36,0.1); border: 1px solid rgba(251,191,36,0.3); }
.g-callout-tip { background: rgba(74,222,128,0.1); border: 1px solid rgba(74,222,128,0.3); }
.g-callout-security { background: rgba(168,85,247,0.1); border: 1px solid rgba(168,85,247,0.3); }
.g-code { background: var(--bg-elev-2); padding: 2px 6px; border-radius: 4px; font-family: ui-monospace, monospace; font-size: 13px; color: var(--accent); }
.g-faq { display: flex; flex-direction: column; gap: 16px; }
.g-faq-item { padding: 16px; background: var(--bg-elev); border: 1px solid var(--border); border-radius: 10px; }
.g-faq-item strong { font-size: 15px; }
pre.g-code { display: block; padding: 14px; margin: 12px 0; font-size: 13px; line-height: 1.6; overflow-x: auto; }
@media (max-width: 768px) { .guide-layout { flex-direction: column; } .guide-sidebar { width: 100%; max-height: 200px; } .guide-content { padding: 20px; } }
`;
