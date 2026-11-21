import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// In development the UI talks to the backend through this proxy, so the
// browser sees one origin. In production fluxd serves the built UI itself
// (`fluxd serve --ui-dir ui/dist`).
const fluxd = process.env.FLUXD_URL ?? "http://127.0.0.1:7700";
const agent = process.env.FLUX_AGENT_URL ?? "http://127.0.0.1:7701";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: "127.0.0.1",
    port: Number(process.env.FLUX_UI_PORT ?? 5173),
    strictPort: true,
    proxy: {
      "/v1": { target: fluxd, changeOrigin: true },
      "/agent": { target: agent, changeOrigin: true, rewrite: (path) => path.replace(/^\/agent/, "") },
    },
  },
  build: {
    target: "es2022",
    sourcemap: true,
    chunkSizeWarningLimit: 900,
  },
});
