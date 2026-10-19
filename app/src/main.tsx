import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
// Self-hosted fonts (bundled by Vite) — replaces the render-blocking remote
// Google Fonts @import so first paint never waits on a network round-trip.
import "@fontsource/dm-sans/400.css";
import "@fontsource/dm-sans/500.css";
import "@fontsource/dm-sans/600.css";
import "@fontsource/dm-sans/700.css";
import "@fontsource/sora/400.css";
import "@fontsource/sora/500.css";
import "@fontsource/sora/600.css";
import "@fontsource/sora/700.css";
import "@fontsource/sora/800.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/jetbrains-mono/700.css";
import "./index.css";
import App from "./App";
import { ErrorBoundary } from "./shared/components/common";
import { useSettingsStore } from "./store/settingsStore";

// Stage-2 boot RCA mark: absolute anchor (ms since navigation start) for the
// dev-only boot timeline. Everything after this is post-module-graph.
if (import.meta.env.DEV) {
  console.info(`[boot] main.tsx evaluated @${Math.round(performance.now())}ms`);
}

// Boot settings and model catalog into the store
void useSettingsStore.getState().loadSettings();
void useSettingsStore.getState().loadModelCatalog();

const TrayApp = lazy(() => import("./tray/TrayApp").then(m => ({ default: m.TrayApp })));

// Basic window detection for Tauri
const isTray = window.location.pathname.includes("tray") || 
               window.location.search.includes("window=tray") ||
               window.__TAURI_METADATA__?.windowLabel === "tray";

if (isTray) {
  document.documentElement.classList.add("is-tray");
  document.body.classList.add("is-tray");
  import("./tray/TrayApp").catch(() => {});
}

document.addEventListener("visibilitychange", () => {
  document.body.classList.toggle("is-paused", document.hidden);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary name="Root">
      {isTray ? (
        <Suspense fallback={<div className="h-screen w-full bg-[rgb(var(--background))]" />}>
          <TrayApp />
        </Suspense>
      ) : (
        <App />
      )}
    </ErrorBoundary>
  </React.StrictMode>
);
