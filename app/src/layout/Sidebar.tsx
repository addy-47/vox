import React from "react";
import { NavLink } from "react-router-dom";
import { Settings, Database, Monitor, Sun, Moon, Activity } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import logo from "@/assets/logo.webp";
import logoLight from "@/assets/logo-light.webp";
import { useSettingsStore } from "@/store/settingsStore";

const topNavItems = [
  { icon: Monitor, label: "HOME", path: "/" },
  { icon: Database, label: "memory", path: "/history" },
  { icon: Activity, label: "HEALTH", path: "/monitoring" },
  { icon: Settings, label: "SYSTEM", path: "/settings" },
];

export const Sidebar: React.FC = () => {
  const theme = useSettingsStore(s => s.draftSettings?.ui.theme || 'dark');
  const toggleTheme = useSettingsStore(s => s.toggleTheme);

  return (
    <aside className="fixed left-0 top-0 h-full z-40 flex flex-col glass-surface glass-base border-r border-[rgba(var(--accent),0.06)] w-[96px]">
      {/* Top logo area */}
      <div className="flex flex-col items-center justify-center py-20">
        <div className="relative group">
          {/* Glow ring behind logo */}
          <div className="absolute inset-[-4px] rounded-full bg-[rgb(var(--accent))]/10 blur-md opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
          <div
            className="relative transition-all duration-300 hover:scale-110 active:scale-95"
            style={{ width: 32, height: 32 }}
          >
            <div 
              className="absolute inset-0 bg-[rgb(var(--accent))]"
              style={{
                maskImage: `url(${theme === 'light' ? logoLight : logo})`,
                WebkitMaskImage: `url(${theme === 'light' ? logoLight : logo})`,
                maskSize: 'contain',
                WebkitMaskSize: 'contain',
                maskRepeat: 'no-repeat',
                WebkitMaskRepeat: 'no-repeat',
              }}
            />
          </div>
        </div>
      </div>

      {/* Main Navigation */}
      <nav className="flex flex-col items-center gap-1  flex-1">
        {topNavItems.map((item) => (
          <NavLink
            key={item.label}
            to={item.path}
            end={item.path === "/"}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center gap-2 w-full py-5 transition-all duration-300 relative group hover:bg-[rgb(var(--accent))]/5",
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
          className="p-3 rounded-full glass-surface glass-base text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all duration-300 group"
          aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
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
