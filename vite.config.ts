import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // Not watching the Rust build output: cargo locks files there and the watcher crashes the dev server.
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    outDir: "dist",
    chunkSizeWarningLimit: 2000,
    // Second page: the Justice Clips notification shown over games.
    rollupOptions: { input: { main: "index.html", overlay: "overlay.html" } },
  },
});

