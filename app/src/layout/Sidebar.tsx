import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Database, Monitor, Sun, Moon } from "lucide-react";
import { cn } from "../shared/lib/utils";
import logo from "../assets/logo.webp";
import logoLight from "../assets/logo-light.webp";

const topNavItems = [
  { icon: Monitor, label: "HOME", path: "/" },
  { icon: Database, label: "memory", path: "/history" },
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
    <aside className="fixed left-0 top-0 h-full z-40 flex flex-col bg-[rgb(var(--sidebar))] border-r border-[rgba(var(--border),0.05)] transition-colors duration-300" style={{ width: "96px" }}>
      {/* Top logo area */}
      <div className="flex flex-col items-center justify-center py-10">
        <div
          className={cn(
            "flex items-center justify-center rounded-full overflow-hidden transition-all duration-300 hover:scale-110 active:scale-95",
            theme === 'light' ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/30 shadow-lg shadow-black/[0.05]" : "bg-gradient-to-br from-[rgb(var(--accent))]/20 to-[rgb(var(--accent))]/5 border-[rgb(var(--accent))]/20"
          )}
          style={{
            width: 52,
            height: 52,
            borderWidth: "1px",
          }}
        >
          <img
            src={theme === 'light' ? logoLight : logo}
            alt="VOX"
            className="w-8 h-8 object-contain transition-all duration-500"
          />
        </div>
      </div>

      {/* Main Navigation */}
      <nav className="flex flex-col items-center gap-1 py-4 flex-1">
        {topNavItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            end={item.path === "/"}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-2 w-full py-5 transition-all duration-300 relative group",
                isActive
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )
            }
          >
            {({ isActive }) => (
              <>
                <item.icon size={20} strokeWidth={isActive ? 2 : 1.5} className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")} />
                <span
                  className="font-bold uppercase tracking-[0.2em]"
                  style={{ fontSize: 8.5 }}
                >
                  {item.label}
                </span>
                {isActive && (
                  <div
                    className="absolute left-0 top-4 bottom-4 w-1 rounded-r-full bg-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.6)]"
                  />
                )}
              </>
            )}
          </NavLink>
        ))}
      </nav>

      {/* Theme Toggle Only at Bottom */}
      <div className="flex flex-col items-center py-10 border-t border-[rgba(var(--border),0.05)]">
        <button
          onClick={toggleTheme}
          className="p-4 rounded-2xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-white/[0.03] transition-all duration-300 group"
          title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
        >
          {theme === 'dark'
            ? <Sun size={20} strokeWidth={1.5} className="group-hover:rotate-45 transition-transform duration-500" />
            : <Moon size={20} strokeWidth={1.5} className="group-hover:-rotate-12 transition-transform duration-500" />
          }
        </button>
      </div>
    </aside>
  );
};
