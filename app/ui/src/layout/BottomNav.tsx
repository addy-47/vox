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
      className="fixed bottom-0 left-0 right-0 z-50 flex items-center justify-around md:hidden bg-[#0e0e0e] border-t border-white/5 h-[80px] pb-safe"
    >
      {navItems.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.path === "/"}
          className={({ isActive }) =>
            cn(
              "flex flex-col items-center justify-center gap-1.5 flex-1 py-3 transition-all duration-300",
              isActive ? "text-[#00dbe9]" : "text-white/20 hover:text-white/40"
            )
          }
        >
          <div
            className={cn(
              "flex items-center justify-center rounded-xl transition-all duration-300",
              "p-2"
            )}
          >
            <item.icon size={22} strokeWidth={isActive => isActive ? 2 : 1.5} />
          </div>
          <span className="font-bold uppercase tracking-[0.2em] text-[8px]">
            {item.label}
          </span>
          {/* Active indicator dot */}
          <div className={cn(
            "w-1 h-1 rounded-full bg-[#00dbe9] transition-all duration-500",
            "opacity-0 scale-0",
            "mt-1"
          )} 
          style={{ opacity: 0 }} // Simplified for now as isActive logic in className handles it
          />
        </NavLink>
      ))}
    </nav>
  );
};
