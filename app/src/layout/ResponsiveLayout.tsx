import React, { useRef, useState, useEffect, lazy, Suspense } from "react";
import { EdgeNav } from "./EdgeNav";
import { TitleBar } from "./TitleBar";
import { AmbientBackground } from "@/shared/components/common";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Activity } from "lucide-react";
import { useVoxFootprint } from "@/shared/hooks/useVoxFootprint";
import { cn } from "@/shared/lib/utils";
import { ModelStatusOverlay } from "@/shared/components/settings/ModelStatusOverlay";
import { RestoreDefaultsButton } from "@/shared/components/settings/RestoreDefaultsButton";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";

const Monitoring = lazy(() => import("@/pages/Monitoring").then((m) => ({ default: m.Monitoring })));

interface ResponsiveLayoutProps {
  children?: React.ReactNode;
}

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  const location = useLocation();
  const navigate = useNavigate();
  const [monitorOpen, setMonitorOpen] = useState(false);
  const monitorBtnRef = useRef<HTMLButtonElement>(null);
  const { voxCpu, voxRam, isReady } = useVoxFootprint();
  const { openProfiler } = useProfilerDrawer();

  // Ref to track compact state across renders during window resize
  const wasCompactRef = useRef(window.innerWidth < 1024);

  // Bidirectional viewport transition: compact (EdgeNav route) ↔ full-max (corner popover)
  useEffect(() => {
    const handleResize = () => {
      const isCompact = window.innerWidth < 1024;
      if (wasCompactRef.current && !isCompact) {
        // Compact → Full-max: switch from route page to popover
        if (location.pathname === "/monitoring") {
          navigate("/", { replace: true });
          setMonitorOpen(true);
        }
      } else if (!wasCompactRef.current && isCompact) {
        // Full-max → Compact: switch from popover to route page
        if (monitorOpen) {
          setMonitorOpen(false);
          navigate("/monitoring");
        }
      }
      wasCompactRef.current = isCompact;
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [location.pathname, monitorOpen, navigate]);

  // ── Arrow Keys Page Navigation ─────────────────────────────────────────────
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Check if focus is in an input/textarea/select/editable
      const activeEl = document.activeElement;
      if (activeEl) {
        const tagName = activeEl.tagName.toLowerCase();
        if (
          tagName === "input" ||
          tagName === "textarea" ||
          tagName === "select" ||
          activeEl.getAttribute("contenteditable") === "true"
        ) {
          return;
        }
      }

      const isCompact = window.innerWidth < 1024;
      // Match visual sequence in EdgeNav: Home (/) -> History (/history) -> Memory (/memory) -> System (/settings)
      const routes = isCompact
        ? ["/", "/history", "/memory", "/settings", "/monitoring"]
        : ["/", "/history", "/memory", "/settings"];

      const currentIndex = routes.indexOf(location.pathname);
      if (currentIndex === -1) return;

      if (e.key === "ArrowRight") {
        e.preventDefault();
        setMonitorOpen(false);
        const nextIndex = (currentIndex + 1) % routes.length;
        navigate(routes[nextIndex]);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        setMonitorOpen(false);
        const nextIndex = (currentIndex - 1 + routes.length) % routes.length;
        navigate(routes[nextIndex]);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [location.pathname, navigate]);

  const isSettings = location.pathname === "/settings";
  // Ambient origin — standardized across all views (Home, History, Settings, Memory) to calc(50% - 36px)
  const ambientOriginY = "calc(50% - 36px)";

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        width: "100%",
        overflow: "hidden",
      }}
      className="bg-[rgb(var(--background))] text-[rgb(var(--foreground))]"
    >
      <TitleBar />

      {/* ── Content Area ──────────────────────────────────────────────────── */}
      <div style={{ flex: 1, display: "flex", overflow: "hidden", position: "relative", minHeight: 0 }}>
        {/* Resize Handles (Invisible, for cursor hit-testing on Linux) */}
        <div className="absolute top-0 left-0 w-full h-[3px] cursor-ns-resize z-[100]" />
        <div className="absolute bottom-0 left-0 w-full h-[3px] cursor-ns-resize z-[100]" />
        <div className="absolute top-0 left-0 h-full w-[3px] cursor-ew-resize z-[100]" />
        <div className="absolute top-0 right-0 h-full w-[3px] cursor-ew-resize z-[100]" />

        {/* Corner Handles */}
        <div className="absolute top-0 left-0 w-2 h-2 cursor-nwse-resize z-[110]" />
        <div className="absolute top-0 right-0 w-2 h-2 cursor-nesw-resize z-[110]" />
        <div className="absolute bottom-0 left-0 w-2 h-2 cursor-nesw-resize z-[110]" />
        <div className="absolute bottom-0 right-0 w-2 h-2 cursor-nwse-resize z-[110]" />

        {/* Ambient Background — visible on every page */}
        <AmbientBackground
          originY={ambientOriginY}
          rippleShape={location.pathname === "/history" ? "orbit" : "circle"}
        />

        {/* Bottom navigation */}
        <EdgeNav />

        {/* ── Engine Monitor Area — bottom-left ───────────────────────────── */}
        <div className="hidden lg:flex fixed bottom-4 left-4 z-50 items-center gap-2.5">
          {/* Monitor toggle button */}
          <button
            ref={monitorBtnRef}
            onClick={() => setMonitorOpen((v) => !v)}
            className={cn(
              "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 hover:scale-105 cursor-pointer glass-card",
              monitorOpen
                ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/60"
                : "bg-transparent border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
            )}
            aria-label="Engine Monitor"
            aria-expanded={monitorOpen}
            aria-haspopup="dialog"
          >
            <Activity size={24} strokeWidth={2} />
          </button>

          {/* Mini footprint HUD — CPU% · RAM MB (Click to launch Memory Profiler) */}
          {isReady && (
            <Tooltip label="Open UI Memory Profiler">
              <button
                onClick={() => {
                  setMonitorOpen(false);
                  openProfiler();
                }}
                className="text-[14px] font-mono text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] px-2 py-1 rounded-lg transition-all leading-none select-none tabular-nums cursor-pointer border border-transparent hover:border-[rgba(var(--border),0.15)]"
              >
                {voxCpu.toFixed(1)}% · {Math.round(voxRam)} MB
              </button>
            </Tooltip>
          )}
        </div>

        {/* Monitoring Popover */}
        <Suspense fallback={null}>
          <Monitoring
            popover
            open={monitorOpen}
            onClose={() => setMonitorOpen(false)}
            anchorRef={monitorBtnRef}
          />
        </Suspense>

        {/* ── Status Info & Default Reset Controls Area — bottom-right ── */}
        {isSettings && (
          <div className="hidden lg:flex fixed bottom-4 right-4 z-50 items-center gap-2 lg:gap-3 px-3 lg:px-4 py-2.5 bg-transparent border-transparent shadow-none max-w-[calc(100vw-320px)]">
            <ModelStatusOverlay />
            <div className="w-px h-4 bg-[rgba(var(--accent),0.15)] shrink-0" />
            <RestoreDefaultsButton />
          </div>
        )}

        {/* Page content */}
        <main
          style={{
            position: "relative",
            zIndex: 10,
            flex: 1,
            height: "100%",
            overflow: "hidden",
            width: "100%",
            contain: "layout style",
          }}
        >
          <div className="h-full w-full overflow-hidden flex flex-col">
            {children || <Outlet />}
          </div>
        </main>
      </div>
    </div>
  );
};
