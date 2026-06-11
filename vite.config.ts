import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// Tauri expects a fixed dev-server port and ignores Vite's own console clearing.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Don't watch the Rust side from the Vite dev server.
      ignored: ["**/src-tauri/**"],
    },
  },
  // Produce output the Tauri config points at (`frontendDist: "../dist"`).
  build: {
    target: "es2021",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: false,
  },
  // Only env vars prefixed like this are exposed to the client.
  envPrefix: ["VITE_", "TAURI_"],
});
