import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Database, Share2, Monitor } from "lucide-react";
import { cn } from "../shared/lib/utils";
import logo from "../assets/logo.webp";

const navItems = [
  { icon: Monitor, label: "INTELLIGENCE", path: "/" },
  { icon: Database, label: "MEMORY", path: "/history" },
  { icon: Share2, label: "NETWORK", path: "/history" },
  { icon: Settings, label: "SYSTEM", path: "/settings" },
];

export const Sidebar: React.FC = () => {
  return (
    <aside className="fixed left-0 top-0 h-full z-40 flex flex-col" style={{ width: "72px" }}>
      {/* Top logo area */}
      <div
        className="flex flex-col items-center justify-center py-6"
        style={{ borderBottom: "1px solid rgba(255,255,255,0.06)" }}
      >
        <div
          className="flex items-center justify-center rounded-full overflow-hidden"
          style={{
            width: 40,
            height: 40,
            background: "rgba(0,219,233,0.12)",
            border: "1.5px solid rgba(0,219,233,0.3)",
          }}
        >
          <img
            src={logo}
            alt="VOX"
            style={{ width: 28, height: 28, objectFit: "contain", objectPosition: "center" }}
          />
        </div>
        <span
          className="mt-2 font-bold tracking-widest uppercase"
          style={{ fontSize: 9, color: "#00dbe9", letterSpacing: "0.2em" }}
        >
          VOX-1
        </span>
      </div>

      {/* Nav items */}
      <nav className="flex flex-col items-center gap-1 py-6 flex-1">
        {navItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            end={item.path === "/"}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-1 w-full py-4 transition-all duration-200 relative",
                isActive
                  ? "text-[#00dbe9]"
                  : "text-slate-600 hover:text-slate-400"
              )
            }
          >
            {({ isActive }) => (
              <>
                {isActive && (
                  <div
                    className="absolute right-0 top-2 bottom-2 rounded-l-sm"
                    style={{ width: 2, background: "#00dbe9" }}
                  />
                )}
                <item.icon size={18} strokeWidth={1.5} />
                <span
                  className="font-bold uppercase tracking-widest"
                  style={{ fontSize: 7, letterSpacing: "0.15em" }}
                >
                  {item.label}
                </span>
              </>
            )}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
};
