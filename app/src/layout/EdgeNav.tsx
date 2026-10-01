import React from "react";
import { NavLink } from "react-router-dom";
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
  // NOTE: the click-time `navigatingTo` state and its three decorative rings
  // were removed. The flag was cleared in a useEffect on the same commit that
  // set it, so for a loaded route it lasted exactly one frame and never painted
  // three elements. Route feedback is now the route cross-fade + page loader in
  // ResponsiveLayout / App.
  return (
    <>
      {/* Standard bottom-dock feather: dissolves scrolled content behind the floating nav */}
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
    </>
  );
};
