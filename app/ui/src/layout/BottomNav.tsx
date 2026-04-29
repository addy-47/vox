import React from "react";
import { NavLink } from "react-router-dom";
import { Mic, MessageSquare, Database } from "lucide-react";
import { cn } from "../shared/lib/utils";

const navItems = [
  { icon: Mic, label: "LISTEN", path: "/" },
  { icon: MessageSquare, label: "CHAT", path: "/history" },
  { icon: Database, label: "MEMORY", path: "/settings" },
];

export const BottomNav: React.FC = () => {
  return (
    <nav
      className="fixed bottom-0 left-0 right-0 z-50 flex items-center justify-around md:hidden"
      style={{
        height: 72,
        background: "rgba(5,5,5,0.85)",
        backdropFilter: "blur(24px)",
        borderTop: "1px solid rgba(255,255,255,0.06)",
      }}
    >
      {navItems.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.path === "/"}
          className={({ isActive }) =>
            cn(
              "flex flex-col items-center justify-center gap-1.5 flex-1 py-3 transition-all duration-200",
              isActive ? "text-[#00dbe9]" : "text-slate-600 hover:text-slate-400"
            )
          }
        >
          {({ isActive }) => (
            <>
              <div
                className={cn(
                  "flex items-center justify-center rounded-xl transition-all",
                  isActive
                    ? "bg-[#00dbe9]/15 p-2"
                    : "p-2"
                )}
              >
                <item.icon size={20} strokeWidth={1.5} />
              </div>
              <span className="font-bold uppercase tracking-widest" style={{ fontSize: 9 }}>
                {item.label}
              </span>
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
};
