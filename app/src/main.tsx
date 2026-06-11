import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import { SettingsProvider } from "./shared/context/SettingsContext";

// Lazy load entry points to separate chunks
const App = lazy(() => import("./App"));
const TrayApp = lazy(() => import("./tray/TrayApp").then(m => ({ default: m.TrayApp })));

// Basic window detection for Tauri
const isTray = window.location.pathname.includes("tray") || 
               window.location.search.includes("window=tray") ||
               (window as any).__TAURI_METADATA__?.windowLabel === "tray";

if (isTray) {
  document.documentElement.classList.add("is-tray");
  document.body.classList.add("is-tray");
}

// Global Loading State for Window Initialization
const WindowLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <div className="w-10 h-10 border-2 border-white/5 border-t-white/40 rounded-full animate-spin" />
  </div>
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsProvider>
      <Suspense fallback={<WindowLoader />}>
        {isTray ? <TrayApp /> : <App />}
      </Suspense>
    </SettingsProvider>
  </React.StrictMode>
);
