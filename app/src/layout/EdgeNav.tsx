import React, { useState, useEffect } from "react";
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
  const [navigatingTo, setNavigatingTo] = useState<string | null>(null);

  // Clear navigation loader as soon as the active route updates
  useEffect(() => {
    setNavigatingTo(null);
  }, [location.pathname]);

  const handleNavClick = (targetPath: string) => {
    if (location.pathname !== targetPath) {
      setNavigatingTo(targetPath);
    }
  };

  return (
    <>
      {/* Standard bottom-dock feather: dissolves scrolled content behind the floating nav */}
      <BottomDockFeather className="fixed bottom-0 left-0 right-0 h-[110px] z-[38]" />

      <nav
        data-edge-nav
        data-spatial-zone="dock"
        className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[60] pointer-events-auto flex items-center gap-2 px-3 py-1.5 h-[56px] glass-card border border-[rgba(var(--accent),0.15)] rounded-full shadow-2xl"
      >
        {navItems.map((item) => {
          const isNavigatingThis = navigatingTo === item.path;

          return (
            <Tooltip key={item.label} label={item.label} side="top">
              <NavLink
                to={item.path}
                end={item.path === "/"}
                onClick={() => handleNavClick(item.path)}
                className={({ isActive }) =>
                  cn(
                    "relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))]",
                    (isActive || isNavigatingThis) && "text-[rgb(var(--accent))] bg-transparent"
                  )
                }
              >
                {({ isActive }) => (
                  <>
                    <item.icon
                      size={24}
                      strokeWidth={isActive || isNavigatingThis ? 2 : 1.5}
                      className={cn(
                        "transition-transform duration-500",
                        !isActive && !isNavigatingThis && "group-hover:scale-110",
                        isNavigatingThis && "scale-95 animate-pulse"
                      )}
                    />

                    {/* Miniature Orbital Navigation Spinner on click */}
                    {isNavigatingThis && (
                      <>
                        <span className="absolute -inset-1 rounded-full border border-dashed border-[rgb(var(--accent))] animate-spin pointer-events-none opacity-80" />
                        <span className="absolute -inset-0.5 rounded-full border border-[rgb(var(--accent))]/40 animate-pulse pointer-events-none" />
                        <span className="absolute -top-1 left-1/2 -translate-x-1/2 w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgb(var(--accent))] pointer-events-none" />
                      </>
                    )}

                    {/* Active Indicator dot */}
                    {isActive && !isNavigatingThis && (
                      <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                    )}
                  </>
                )}
              </NavLink>
            </Tooltip>
          );
        })}

        {/* Compact layout — monitoring in EdgeNav instead of corner */}
        <Tooltip label={LAYOUT_COPY.nav.monitor} side="top">
          <NavLink
            to="/monitoring"
            onClick={() => handleNavClick("/monitoring")}
            className={({ isActive }) =>
              cn(
                "lg:hidden relative flex items-center justify-center w-11 h-11 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all duration-300 group hover:bg-[rgb(var(--accent))]/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))]",
                (isActive || navigatingTo === "/monitoring") && "text-[rgb(var(--accent))] bg-transparent"
              )
            }
            aria-label={LAYOUT_COPY.nav.engineMonitor}
          >
            {({ isActive }) => {
              const isNavigatingThis = navigatingTo === "/monitoring";

              return (
                <>
                  <Activity
                    size={24}
                    strokeWidth={isActive || isNavigatingThis ? 2 : 1.5}
                    className={cn(
                      "transition-transform duration-500",
                      !isActive && !isNavigatingThis && "group-hover:scale-110",
                      isNavigatingThis && "scale-95 animate-pulse"
                    )}
                  />

                  {/* Miniature Orbital Navigation Spinner on click */}
                  {isNavigatingThis && (
                    <>
                      <span className="absolute -inset-1 rounded-full border border-dashed border-[rgb(var(--accent))] animate-spin pointer-events-none opacity-80" />
                      <span className="absolute -inset-0.5 rounded-full border border-[rgb(var(--accent))]/40 animate-pulse pointer-events-none" />
                      <span className="absolute -top-1 left-1/2 -translate-x-1/2 w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgb(var(--accent))] pointer-events-none" />
                    </>
                  )}

                  {/* Active Indicator dot */}
                  {isActive && !isNavigatingThis && (
                    <div className="absolute -bottom-1 w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                  )}
                </>
              );
            }}
          </NavLink>
        </Tooltip>
      </nav>
    </>
  );
};
