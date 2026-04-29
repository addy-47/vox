import React from "react";
import { Sidebar } from "./Sidebar";
import { BottomNav } from "./BottomNav";
import { Settings, User } from "lucide-react";

interface ResponsiveLayoutProps {
  children: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  return (
    <div
      className="relative min-h-screen"
      style={{ background: "#050505", color: "#e5e2e1" }}
    >
      {/* Global atmospheric glow */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden" style={{ zIndex: 0 }}>
        <div
          className="absolute rounded-full"
          style={{
            top: "30%",
            left: "30%",
            width: 600,
            height: 600,
            background: "radial-gradient(circle, rgba(0,219,233,0.04) 0%, transparent 70%)",
            transform: "translate(-50%, -50%)",
          }}
        />
        <div
          className="absolute rounded-full"
          style={{
            bottom: "10%",
            right: "10%",
            width: 400,
            height: 400,
            background: "radial-gradient(circle, rgba(107,1,204,0.05) 0%, transparent 70%)",
          }}
        />
      </div>

      {/* Top header bar */}
      <header
        className="fixed top-0 left-0 right-0 z-50 flex items-center justify-between px-6"
        style={{
          height: 56,
          background: "rgba(5,5,5,0.8)",
          backdropFilter: "blur(20px)",
          borderBottom: "1px solid rgba(255,255,255,0.06)",
          // push right of sidebar on desktop
        }}
      >
        <div className="flex items-center gap-3 md:ml-[72px]">
          <span
            className="font-bold tracking-widest"
            style={{ color: "#00dbe9", fontSize: 18, fontFamily: "'Space Grotesk', sans-serif" }}
          >
            VOX
          </span>
          <div style={{ width: 1, height: 18, background: "rgba(255,255,255,0.15)" }} />
          <span
            className="font-mono tracking-widest uppercase"
            style={{ fontSize: 11, color: "rgba(255,255,255,0.35)" }}
          >
            NEURAL_INTERFACE_V1.0
          </span>
        </div>
        <div className="flex items-center gap-3">
          <button
            className="flex items-center justify-center rounded-full transition-all hover:bg-white/10"
            style={{ width: 36, height: 36, color: "rgba(255,255,255,0.4)" }}
          >
            <Settings size={16} strokeWidth={1.5} />
          </button>
          <button
            className="flex items-center justify-center rounded-full transition-all"
            style={{
              width: 36,
              height: 36,
              background: "rgba(0,219,233,0.12)",
              border: "1.5px solid rgba(0,219,233,0.25)",
              color: "#00dbe9",
            }}
          >
            <User size={16} strokeWidth={1.5} />
          </button>
        </div>
      </header>

      {/* Left sidebar (desktop only) */}
      <div className="hidden md:block">
        <Sidebar />
      </div>

      {/* Bottom nav (mobile only) */}
      <BottomNav />

      {/* Main content area */}
      <main
        className="relative"
        style={{
          paddingTop: 56, // header height
          marginLeft: 0,
          minHeight: "100vh",
          zIndex: 1,
        }}
      >
        <div className="md:ml-[72px] h-full">
          {children}
        </div>
      </main>
    </div>
  );
};
