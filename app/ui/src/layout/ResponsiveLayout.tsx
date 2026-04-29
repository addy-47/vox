import React from "react";
import { Sidebar } from "./Sidebar";
import { BottomNav } from "./BottomNav";


interface ResponsiveLayoutProps {
  children: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  return (
    <div
      className="relative h-screen overflow-hidden"
      style={{ background: "#050505", color: "#e5e2e1" }}
    >
      {/* Global atmospheric glow */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden" style={{ zIndex: 0 }}>
        <div
          className="absolute rounded-full"
          style={{
            top: "30%",
            left: "30%",
            width: 800,
            height: 800,
            background: "radial-gradient(circle, rgba(0,219,233,0.06) 0%, transparent 70%)",
            transform: "translate(-50%, -50%)",
            filter: "blur(80px)",
          }}
        />
        <div
          className="absolute rounded-full"
          style={{
            bottom: "0%",
            right: "0%",
            width: 600,
            height: 600,
            background: "radial-gradient(circle, rgba(107,1,204,0.08) 0%, transparent 70%)",
            filter: "blur(100px)",
          }}
        />
      </div>

      {/* Header removed as requested */}

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
          marginLeft: 0,
          height: "100vh",
          zIndex: 1,
        }}
      >
        <div className="md:ml-[96px] h-full overflow-hidden">
          {children}
        </div>
      </main>
    </div>
  );
};
