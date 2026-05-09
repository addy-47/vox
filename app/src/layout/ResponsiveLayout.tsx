import React from "react";
import { Sidebar } from "./Sidebar";
import { BottomNav } from "./BottomNav";
import { TitleBar } from "./TitleBar";
import { Outlet } from "react-router-dom";

interface ResponsiveLayoutProps {
  children?: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        width: "100%",
        overflow: "hidden",
      }}
      className="bg-[rgb(var(--background))] text-[rgb(var(--foreground))]"
    >
      <TitleBar />
      {/* ── Content Area ──────────────────────────────────────────────────── */}
      <div style={{ flex: 1, display: "flex", overflow: "hidden", position: "relative", minHeight: 0 }}>
        {/* Resize Handles (Invisible, for cursor hit-testing on Linux) */}
        <div className="absolute top-0 left-0 w-full h-[3px] cursor-ns-resize z-[100]" />
        <div className="absolute bottom-0 left-0 w-full h-[3px] cursor-ns-resize z-[100]" />
        <div className="absolute top-0 left-0 h-full w-[3px] cursor-ew-resize z-[100]" />
        <div className="absolute top-0 right-0 h-full w-[3px] cursor-ew-resize z-[100]" />
        
        {/* Corner Handles */}
        <div className="absolute top-0 left-0 w-2 h-2 cursor-nwse-resize z-[110]" />
        <div className="absolute top-0 right-0 w-2 h-2 cursor-nesw-resize z-[110]" />
        <div className="absolute bottom-0 left-0 w-2 h-2 cursor-nesw-resize z-[110]" />
        <div className="absolute bottom-0 right-0 w-2 h-2 cursor-nwse-resize z-[110]" />

        {/* Background atmospheric orbs — fixed to viewport, pointer-events none */}
        <div style={{ position: "fixed", inset: 0, pointerEvents: "none", zIndex: 0 }}>
          <div style={{ position: "absolute", top: "15%", left: "5%", width: "45vw", height: "45vw", borderRadius: "50%", background: "radial-gradient(circle, rgba(0,219,233,0.15) 0%, transparent 70%)", filter: "blur(140px)", opacity: 0.3 }} />
          <div style={{ position: "absolute", bottom: "10%", right: "5%", width: "40vw", height: "40vw", borderRadius: "50%", background: "radial-gradient(circle, rgba(0,219,233,0.1) 0%, transparent 70%)", filter: "blur(120px)", opacity: 0.15 }} />
          <div style={{ position: "absolute", top: "40%", right: "10%", width: "30vw", height: "30vw", borderRadius: "50%", background: "radial-gradient(circle, rgba(0,219,233,0.05) 0%, transparent 70%)", filter: "blur(100px)", opacity: 0.2 }} />
        </div>

        {/* Left sidebar — desktop only */}
        <div className="hidden md:block">
          <Sidebar />
        </div>

        {/* Bottom nav — mobile only */}
        <BottomNav />

        {/* Page content */}
        <main style={{ position: "relative", zIndex: 10, flex: 1, height: "100%", overflow: "hidden", width: "100%" }}>
          <div className="md:pl-[96px] h-full w-full overflow-hidden flex flex-col pb-[64px] md:pb-0">
            {children || <Outlet />}
          </div>
        </main>
      </div>
    </div>
  );
};
