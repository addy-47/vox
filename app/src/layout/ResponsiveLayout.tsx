import React, { useRef, useState, useEffect, useCallback, lazy, Suspense } from "react";
import { EdgeNav } from "./EdgeNav";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { TitleBar } from "./TitleBar";
import { AmbientBackground, HelpPanel, NotificationPanel, ErrorBoundary } from "@/shared/components/common";
import { ActiveSessionHeader } from "@/shared/components/home";
import { EdgePanel, TopRightCluster, BottomDockFeather } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { Activity, PanelLeft } from "lucide-react";
import { useVoxFootprint } from "@/shared/hooks/useVoxFootprint";
import { cn } from "@/shared/lib/utils";
import { ModelStatusOverlay } from "@/shared/components/settings/ModelStatusOverlay";
import { RestoreDefaultsButton } from "@/shared/components/settings/RestoreDefaultsButton";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { SESSION_COPY } from "@/data/sessionCopy";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { useSessionStore } from "@/store/sessionStore";

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
  const { isPanelOpen, closePanel, togglePanel } = usePanelStateContext();
  const historyDisplayMode = useHistoryFilterStore((s) => s.displayMode);
  const interactionState = useSessionStore((s) => s.interactionState);

  const closeHelp = useCallback(() => closePanel("help"), [closePanel]);
  const closeNotifications = useCallback(() => closePanel("notifications"), [closePanel]);

  const sessionsOpen = isPanelOpen("sessions");

  // Ref to track compact state across renders during window resize
  const wasCompactRef = useRef(window.innerWidth < 1024);
  const pathnameRef = useRef(location.pathname);
  const monitorOpenRef = useRef(monitorOpen);

  // Sync refs via effects — never write to refs in the render body (concurrent-mode safe)
  useEffect(() => { pathnameRef.current = location.pathname; }, [location.pathname]);
  useEffect(() => { monitorOpenRef.current = monitorOpen; }, [monitorOpen]);

  // Bidirectional viewport transition: compact (EdgeNav route) ↔ full-max (corner popover)
  useEffect(() => {
    let rAfId: number | null = null;
    const handleResize = () => {
      if (rAfId !== null) return;
      rAfId = requestAnimationFrame(() => {
        rAfId = null;
        const isCompact = window.innerWidth < 1024;
        if (wasCompactRef.current && !isCompact) {
          // Compact → Full-max: switch from route page to popover
          if (pathnameRef.current === "/monitoring") {
            navigate("/", { replace: true });
            setMonitorOpen(true);
          }
        } else if (!wasCompactRef.current && isCompact) {
          // Full-max → Compact: switch from popover to route page
          if (monitorOpenRef.current) {
            setMonitorOpen(false);
            navigate("/monitoring");
          }
        }
        wasCompactRef.current = isCompact;
      });
    };

    window.addEventListener("resize", handleResize, { passive: true });
    return () => {
      window.removeEventListener("resize", handleResize);
      if (rAfId !== null) cancelAnimationFrame(rAfId);
    };
  }, [navigate]);

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
  const isHome = location.pathname === "/";
  const isMonitoring = location.pathname === "/monitoring";
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
          paused={interactionState === "Speaking"}
          rippleShape={
            location.pathname === "/history" && historyDisplayMode === "orbit"
              ? "orbit"
              : "circle"
          }
        />

        {/* Page content */}
        <main
          style={{
            position: "relative",
            flex: 1,
            height: "100%",
            overflow: "hidden",
            width: "100%",
          }}
        >
          <div className="h-full w-full overflow-hidden flex flex-col">
            {children || <Outlet />}
          </div>
        </main>

        {/* ── Session toggle (top-left) — Home only; z-[60] so always above EdgePanel z-[35] ── */}
        {isHome && (
          <div className="absolute top-4 left-5 z-[60] pointer-events-none flex items-center gap-2.5">
            <Tooltip label={SESSION_COPY.railTitle} side="bottom">
              <button
                onClick={() => togglePanel("sessions")}
                aria-label={SESSION_COPY.openRailAriaLabel}
                aria-expanded={sessionsOpen}
                data-edge-trigger="left"
                className={cn(
                  "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all cursor-pointer pointer-events-auto shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                  sessionsOpen
                    ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)]"
                    : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
                )}
              >
                <PanelLeft size={14} strokeWidth={1.75} />
              </button>
            </Tooltip>

            {/* Active Session & Project Header Breadcrumb (shown on Home when panel is closed) */}
            <ActiveSessionHeader
              panelOpen={sessionsOpen}
              isHome={true}
              onOpenPanel={() => togglePanel("sessions")}
            />
          </div>
        )}

        {/* ── Help + Notifications cluster (top-right) — hidden on /monitoring; z-[60] so always above EdgePanel z-[35] ── */}
        {!isMonitoring && (
          <div className="absolute top-0 right-0 z-[60] pointer-events-none">
            {/* Feathering haze: dissolves panel content around trigger buttons with zero GPU blur overhead */}
            <div
              aria-hidden="true"
              className="absolute top-0 right-0 w-44 h-28 pointer-events-none"
              style={{
                background:
                  "radial-gradient(ellipse 100% 90% at 100% 0%, rgb(var(--card)) 20%, rgba(var(--card), 0.75) 50%, transparent 80%)",
              }}
            />
            {/* Buttons sit above the haze */}
            <div className="relative pt-4 pr-5 pointer-events-auto">
              <TopRightCluster />
            </div>
          </div>
        )}


        {/* ── Engine Monitor Area — bottom-left ───────────────────────────── */}
        <div className="hidden lg:flex fixed bottom-4 left-4 z-40 items-center gap-2.5 pointer-events-none">
          {/* Standard bottom-dock feather: dissolves scrolled content above the monitor button */}
          <BottomDockFeather className="absolute -inset-x-6 bottom-[calc(100%-2px)] h-10" />
          {/* relative: keeps the positioned feather painted underneath the controls */}
          <div className="relative pointer-events-auto flex items-center gap-2.5">
            {/* Monitor toggle button */}
            <button
              ref={monitorBtnRef}
              onClick={() => setMonitorOpen((v) => !v)}
              className={cn(
                "relative flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 hover:scale-105 cursor-pointer glass-card",
                monitorOpen
                  ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/60"
                  : "bg-transparent border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
              )}
              aria-label={LAYOUT_COPY.nav.engineMonitor}
              aria-expanded={monitorOpen}
              aria-haspopup="dialog"
            >
              <Activity size={24} strokeWidth={2} />
            </button>

            {/* Mini footprint HUD — CPU% · RAM MB (Click to launch Memory Profiler) */}
            {isReady && (
              <Tooltip label={LAYOUT_COPY.nav.openProfiler}>
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
        </div>

        {/* Monitoring Popover */}
        <ErrorBoundary name="MonitoringPopover">
          <Suspense fallback={null}>
            <Monitoring
              popover
              open={monitorOpen}
              onClose={() => setMonitorOpen(false)}
              anchorRef={monitorBtnRef}
            />
          </Suspense>
        </ErrorBoundary>

        {/* ── Status Info & Default Reset Controls Area — bottom-right ── */}
        {isSettings && (
          <div className="hidden lg:flex fixed bottom-4 right-4 z-40 pointer-events-none items-center gap-2 lg:gap-3 max-w-[calc(50vw-180px)]">
            {/* Standard bottom-dock feather: dissolves scrolled content behind the dock */}
            <BottomDockFeather className="absolute -inset-x-8 -bottom-4 -top-10" />

            <div className="relative pointer-events-auto flex items-center gap-2 lg:gap-3 px-3 lg:px-4 py-2.5 bg-transparent border-transparent shadow-none">
              <ModelStatusOverlay />
              <div className="w-px h-4 bg-[rgba(var(--accent),0.15)] shrink-0" />
              <RestoreDefaultsButton />
            </div>
          </div>
        )}

        {/* Bottom navigation (topmost in bottom layer) */}
        <EdgeNav />

        {/* ── Right Edge Rails (Help & Notifications) ── */}
        <EdgePanel
          side="right"
          open={isPanelOpen("help")}
          onClose={closeHelp}
          minimalHeader
        >
          <ErrorBoundary name="HelpPanel">
            <HelpPanel onClose={closeHelp} />
          </ErrorBoundary>
        </EdgePanel>
        <EdgePanel
          side="right"
          open={isPanelOpen("notifications")}
          onClose={closeNotifications}
          minimalHeader
        >
          <ErrorBoundary name="NotificationPanel">
            <NotificationPanel onClose={closeNotifications} />
          </ErrorBoundary>
        </EdgePanel>
      </div>
    </div>
  );
};
