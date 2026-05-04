import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { TrayApp } from "./tray/TrayApp";
import "./index.css";

// Basic window detection for Tauri
const isTray = window.location.pathname.includes("tray") || 
               window.location.search.includes("window=tray") ||
               (window as any).__TAURI_METADATA__?.windowLabel === "tray";

if (isTray) {
  document.documentElement.classList.add("is-tray");
  document.body.classList.add("is-tray");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isTray ? <TrayApp /> : <App />}
  </React.StrictMode>
);
