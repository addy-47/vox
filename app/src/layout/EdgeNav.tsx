import React from "react";
import { NavLink } from "react-router-dom";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { SlidersHorizontal, House, Activity, History, Network } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { BottomDockFeather } from "@/shared/ui";

const navItems = [
  { icon: House, label: "Home", path: "/" },
  { icon: History, label: "History", path: "/history" },
  { icon: Network, label: "Memory", path: "/memory" },
  { icon: SlidersHorizontal, label: "Settings", path: "/settings" },
];

import { Tooltip } from "@/shared/ui/Tooltip";

export const EdgeNav: React.FC = () => {
  return (
    <>
      {/* Standard bottom-dock feather: dissolves scrolled content (incl. open panels) behind the floating nav.
          z-[38] sits above EdgePanels (z-35) and page drawers (z-30) but below layout docks (z-40) and the nav itself (z-60). */}
      <BottomDockFeather className="fixed bottom-0 left-0 right-0 h-[110px] z-[38]" />

      <nav
        data-edge-nav
        data-spatial-zone="dock"
        className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[60] pointer-events-auto flex items-center gap-2 px-3 py-1.5 h-[56px] glass-card border border-[rgba(var(--accent),0.15)] rounded-full shadow-2xl"
      >
        {navItems.map((item) => (
          <Tooltip key={item.label} label={item.label} side="top">
            <NavLink
              to={item.path}
              end={item.path === "/"}
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
                    className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
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
                  className={cn("transition-transform duration-500", !isActive && "group-hover:scale-110")}
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
    </>
  );
};
