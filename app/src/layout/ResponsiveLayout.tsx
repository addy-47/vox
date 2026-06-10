import React from "react";
import { EdgeNav } from "./EdgeNav";
import { TitleBar } from "./TitleBar";
import { AmbientBackground } from "@/shared/components/AmbientBackground";
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

        {/* Deep Space Ambient Background */}
        <AmbientBackground />

        {/* Unified bottom strip navigation */}
        <EdgeNav />

        {/* Page content */}
        <main style={{ position: "relative", zIndex: 10, flex: 1, height: "100%", overflow: "hidden", width: "100%" }}>
          <div className="h-full w-full overflow-hidden flex flex-col pb-[80px]">
            {children || <Outlet />}
          </div>
        </main>
      </div>
    </div>
  );
};
