import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";
import { SettingsProvider } from "./shared/context/SettingsContext";
import { ErrorBoundary } from "./shared/components/common";

const TrayApp = lazy(() => import("./tray/TrayApp").then(m => ({ default: m.TrayApp })));

// Basic window detection for Tauri
const isTray = window.location.pathname.includes("tray") || 
               window.location.search.includes("window=tray") ||
               (window as any).__TAURI_METADATA__?.windowLabel === "tray";

if (isTray) {
  document.documentElement.classList.add("is-tray");
  document.body.classList.add("is-tray");
  import("./tray/TrayApp").catch(() => {});
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary name="Root">
      <SettingsProvider>
        {isTray ? (
          <Suspense fallback={<div className="h-screen w-full bg-[rgb(var(--background))]" />}>
            <TrayApp />
          </Suspense>
        ) : (
          <App />
        )}
      </SettingsProvider>
    </ErrorBoundary>
  </React.StrictMode>
);
