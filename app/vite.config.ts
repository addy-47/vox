import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { visualizer } from "rollup-plugin-visualizer";
import inspect from "vite-plugin-inspect";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig(async ({ mode }) => ({
  plugins: [
    react(),
    tailwindcss(),
    // Dev-only: bundle inspection tools. Loading them in production builds
    // slows `pnpm build` and risks shipping the inspect middleware client.
    ...(mode === "development"
      ? [
          inspect(),
          visualizer({
            filename: "dist/stats.html",
            gzipSize: true,
            brotliSize: true,
            open: false,
          }),
        ]
      : []),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@shared": path.resolve(__dirname, "./src/shared"),
      "@layout": path.resolve(__dirname, "./src/layout"),
      "@pages": path.resolve(__dirname, "./src/pages"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  // Dev cold-start: see `server.warmup` below — the pre-bundle + graph warm-up
  // targets the loader → Home transition in `tauri dev`, and the lazy route
  // pages joined as entries so heavyweight deps are pre-bundled instead of
  // being discovered mid-session (which triggers a full page reload that
  // tears down every live IPC callback).
  optimizeDeps: {
    entries: [
      "index.html",
      "src/main.tsx",
      "src/pages/History.tsx",
      "src/pages/Memory.tsx",
      "src/pages/Settings.tsx",
      "src/pages/Monitoring.tsx",
      "src/tray/TrayApp.tsx",
    ],
    include: [
      "react",
      "react-dom",
      "react-router-dom",
      "zustand",
      "framer-motion",
      "lucide-react",
      "three",
      "clsx",
      "tailwind-merge",
      "@tanstack/react-virtual",
      "@tauri-apps/api",
      "@tauri-apps/plugin-dialog",
      "@tauri-apps/plugin-fs",
      "@tauri-apps/plugin-opener",
      "@tauri-apps/plugin-process",
      "@tauri-apps/plugin-updater",
      "react-markdown",
      "lenis",
      "thinking-orbs",
      "react-colorful",
      "@floating-ui/react",
      "@noriginmedia/norigin-spatial-navigation",
    ],
  },
  server: {
    warmup: {
      // Boot-path entries. Warmup resolves each entry's full static import
      // graph (App → layout → Home) before the webview navigates, so the
      // first real request is served from an already-transformed module.
      clientFiles: ["./index.html", "./src/main.tsx", "./src/App.tsx", "./src/pages/Home.tsx"],
    },
    port: 1420,
    strictPort: true,
    host: "0.0.0.0",
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
      // 4. use polling to avoid ENOSPC when system inotify limit is exhausted
      usePolling: true,
      interval: 1000,
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor': ['react', 'react-dom', 'react-router-dom'],
          'three': ['three'],
          'tauri-api': ['@tauri-apps/api'],
          'ui-core': ['lucide-react', 'framer-motion', 'clsx', 'tailwind-merge'],
        },
      },
    },
  },
}));
