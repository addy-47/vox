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
import { usePageDrawer } from "@/shared/context/PageDrawerContext";
import { SESSION_COPY } from "@/data/sessionCopy";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { useSessionStore } from "@/store/sessionStore";
import { getStackSize } from "@/shared/lib/overlayStack";

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
  const { openActiveDrawer, closeActiveDrawer } = usePageDrawer();
  const { isPanelOpen, closePanel, openPanel, togglePanel } = usePanelStateContext();
  const historyDisplayMode = useHistoryFilterStore((s) => s.displayMode);
  const interactionState = useSessionStore((s) => s.interactionState);

  const [helpInitialShortcuts, setHelpInitialShortcuts] = useState(false);

  const closeHelp = useCallback(() => {
    setHelpInitialShortcuts(false);
    closePanel("help");
  }, [closePanel]);
  const closeNotifications = useCallback(() => closePanel("notifications"), [closePanel]);
  const isHelpOpen = isPanelOpen("help");
  const isHelpOpenRef = useRef(isHelpOpen);
  useEffect(() => {
    isHelpOpenRef.current = isHelpOpen;
  }, [isHelpOpen]);

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

  // ── Arrow-key handler: page-nav vs in-page movement vs spatial (v2) ──
  // Precedence (checked in order):
  //   1. If focus is in input/textarea/select/contenteditable → caret keys win (skip)
  //   2. If focus is inside [data-arrow-nav] → widget handles (stopPropagation)
  //   3. If focus is in EdgeNav (nav[data-edge-nav]) or on body/nothing-focusable → page-nav
  //   4. Otherwise → spatial move to nearest focusable in that direction
  useEffect(() => {
    const isEditable = (el: Element | null): boolean => {
      if (!el) return false;
      const tag = el.tagName.toLowerCase();
      return tag === "input" || tag === "textarea" || tag === "select" || el.getAttribute("contenteditable") === "true";
    };

    const isInArrowGroup = (el: Element | null): boolean => {
      return !!el && !!el.closest("[data-arrow-nav]");
    };

    const focusableSelector = 'button:not([disabled]),[tabIndex="0"]:not([tabindex="-1"]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[href],[contenteditable]';

    const spatialMove = (direction: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight") => {
      const all = Array.from(document.querySelectorAll<HTMLElement>(focusableSelector));
      const active = document.activeElement as HTMLElement | null;

      const candidates = all.filter((el) => {
        if (el === active) return false;
        if (el.getAttribute("tabindex") === "-1") return false;
        if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
        if (el.closest("[aria-hidden='true']")) return false;
        if (el.closest(".pointer-events-none") && !el.closest(".pointer-events-auto")) return false;
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0;
      });

      if (candidates.length === 0) return;

      const ar = active?.getBoundingClientRect();
      if (!ar || active === document.body) {
        candidates[0]?.focus();
        return;
      }

      const activeCx = ar.left + ar.width / 2;
      const activeCy = ar.top + ar.height / 2;

      let best: HTMLElement | null = null;
      let bestScore = Infinity;

      for (const el of candidates) {
        const r = el.getBoundingClientRect();
        const candCx = r.left + r.width / 2;
        const candCy = r.top + r.height / 2;

        let primary = Infinity;
        let secondary = Infinity;
        let overlap = 0;

        if (direction === "ArrowRight") {
          if (r.left >= ar.left + 4 || candCx > activeCx + 4) {
            primary = Math.max(0, r.left - ar.right);
            secondary = Math.abs(candCy - activeCy);
            overlap = Math.max(0, Math.min(ar.bottom, r.bottom) - Math.max(ar.top, r.top));
          }
        } else if (direction === "ArrowLeft") {
          if (r.right <= ar.right - 4 || candCx < activeCx - 4) {
            primary = Math.max(0, ar.left - r.right);
            secondary = Math.abs(candCy - activeCy);
            overlap = Math.max(0, Math.min(ar.bottom, r.bottom) - Math.max(ar.top, r.top));
          }
        } else if (direction === "ArrowDown") {
          if (r.top >= ar.top + 4 || candCy > activeCy + 4) {
            primary = Math.max(0, r.top - ar.bottom);
            secondary = Math.abs(candCx - activeCx);
            overlap = Math.max(0, Math.min(ar.right, r.right) - Math.max(ar.left, r.left));
          }
        } else if (direction === "ArrowUp") {
          if (r.bottom <= ar.bottom - 4 || candCy < activeCy - 4) {
            primary = Math.max(0, ar.top - r.bottom);
            secondary = Math.abs(candCx - activeCx);
            overlap = Math.max(0, Math.min(ar.right, r.right) - Math.max(ar.left, r.left));
          }
        }

        if (primary < Infinity) {
          const overlapBonus = overlap > 0 ? 0.4 : 1.0;
          const score = (primary + 1) + (secondary * 2.2 * overlapBonus);
          if (score < bestScore) {
            bestScore = score;
            best = el;
          }
        }
      }

      best?.focus();
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;

      // 1. Editables → caret keys win
      if (isEditable(activeEl)) return;

      // ---- Globals (outside editables, always) ----
      if (e.repeat) return;
      const key = e.key;
      const mod = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;

      // Application Lifecycle (Ctrl+W close, Ctrl+Q quit)
      if (mod && (key === "w" || key === "W")) {
        e.preventDefault();
        import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          getCurrentWindow().close();
        }).catch(() => {});
        return;
      }
      if (mod && (key === "q" || key === "Q")) {
        e.preventDefault();
        import("@tauri-apps/plugin-process").then(({ exit }) => {
          exit(0);
        }).catch(() => {});
        return;
      }

      // Rails & Popovers
      if (mod && (key === "m" || key === "M")) { e.preventDefault(); setMonitorOpen((v) => !v); return; }
      if (mod && (key === "s" || key === "S")) { e.preventDefault(); togglePanel("sessions"); return; }
      if (mod && (key === "n" || key === "N")) { e.preventDefault(); togglePanel("notifications"); return; }
      
      // Ctrl + / -> Toggle Help & Guide
      if (mod && (key === "/" || e.code === "Slash")) {
        e.preventDefault();
        if (isHelpOpenRef.current) {
          closeHelp();
        } else {
          setHelpInitialShortcuts(false);
          openPanel("help");
        }
        return;
      }
      // ? (or Shift + /) -> Toggle Keyboard Shortcuts sheet
      if (!mod && (key === "?" || (shift && (key === "/" || e.code === "Slash")))) {
        e.preventDefault();
        if (isHelpOpenRef.current) {
          closeHelp();
        } else {
          setHelpInitialShortcuts(true);
          openPanel("help");
        }
        return;
      }

      // Page Navigation: Shift + Left / Right (always wins outside editables)
      if (shift && (key === "ArrowRight" || key === "ArrowLeft")) {
        e.preventDefault();
        const isCompact = window.innerWidth < 1024;
        const routes = isCompact ? ["/", "/history", "/memory", "/settings", "/monitoring"] : ["/", "/history", "/memory", "/settings"];
        const currentIndex = routes.indexOf(location.pathname);
        if (currentIndex !== -1) {
          setMonitorOpen(false);
          if (key === "ArrowRight") {
            navigate(routes[(currentIndex + 1) % routes.length]);
          } else {
            navigate(routes[(currentIndex - 1 + routes.length) % routes.length]);
          }
        }
        return;
      }

      // Context Drawers: Shift + Up / Down (contextual by route)
      if (shift && key === "ArrowUp") {
        e.preventDefault();
        if (getStackSize() === 0) {
          openActiveDrawer();
        }
        return;
      }
      if (shift && key === "ArrowDown") {
        e.preventDefault();
        closeActiveDrawer();
        return;
      }

      // ---- Plain Directional Arrows: Spatial Navigation inside active zone ----
      if (key !== "ArrowRight" && key !== "ArrowLeft" && key !== "ArrowUp" && key !== "ArrowDown") return;

      // 2. Widget group with roving tabindex owns it
      if (isInArrowGroup(activeEl)) return;

      // 3. Zone-bounded spatial move
      spatialMove(key as "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown");
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [location.pathname, navigate, togglePanel, openPanel, openActiveDrawer, closeActiveDrawer]);

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
      <div data-spatial-zone="stage" style={{ flex: 1, display: "flex", overflow: "hidden", position: "relative", minHeight: 0 }}>
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
            <HelpPanel onClose={closeHelp} initialShortcuts={helpInitialShortcuts} />
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
