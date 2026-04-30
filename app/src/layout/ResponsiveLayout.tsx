import { Sidebar } from "./Sidebar";
import { BottomNav } from "./BottomNav";
import { Outlet } from "react-router-dom";

interface ResponsiveLayoutProps {
  children?: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  return (
    <div
      className="relative min-h-screen w-full overflow-hidden bg-[rgb(var(--background))] text-[rgb(var(--foreground))] transition-colors duration-500"
    >
      {/* Global atmospheric glow & Orbs */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden z-0">
        {/* Primary Cyan Orb */}
        <div
          className="absolute rounded-full opacity-20 dark:opacity-40 transition-all duration-1000"
          style={{
            top: "15%",
            left: "5%",
            width: "45vw",
            height: "45vw",
            background: "radial-gradient(circle, rgba(0, 219, 233, 0.15) 0%, transparent 70%)",
            filter: "blur(140px)",
          }}
        />
        {/* Secondary Cyan Orb */}
        <div
          className="absolute rounded-full opacity-10 dark:opacity-30 transition-all duration-1000"
          style={{
            bottom: "10%",
            right: "5%",
            width: "40vw",
            height: "40vw",
            background: "radial-gradient(circle, rgba(0, 219, 233, 0.1) 0%, transparent 70%)",
            filter: "blur(120px)",
          }}
        />
        {/* Ambient accent */}
        <div
          className="absolute rounded-full opacity-20 dark:opacity-40"
          style={{
            top: "40%",
            right: "10%",
            width: "30vw",
            height: "30vw",
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
        <div className="md:ml-[96px] h-full overflow-hidden flex flex-col pb-[64px] md:pb-0">
          {children || <Outlet />}
        </div>
      </main>
    </div>
  );
};
