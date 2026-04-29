import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Database, Monitor, Sun, Moon } from "lucide-react";
import { cn } from "../shared/lib/utils";
import logo from "../assets/logo.webp";

const topNavItems = [
  { icon: Monitor, label: "VOX", path: "/" },
  { icon: Database, label: "LOGS", path: "/history" },
];

const bottomNavItems = [
  { icon: Settings, label: "SYSTEM", path: "/settings" },
];

export const Sidebar: React.FC = () => {
  const [theme, setTheme] = React.useState<'dark' | 'light'>(
    (localStorage.getItem('theme') as 'dark' | 'light') || 'dark'
  );

  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  const toggleTheme = () => setTheme(prev => prev === 'dark' ? 'light' : 'dark');

  return (
    <aside className="fixed left-0 top-0 h-full z-40 flex flex-col bg-[rgb(var(--card))] border-r border-[rgba(var(--border))] shadow-2xl" style={{ width: "96px" }}>
      {/* Top logo area */}
      <div className="flex flex-col items-center justify-center py-8">
        <div
          className="flex items-center justify-center rounded-full overflow-hidden transition-all duration-300 hover:scale-105"
          style={{
            width: 48,
            height: 48,
            background: "linear-gradient(135deg, rgba(var(--accent),0.15) 0%, rgba(var(--accent),0.05) 100%)",
            border: "1px solid rgba(var(--accent),0.3)",
          }}
        >
          <img
            src={logo}
            alt="VOX"
            style={{ width: 32, height: 32, objectFit: "contain" }}
          />
        </div>
      </div>

      {/* Top Nav items */}
      <nav className="flex flex-col items-center gap-2 py-4 flex-1">
        {topNavItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            end={item.path === "/"}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-1 w-full py-4 transition-all duration-200 relative group",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-white/20 hover:text-white/40"
              )
            }
          >
            {({ isActive }) => (
              <>
                <item.icon size={22} strokeWidth={1.5} className={cn("transition-transform duration-300", !isActive && "group-hover:scale-110")} />
                <span
                  className="font-bold uppercase tracking-widest mt-1.5"
                  style={{ fontSize: 9, letterSpacing: "0.15em" }}
                >
                  {item.label}
                </span>
                {isActive && (
                  <div
                    className="absolute left-0 top-4 bottom-4 w-1 rounded-r-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]"
                  />
                )}
              </>
            )}
          </NavLink>
        ))}
      </nav>

      {/* Theme Toggle & Bottom Nav */}
      <div className="flex flex-col items-center gap-2 py-8 border-t border-[rgba(var(--border))]">
        {/* Theme Toggle */}
        <button 
          onClick={toggleTheme}
          className="p-3 mb-4 rounded-xl text-white/20 hover:text-[rgb(var(--accent))] hover:bg-white/[0.03] transition-all"
        >
          {theme === 'dark' ? <Sun size={20} strokeWidth={1.5} /> : <Moon size={20} strokeWidth={1.5} />}
        </button>

        {bottomNavItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-1 w-full py-3 transition-all duration-200 relative group",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-white/20 hover:text-white/40"
              )
            }
          >
            {({ isActive }) => (
              <>
                <item.icon size={22} strokeWidth={1.5} className={cn("transition-transform duration-300", !isActive && "group-hover:scale-110")} />
                <span
                  className="font-bold uppercase tracking-widest mt-1.5"
                  style={{ fontSize: 9, letterSpacing: "0.15em" }}
                >
                  {item.label}
                </span>
                {isActive && (
                  <div
                    className="absolute left-0 top-3 bottom-3 w-1 rounded-r-full bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]"
                  />
                )}
              </>
            )}
          </NavLink>
        ))}
      </div>
    </aside>
  );
};
