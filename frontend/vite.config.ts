import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// During `tauri dev`, the frontend is served by Tauri on a custom port.
// During plain `npm run dev`, Vite serves on 5173 and proxies /api to the Rust server on 3000.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
