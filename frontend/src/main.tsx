import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { ThemeProvider } from "./theme";
import "./styles.css";
import { App } from "./App";
import { Login } from "./pages/Login";
import { Dashboard } from "./pages/Dashboard";
import { Patients } from "./pages/Patients";
import { PatientDetailPage } from "./pages/PatientDetailPage";
import { CalendarPage } from "./pages/CalendarPage";
import { AnalyticsPage } from "./pages/AnalyticsPage";
import { CheckoutPage } from "./pages/CheckoutPage";
import { IntakePage } from "./pages/IntakePage";
import { MessagesPage } from "./pages/MessagesPage";
import { UsersPage } from "./pages/UsersPage";
import { Settings } from "./pages/Settings";

function Protected({ children }: { children: React.ReactNode }) {
  const token = localStorage.getItem("pms_token");
  if (!token) return <Navigate to="/login" replace />;
  return <>{children}</>;
}

function NotFound() {
  return (
    <div style={{ textAlign: "center", padding: 60 }}>
      <div style={{ fontSize: 48 }}>🔍</div>
      <h2 style={{ margin: "12px 0" }}>Page not found</h2>
      <p style={{ color: "var(--text-dim)", marginBottom: 20 }}>That page doesn't exist.</p>
      <a href="#/" className="btn-primary" style={{ display: "inline-block", padding: "10px 20px", borderRadius: 8, textDecoration: "none", color: "#fff" }}>Back to Dashboard</a>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <HashRouter>
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route path="/" element={<Protected><App /></Protected>}>
            <Route index element={<Dashboard />} />
            <Route path="patients" element={<Patients />} />
            <Route path="patients/:id" element={<PatientDetailPage />} />
            <Route path="checkout/:id" element={<CheckoutPage />} />
            <Route path="calendar" element={<CalendarPage />} />
            <Route path="analytics" element={<AnalyticsPage />} />
            <Route path="intake" element={<IntakePage />} />
            <Route path="messages" element={<MessagesPage />} />
            <Route path="users" element={<UsersPage />} />
            <Route path="settings" element={<Settings />} />
            <Route path="*" element={<NotFound />} />
          </Route>
        </Routes>
      </HashRouter>
    </ThemeProvider>
  </React.StrictMode>
);
