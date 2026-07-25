import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { useTheme } from "./theme";
import { useEffect, useState } from "react";
import { auth, type User } from "./api";

export function App() {
  const { theme, toggle } = useTheme();
  const nav = useNavigate();
  const loc = useLocation();
  const [user, setUser] = useState<User | null>(null);

  useEffect(() => {
    const raw = localStorage.getItem("pms_user");
    if (raw) {
      try { setUser(JSON.parse(raw)); } catch { localStorage.removeItem("pms_user"); }
    }
    auth.me().then((r) => setUser(r.data)).catch(() => {});
  }, []);

  const logout = () => {
    localStorage.removeItem("pms_token");
    localStorage.removeItem("pms_user");
    nav("/login");
  };

  const navItem = (path: string, label: string, icon: string) => (
    <button
      className={loc.pathname === path || (path !== "/" && loc.pathname.startsWith(path)) ? "nav-item active" : "nav-item"}
      onClick={() => nav(path)}
    >
      <span className="nav-icon">{icon}</span>
      <span>{label}</span>
    </button>
  );

  return (
    <div className="layout">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">👁️</div>
          <div>
            <div className="brand-title">OptiCore</div>
            <div className="brand-sub">Dry Eye Clinic</div>
          </div>
        </div>

        <nav className="nav">
          {navItem("/", "Dashboard", "📊")}
          {navItem("/patients", "Patients", "👥")}
          {navItem("/calendar", "Calendar", "📅")}
          {navItem("/intake", "Intake", "📥")}
          {navItem("/messages", "Messages", "📬")}
          {navItem("/users", "Users", "🧑‍⚕️")}
          {navItem("/analytics", "Analytics", "📈")}
          {navItem("/settings", "Settings", "⚙️")}
        </nav>

        <div className="sidebar-footer">
          <button className="btn-ghost theme-toggle" onClick={toggle} title="Toggle theme">
            {theme === "dark" ? "☀️ Light" : "🌙 Dark"}
          </button>
          {user && (
            <div className="user-box">
              <div className="user-avatar">
                {(user.first_name?.[0] || "?") + (user.last_name?.[0] || "")}
              </div>
              <div className="user-meta">
                <div className="user-name">{user.first_name} {user.last_name}</div>
                <div className="user-role">{user.role}</div>
              </div>
            </div>
          )}
          <button className="btn-danger logout-btn" onClick={logout}>Sign out</button>
        </div>
      </aside>

      <main className="content">
        <Outlet />
      </main>

      <style>{`
        .layout { display: flex; height: 100vh; overflow: hidden; }
        .sidebar {
          width: 240px; flex-shrink: 0;
          background: var(--bg-elev);
          border-right: 1px solid var(--border);
          display: flex; flex-direction: column;
          padding: 20px 16px;
        }
        .brand { display: flex; align-items: center; gap: 12px; padding: 4px 8px 24px; }
        .brand-mark { font-size: 28px; }
        .brand-title { font-weight: 700; font-size: 16px; }
        .brand-sub { font-size: 12px; color: var(--text-dim); }
        .nav { display: flex; flex-direction: column; gap: 4px; flex: 1; }
        .nav-item {
          display: flex; align-items: center; gap: 12px;
          background: transparent; color: var(--text-dim);
          padding: 10px 12px; border-radius: 8px; text-align: left;
          font-weight: 500; width: 100%;
        }
        .nav-item:hover { background: var(--bg-elev-2); color: var(--text); }
        .nav-item.active { background: var(--accent-soft); color: var(--accent); }
        .nav-icon { font-size: 18px; width: 22px; text-align: center; }
        .sidebar-footer { display: flex; flex-direction: column; gap: 12px; padding-top: 16px; border-top: 1px solid var(--border); }
        .theme-toggle { width: 100%; }
        .user-box { display: flex; align-items: center; gap: 10px; padding: 4px; }
        .user-avatar {
          width: 36px; height: 36px; border-radius: 50%;
          background: var(--accent); color: white;
          display: flex; align-items: center; justify-content: center;
          font-weight: 600; font-size: 13px; flex-shrink: 0;
        }
        .user-name { font-size: 13px; font-weight: 600; }
        .user-role { font-size: 11px; color: var(--text-dim); text-transform: capitalize; }
        .logout-btn { width: 100%; }
        .content { flex: 1; overflow-y: auto; padding: 32px 40px; }
      `}</style>
    </div>
  );
}
