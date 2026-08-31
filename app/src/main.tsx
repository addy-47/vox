import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";
import { SettingsProvider } from "./shared/context/SettingsContext";
import { ErrorBoundary } from "./shared/components/common";

const TrayApp = lazy(() => import("./tray/TrayApp").then(m => ({ default: m.TrayApp })));
const ToastApp = lazy(() => import("./toast/ToastApp").then(m => ({ default: m.ToastApp })));

// Basic window detection for Tauri
const isTray = window.location.pathname.includes("tray") || 
               window.location.search.includes("window=tray") ||
               (window as any).__TAURI_METADATA__?.windowLabel === "tray";
const isToast = window.location.pathname.includes("toast") ||
                window.location.search.includes("window=toast") ||
                (window as any).__TAURI_METADATA__?.windowLabel === "toast";

if (isTray) {
  document.documentElement.classList.add("is-tray");
  document.body.classList.add("is-tray");
  import("./tray/TrayApp").catch(() => {});
}
if (isToast) {
  document.documentElement.classList.add("is-toast");
  document.body.classList.add("is-toast");
  import("./toast/ToastApp").catch(() => {});
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary name="Root">
      <SettingsProvider>
        {isToast ? (
          <Suspense fallback={<div className="h-screen w-full bg-transparent" />}>
            <ToastApp />
          </Suspense>
        ) : isTray ? (
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
