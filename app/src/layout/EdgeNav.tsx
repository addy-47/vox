import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Monitor, Activity, Clock, Database } from "lucide-react";
import { cn } from "@/shared/lib/utils";

const navItems = [
  { icon: Monitor, label: "Home", path: "/" },
  { icon: Clock, label: "History", path: "/history" },
  { icon: Database, label: "Memory", path: "/memory" },
  { icon: Settings, label: "System", path: "/settings" },
];

export const EdgeNav: React.FC = () => {
  return (
    <>
      {/* Soft glass/fade mask backdrop behind floating nav for small-screen layouts */}
      <div
        className="lg:hidden fixed bottom-0 left-0 right-0 h-[110px] pointer-events-none z-40 bg-gradient-to-b from-transparent via-[rgb(var(--background))]/60 to-[rgb(var(--background))]/95 backdrop-blur-[16px]"
        style={{
          WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 35%, black 100%)",
          maskImage: "linear-gradient(to bottom, transparent 0%, black 35%, black 100%)",
        }}
      />

      <nav className="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 px-3 py-1.5 h-[56px] glass-card border border-[rgba(var(--accent),0.15)] rounded-full shadow-2xl">
        {navItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            end={item.path === "/"}
            className={({ isActive }) =>
              cn(
                "relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5",
                isActive && "text-[rgb(var(--accent))] bg-transparent"
              )
            }
          >
            {({ isActive }) => (
              <>
                <item.icon
                  size={24}
                  strokeWidth={isActive ? 2 : 1.5}
                  className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
                />
                
                {/* Tooltip */}
                <span className="absolute bottom-14 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none px-2.5 py-1 rounded-md text-[12px] font-bold tracking-wider uppercase bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-lg">
                  {item.label}
                </span>

                {/* Active Indicator dot */}
                {isActive && (
                  <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                )}
              </>
            )}
          </NavLink>
        ))}

        {/* Compact layout — monitoring in EdgeNav instead of corner */}
        <NavLink
          to="/monitoring"
          className={({ isActive }) =>
            cn(
              "lg:hidden relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5",
              isActive && "text-[rgb(var(--accent))] bg-transparent"
            )
          }
          aria-label="Engine Monitor"
        >
          {({ isActive }) => (
            <>
              <Activity
                size={24}
                strokeWidth={isActive ? 2 : 1.5}
                className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
              />
              {/* Tooltip */}
              <span className="absolute bottom-14 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none px-2.5 py-1 rounded-md text-[12px] font-bold tracking-wider uppercase bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-lg">
                Monitor
              </span>
              {/* Active Indicator dot */}
              {isActive && (
                <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
              )}
            </>
          )}
        </NavLink>
      </nav>
    </>
  );
};
