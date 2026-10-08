import React from "react";
import { NavLink, useLocation } from "react-router-dom";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { SlidersHorizontal, House, Activity, History, Network } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { BottomDockFeather } from "@/shared/ui";
import { Tooltip } from "@/shared/ui/Tooltip";

const navItems = [
  { icon: House, label: "Home", path: "/" },
  { icon: History, label: "History", path: "/history" },
  { icon: Network, label: "Memory", path: "/memory" },
  { icon: SlidersHorizontal, label: "Settings", path: "/settings" },
];

export const EdgeNav: React.FC = () => {
  const location = useLocation();
  return (
    <nav
      data-edge-nav
      data-spatial-zone="dock"
      className="fixed bottom-[calc(1rem+env(safe-area-inset-bottom))] left-1/2 -translate-x-1/2 z-[60] pointer-events-auto flex items-center gap-2 px-3 py-1.5 h-[56px] glass-card glass-keep-blur border border-[rgba(var(--accent),0.15)] rounded-full"
    >
      {/* Localized feather directly behind floating dock nav */}
      <BottomDockFeather className="absolute -inset-x-6 -bottom-3 -top-6 rounded-full pointer-events-none" />
        {navItems.map((item) => (
          <Tooltip key={item.label} label={item.label} side="top">
            <NavLink
              to={item.path}
              end={item.path === "/"}
              onClick={(e) => {
                if (location.pathname === item.path) {
                  e.preventDefault();
                }
              }}
              className={({ isActive }) =>
                cn(
                  "relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))]",
                  isActive && "text-[rgb(var(--accent))] bg-transparent"
                )
              }
            >
              {({ isActive }) => (
                <>
                  <item.icon
                    size={24}
                    strokeWidth={isActive ? 2 : 1.5}
                    className={cn(
                      "transition-transform duration-500",
                      !isActive && "group-hover:scale-110"
                    )}
                  />

                  {/* Active Indicator dot */}
                  {isActive && (
                    <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                  )}
                </>
              )}
            </NavLink>
          </Tooltip>
        ))}

        {/* Compact layout — monitoring in EdgeNav instead of corner */}
        <Tooltip label={LAYOUT_COPY.nav.monitor} side="top">
          <NavLink
            to="/monitoring"
            onClick={(e) => {
              if (location.pathname === "/monitoring") {
                e.preventDefault();
              }
            }}
            className={({ isActive }) =>
              cn(
                "lg:hidden relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))]",
                isActive && "text-[rgb(var(--accent))] bg-transparent"
              )
            }
            aria-label={LAYOUT_COPY.nav.engineMonitor}
          >
            {({ isActive }) => (
              <>
                <Activity
                  size={24}
                  strokeWidth={isActive ? 2 : 1.5}
                  className={cn(
                    "transition-transform duration-500",
                    !isActive && "group-hover:scale-110"
                  )}
                />

                {/* Active Indicator dot */}
                {isActive && (
                  <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                )}
              </>
            )}
          </NavLink>
        </Tooltip>
      </nav>
  );
};
