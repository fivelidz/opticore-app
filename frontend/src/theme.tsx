import React, { createContext, useContext, useEffect, useState } from "react";

type Theme = "light" | "dark";
interface ThemeCtx {
  theme: Theme;
  toggle: () => void;
}
const Ctx = createContext<ThemeCtx>({ theme: "dark", toggle: () => {} });

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem("pms_theme") as Theme) || "dark"
  );

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("pms_theme", theme);
  }, [theme]);

  return (
    <Ctx.Provider
      value={{
        theme,
        toggle: () => setTheme((t) => (t === "dark" ? "light" : "dark")),
      }}
    >
      {children}
    </Ctx.Provider>
  );
}

export const useTheme = () => useContext(Ctx);
