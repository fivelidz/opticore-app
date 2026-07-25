import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { auth } from "../api";
import { useTheme } from "../theme";

export function Login() {
  const nav = useNavigate();
  const { theme, toggle } = useTheme();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [err, setErr] = useState("");
  const [loading, setLoading] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    setLoading(true);
    try {
      const { data } = await auth.login(username, password);
      localStorage.setItem("pms_token", data.token);
      localStorage.setItem("pms_user", JSON.stringify(data.user));
      nav("/", { replace: true });
    } catch (e: any) {
      setErr(e?.response?.data?.error || "Login failed. Check the server is running.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-page">
      <button className="theme-pill" onClick={toggle}>
        {theme === "dark" ? "☀️" : "🌙"}
      </button>
      <div className="login-card card">
        <div className="login-brand">👁️</div>
        <h1>OptiCore</h1>
        <p className="login-sub">Practice Management System</p>

        <form onSubmit={submit}>
          <label>Username</label>
          <input value={username} onChange={(e) => setUsername(e.target.value)} autoFocus />
          <label style={{ marginTop: 14 }}>Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Enter password"
          />
          {err && <div className="login-err">{err}</div>}
          <button type="submit" className="btn-primary login-btn" disabled={loading}>
            {loading ? "Signing in…" : "Sign in"}
          </button>
        </form>
        <p className="login-hint">
          The admin password is shown in the server window on first run.
        </p>
      </div>

      <style>{`
        .login-page {
          height: 100vh; display: flex; align-items: center; justify-content: center;
          background: radial-gradient(circle at 50% 30%, var(--bg-elev), var(--bg));
        }
        .theme-pill {
          position: absolute; top: 20px; right: 20px;
          width: 40px; height: 40px; border-radius: 50%;
          background: var(--bg-elev); border: 1px solid var(--border);
          font-size: 18px;
        }
        .login-card { width: 380px; text-align: center; }
        .login-brand { font-size: 48px; margin-bottom: 8px; }
        .login-card h1 { font-size: 20px; }
        .login-sub { color: var(--text-dim); margin-bottom: 28px; font-size: 14px; }
        .login-card label { display: block; text-align: left; font-size: 12px; font-weight: 600; color: var(--text-dim); margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.5px; }
        .login-err { color: var(--red); font-size: 13px; margin-top: 12px; }
        .login-btn { width: 100%; margin-top: 20px; padding: 11px; font-size: 15px; }
        .login-hint { font-size: 12px; color: var(--text-dim); margin-top: 18px; }
      `}</style>
    </div>
  );
}
