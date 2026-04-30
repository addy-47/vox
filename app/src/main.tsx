import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { TrayApp } from "./tray/TrayApp";
import "./index.css";

// Basic window detection for Tauri
const isTray = window.location.pathname === "/tray" || 
               window.location.search.includes("window=tray") ||
               (window as any).__TAURI_METADATA__?.windowLabel === "tray";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isTray ? <TrayApp /> : <App />}
  </React.StrictMode>
);
