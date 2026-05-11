import React from "react";
import { NavLink } from "react-router-dom";
import { Mic, History, Settings } from "lucide-react";
import { cn } from "../shared/lib/utils";

const navItems = [
  { icon: Mic, label: "VOX", path: "/" },
  { icon: History, label: "LOGS", path: "/history" },
  { icon: Settings, label: "SYSTEM", path: "/settings" },
];

export const BottomNav: React.FC = () => {
  return (
    <nav
      className="fixed bottom-0 left-0 right-0 z-50 flex items-center justify-around md:hidden bg-[rgb(var(--sidebar))] border-t border-[rgba(var(--border),0.05)] h-[64px] pb-safe backdrop-blur-3xl transition-colors duration-300"
    >
      {navItems.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.path === "/"}
          className={({ isActive }) =>
            cn(
              "flex flex-col items-center justify-center flex-1 h-full transition-all duration-500 relative",
              isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] opacity-60 hover:opacity-80"
            )
          }
        >
          {({ isActive }) => (
            <>
              <div
                className={cn(
                  "flex items-center justify-center rounded-2xl transition-all duration-500 p-2.5",
                  isActive && "bg-[rgb(var(--accent))]/5 scale-110"
                )}
              >
                <item.icon size={24} strokeWidth={isActive ? 2 : 1.5} />
              </div>
              
              {/* Active indicator dot */}
              {isActive && (
                <div className="absolute bottom-1.5 w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]" />
              )}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
};
