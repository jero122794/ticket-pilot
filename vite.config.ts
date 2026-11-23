import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuración recomendada por Tauri: puerto fijo, sin limpiar la consola
// (para ver los logs de Rust) y objetivo según la webview de cada plataforma.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      output: {
        // Radix en su propio chunk: el widget y el panel lo comparten en caché.
        manualChunks: {
          radix: [
            "@radix-ui/react-dialog",
            "@radix-ui/react-select",
            "@radix-ui/react-switch",
            "@radix-ui/react-checkbox",
            "@radix-ui/react-tooltip",
          ],
        },
      },
    },
  },
});
