import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Database, Monitor, Activity } from "lucide-react";
import { cn } from "@/shared/lib/utils";

const navItems = [
  { icon: Monitor, label: "Home", path: "/" },
  { icon: Database, label: "Memory", path: "/history" },
  { icon: Settings, label: "System", path: "/settings" },
];

export const EdgeNav: React.FC = () => {
  return (
    <nav className="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 px-3 py-1.5 rounded-full border border-[rgba(var(--accent),0.08)] bg-black/44 backdrop-blur-xl shadow-[0_10px_30px_rgba(0,0,0,0.5)] h-[56px] transition-all duration-300">
      {navItems.map((item) => (
        <NavLink
          key={item.label}
          to={item.path}
          end={item.path === "/"}
          className={({ isActive }) =>
            cn(
              "relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5",
              isActive && "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/8 border border-[rgb(var(--accent))]/15 shadow-[inset_0_1px_0_0_rgba(255,255,255,0.05)]"
            )
          }
        >
          {({ isActive }) => (
            <>
              <item.icon
                size={20}
                strokeWidth={isActive ? 2 : 1.5}
                className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
              />
              
              {/* Tooltip */}
              <span className="absolute bottom-14 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none px-2.5 py-1 rounded-md text-[11px] font-bold tracking-wider uppercase bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-lg">
                {item.label}
              </span>

              {/* Active Indicator dot */}
              {isActive && (
                <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]" />
              )}
            </>
          )}
        </NavLink>
      ))}

      {/* Mobile-only Activity item */}
      <NavLink
        to="/monitoring"
        className={({ isActive }) =>
          cn(
            "md:hidden relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5",
            isActive && "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/8 border border-[rgb(var(--accent))]/15 shadow-[inset_0_1px_0_0_rgba(255,255,255,0.05)]"
          )
        }
        aria-label="Engine Monitor"
      >
        {({ isActive }) => (
          <>
            <Activity
              size={20}
              strokeWidth={isActive ? 2 : 1.5}
              className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
            />
            {/* Tooltip */}
            <span className="absolute bottom-14 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none px-2.5 py-1 rounded-md text-[11px] font-bold tracking-wider uppercase bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-lg">
              Monitor
            </span>
            {/* Active Indicator dot */}
            {isActive && (
              <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]" />
            )}
          </>
        )}
      </NavLink>
    </nav>
  );
};
