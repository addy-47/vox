import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { visualizer } from "rollup-plugin-visualizer";
import inspect from "vite-plugin-inspect";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [
    react(),
    tailwindcss(),
    inspect(),
    visualizer({
      filename: "dist/stats.html",
      gzipSize: true,
      brotliSize: true,
      open: false,
    }),
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
  server: {
    port: 1420,
    strictPort: true,
    host: host || "0.0.0.0",
    // The HMR websocket host must be reachable FROM THE DEVICE.
    //
    // The `127.0.0.1` default is correct for desktop `tauri dev` (the browser and
    // Vite share a loopback interface) but is wrong on Android: there,
    // `127.0.0.1` is the PHONE's own loopback. With `adb reverse tcp:1420
    // tcp:1420` the phone's loopback:1420 is tunnelled back to this machine, so
    // the websocket must bind loopback for the tunnel to carry it — but only
    // when the reverse tunnel is actually in place. When `TAURI_DEV_HOST` is
    // set (the Android path, see scripts/android-dev.sh) Vite must listen on the
    // LAN address too, because the WebView may reach the dev server over the
    // network instead of the tunnel (e.g. after the reverse mapping is dropped
    // by a reconnect).
    //
    // So: with TAURI_DEV_HOST, HMR goes over the network on 1421; without it,
    // HMR rides the reverse tunnel on 1420. Both ports are reversed by
    // android-dev.sh.
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : {
          protocol: "ws",
          host: "127.0.0.1",
          port: 1420,
        },
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
