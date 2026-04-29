import React from "react";
import { Sidebar } from "./Sidebar";
import { BottomNav } from "./BottomNav";

interface ResponsiveLayoutProps {
  children: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  return (
    <div
      className="relative min-h-screen w-full overflow-hidden bg-[rgb(var(--background))] text-[rgb(var(--foreground))] transition-colors duration-500"
    >
      {/* Global atmospheric glow */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden z-0">
        <div
          className="absolute rounded-full opacity-40 dark:opacity-100"
          style={{
            top: "20%",
            left: "10%",
            width: "80vw",
            height: "80vw",
            background: "radial-gradient(circle, rgba(var(--accent),0.1) 0%, transparent 70%)",
            filter: "blur(120px)",
          }}
        />
        <div
          className="absolute rounded-full opacity-30 dark:opacity-100"
          style={{
            bottom: "-10%",
            right: "-10%",
            width: "60vw",
            height: "60vw",
            background: "radial-gradient(circle, rgba(var(--accent),0.05) 0%, transparent 70%)",
            filter: "blur(100px)",
          }}
        />
      </div>

      {/* Left sidebar (desktop only) */}
      <div className="hidden md:block">
        <Sidebar />
      </div>

      {/* Bottom nav (mobile only) */}
      <BottomNav />

      {/* Main content area */}
      <main
        className="relative z-10 w-full h-screen overflow-hidden"
      >
        <div className="md:ml-[96px] h-full overflow-hidden flex flex-col">
          {children}
        </div>
      </main>
    </div>
  );
};
