import React, { useRef, useState, useEffect, useCallback, useMemo, lazy, Suspense } from "react";
import { EdgeNav } from "./EdgeNav";
import { subscribeViewportFrame, subscribeViewportGate } from "./viewportResize";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { TitleBar } from "./TitleBar";
import { AppHeader } from "./AppHeader";
import { AmbientBackground, HelpPanel, NotificationPanel, ErrorBoundary, OrbitalLoader, DelayedFallback } from "@/shared/components/common";
import { TurnMetricsBadge } from "@/shared/components/home";
import { EdgePanel, BottomDockFeather } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { useLocation, useNavigate, useOutlet } from "react-router-dom";
import { motion } from "framer-motion";
import { Activity } from "lucide-react";
import { useVoxFootprint } from "@/shared/hooks/useVoxFootprint";
import { cn } from "@/shared/lib/utils";
import { ModelStatusOverlay } from "@/shared/components/settings/ModelStatusOverlay";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { usePageDrawer } from "@/shared/context/PageDrawerContext";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { useSessionStore } from "@/store/sessionStore";
import { getStackSize } from "@/shared/lib/overlayStack";
import { isCompactWidth } from "./breakpoints";
import { isCoarsePointer } from "@/lib/capabilities";
import { isRealMobileDevice } from "@/shared/hooks/useMobileViewport";

const Monitoring = lazy(() => import("@/pages/Monitoring").then((m) => ({ default: m.Monitoring })));

// Preload the monitoring chunk on the first idle frame so the popover
// opens without any JS-module loading lag on first click.
if (typeof window !== "undefined") {
  const preloadMonitoring = () => import("@/pages/Monitoring");
  const idleWindow = window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
  };
  if (typeof idleWindow.requestIdleCallback === "function") {
    idleWindow.requestIdleCallback(preloadMonitoring, { timeout: 2000 });
  } else {
    setTimeout(preloadMonitoring, 1500);
  }
}

interface ResponsiveLayoutProps {
  children?: React.ReactNode;
}

const PAGE_TITLES: Record<string, string> = {
  "/": "Vox",
  "/history": "Vox — History",
  "/memory": "Vox — Memory",
  "/settings": "Vox — Settings",
  "/monitoring": "Vox — Monitoring",
};

export const ResponsiveLayout: React.FC<ResponsiveLayoutProps> = ({ children }) => {
  const location = useLocation();
  const navigate = useNavigate();
  const outlet = useOutlet();
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
  const isRightPanelOpen = isHelpOpen || isPanelOpen("notifications");


  // Viewport-resize gate: while a resize is in flight, CSS transitions /
  // animations / backdrop-filter are suspended app-wide and the ambient
  // field parks (see viewportResize.ts + index.css).
  const { isResizing } = useViewportResize();
  const isMobile = isRealMobileDevice();

  // Mobile horizontal swipe navigation across primary routes
  const touchStartX = useRef<number | null>(null);
  const touchStartY = useRef<number | null>(null);
  const touchStartTime = useRef<number>(0);
  const [swipeDirection, setSwipeDirection] = useState<number>(0);
  const PRIMARY_ROUTES = useMemo(
    () => (isMobile ? ["/", "/history", "/memory", "/settings"] : ["/", "/history", "/memory", "/settings", "/monitoring"]),
    [isMobile]
  );

  const handleTouchStart = (e: React.TouchEvent) => {
    if (!isMobile) return;
    const target = e.target as HTMLElement | null;
    if (target?.tagName === "CANVAS" || target?.closest("canvas, input[type='range'], [data-no-swipe], [role='slider']")) {
      return;
    }
    touchStartX.current = e.touches[0].clientX;
    touchStartY.current = e.touches[0].clientY;
    touchStartTime.current = Date.now();
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    if (!isMobile || touchStartX.current === null || touchStartY.current === null) return;
    const deltaX = e.changedTouches[0].clientX - touchStartX.current;
    const deltaY = e.changedTouches[0].clientY - touchStartY.current;
    const elapsed = Math.max(1, Date.now() - touchStartTime.current);
    const velocityX = Math.abs(deltaX) / elapsed;

    touchStartX.current = null;
    touchStartY.current = null;

    // Detect flick velocity or meaningful distance with horizontal angle dominance
    const isHorizontalSwipe = (Math.abs(deltaX) > 45 || velocityX > 0.35) && Math.abs(deltaX) > Math.abs(deltaY) * 1.3;
    if (isHorizontalSwipe) {
      const currentIdx = PRIMARY_ROUTES.indexOf(location.pathname);
      if (currentIdx !== -1) {
        if (deltaX < 0 && currentIdx < PRIMARY_ROUTES.length - 1) {
          setSwipeDirection(1);
          navigate(PRIMARY_ROUTES[currentIdx + 1]);
        } else if (deltaX > 0 && currentIdx > 0) {
          setSwipeDirection(-1);
          navigate(PRIMARY_ROUTES[currentIdx - 1]);
        }
      }
    }
  };


  // Ref to track compact state across renders during window resize
  const wasCompactRef = useRef(isCompactWidth());
  const pathnameRef = useRef(location.pathname);
  const monitorOpenRef = useRef(monitorOpen);

  // Sync refs via effects — never write to refs in the render body (concurrent-mode safe)
  useEffect(() => { pathnameRef.current = location.pathname; }, [location.pathname]);
  useEffect(() => { monitorOpenRef.current = monitorOpen; }, [monitorOpen]);

  // Route side effects: window title + scroll reset on navigation.
  // Previously neither existed — every route shared one static title.
  useEffect(() => {
    document.title = PAGE_TITLES[location.pathname] ?? "Vox";
    window.scrollTo(0, 0);
    import("@/services/historyService").then(({ getTurnsCacheSize }) => {
      // (cyan-flash RCA: theme-token state at every route commit. Remove once found.)
      try {
        const r = document.documentElement;
        const cs = getComputedStyle(r);
        console.info(`[route-nav] -> ${location.pathname} turnsCache=${getTurnsCacheSize()} docTheme=${r.getAttribute('data-theme') || 'none'} cssAccent=[${cs.getPropertyValue('--accent').trim() || 'EMPTY'}] inlineAccent=[${r.style.getPropertyValue('--accent').trim() || 'EMPTY'}]`);
      } catch {
        console.info(`[route-nav] -> ${location.pathname} turnsCache=${getTurnsCacheSize()}`);
      }
    }).catch(() => {});
  }, [location.pathname]);

  // Bidirectional viewport transition: compact (EdgeNav route) ↔ full-max (corner popover).
  // A mode *cross* during a resize only records intent; the navigate commits
  // after the resize gate settles, so dragging across the compact boundary
  // never remounts the page mid-gesture (which would recompile WebGL shaders
  // and re-arm every boot gate). The commit raises the mode-flip loader,
  // which clears after the new route's first paint (see below).
  const pendingFlipRef = useRef<{ toCompact: boolean } | null>(null);
  useEffect(() => {
    const unsubFrame = subscribeViewportFrame(() => {
      const isCompact = isCompactWidth();
      if (wasCompactRef.current !== isCompact) {
        // Latest cross wins; an A→B→A wiggle before settle commits nothing.
        pendingFlipRef.current = { toCompact: isCompact };
      }
      wasCompactRef.current = isCompact;
    });

    const unsubGate = subscribeViewportGate((active) => {
      if (active) return;
      const pending = pendingFlipRef.current;
      pendingFlipRef.current = null;
      if (!pending) return;
      if (!pending.toCompact) {
        // Compact → Full-max: switch from route page to popover
        if (pathnameRef.current === "/monitoring") {
          navigate("/", { replace: true });
          setMonitorOpen(true);
        }
      } else {
        // Full-max → Compact: switch from popover to route page
        if (monitorOpenRef.current && pathnameRef.current !== "/monitoring") {
          setMonitorOpen(false);
          navigate("/monitoring");
        }
      }
    });

    return () => {
      unsubFrame();
      unsubGate();
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

      // Application Lifecycle (Ctrl+W close, Ctrl+Q quit) — desktop window
      // chrome only; inert on touch devices with no window controls.
      if (!isCoarsePointer() && mod && (key === "w" || key === "W")) {
        e.preventDefault();
        import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          getCurrentWindow().close();
        }).catch(() => {});
        return;
      }
      if (!isCoarsePointer() && mod && (key === "q" || key === "Q")) {
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
        const isCompact = isCompactWidth();
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
  const isHistory = location.pathname === "/history";
  const isMemory = location.pathname === "/memory";
  const isMonitoring = location.pathname === "/monitoring";

  // Ambient background logic: slow on home, slower on history/settings, disabled on memory/monitoring.
  // On mobile (Android), ambient field is isolated exclusively to Home (/) to conserve GPU/battery.
  const ambientDisabled = isMemory || isMonitoring || (isMobile && !isHome);
  const rippleSpeedMultiplier = isSettings || isHistory ? 2.2 : 1.4;
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
        {/* Ambient Background — visible on Home, History, Settings; disabled on Memory & Monitoring */}
        {!ambientDisabled && (
          <AmbientBackground
            instanceId="layout"
            originY={ambientOriginY}
            paused={interactionState === "Speaking" || isResizing}
            rippleSpeedMultiplier={rippleSpeedMultiplier}
            rippleShape={
              isHistory && historyDisplayMode === "orbit"
                ? "orbit"
                : "circle"
            }
          />
        )}

        {/* Page content */}
        <main
          onTouchStart={handleTouchStart}
          onTouchEnd={handleTouchEnd}
          style={{
            position: "relative",
            flex: 1,
            height: "100%",
            overflow: "hidden",
            width: "100%",
            display: "flex",
            flexDirection: "column",
            paddingBottom: isMobile ? "calc(60px + env(safe-area-inset-bottom))" : undefined,
          }}
        >
          {/* Unified App Header for all layouts */}
          <AppHeader />

          <Suspense
            fallback={
              <DelayedFallback delayMs={150}>
                <div className="flex-1 w-full h-full flex flex-col items-center justify-center p-6 animate-in fade-in duration-200">
                  <OrbitalLoader size="md" />
                </div>
              </DelayedFallback>
            }
          >
            {(children || outlet) && (
              <motion.div
                key={location.pathname}
                initial={{
                  opacity: isMobile && swipeDirection !== 0 ? 0.8 : 0,
                  x: isMobile && swipeDirection !== 0 ? (swipeDirection > 0 ? "18%" : "-18%") : 0,
                }}
                animate={{ opacity: 1, x: 0 }}
                transition={{
                  duration: isMobile ? 0.22 : 0.15,
                  ease: [0.16, 1, 0.3, 1],
                }}
                className="flex-1 w-full min-h-0 overflow-hidden flex flex-col transform-gpu"
              >
                {children || outlet}
              </motion.div>
            )}
          </Suspense>
        </main>

        {/* ── Engine Monitor Area — bottom-left ───────────────────────────── */}
        <div className="hidden lg:flex fixed bottom-[calc(1rem+env(safe-area-inset-bottom))] left-[max(1rem,env(safe-area-inset-left))] z-40 items-center gap-2.5 pointer-events-none">
          {sessionsOpen && (
            <BottomDockFeather className="absolute -left-4 -bottom-4 -top-12 w-[340px] pointer-events-none" />
          )}
          <div className="relative pointer-events-auto flex items-center gap-2.5">
            {/* Monitor toggle button */}
            <Tooltip label={LAYOUT_COPY.nav.engineMonitor} shortcutId="global.monitor" side="right">
            <button
              ref={monitorBtnRef}
              data-flip-trace="monitor-btn"
              onClick={() => setMonitorOpen((v) => !v)}
              className={cn(
        "relative flex items-center justify-center w-11 h-11 rounded-full border transition-all hover:scale-105 cursor-pointer glass-card glass-keep-blur",
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
            </Tooltip>

            {/* Mini footprint HUD — CPU% · RAM MB (Click to launch Memory Profiler) */}
            {isReady && (
              <Tooltip label={LAYOUT_COPY.nav.openProfiler} shortcutId="home.profiler">
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
        {isHome && interactionState !== "Idle" ? (
          /* ── Turn Performance & Context Utilization Metrics — active session (Home only) ── */
          <div className="hidden lg:flex fixed bottom-4 right-4 z-40 pointer-events-none items-center">
            {isRightPanelOpen && (
              <BottomDockFeather className="absolute -right-4 -bottom-4 -top-12 w-[340px] pointer-events-none" />
            )}
            <div className="relative pointer-events-auto">
              <TurnMetricsBadge />
            </div>
          </div>
        ) : isHome || isSettings ? (
          /* ── Home & Settings View (Idle): Model Status Overlay in same consistent corner ── */
          <div className="hidden lg:flex fixed bottom-4 right-4 z-40 pointer-events-none items-center">
            {isRightPanelOpen && (
              <BottomDockFeather className="absolute -right-4 -bottom-4 -top-12 w-[340px] pointer-events-none" />
            )}
            <div className="relative pointer-events-auto flex items-center px-3 lg:px-4 py-2.5 bg-transparent border-transparent shadow-none">
              <ModelStatusOverlay />
            </div>
          </div>
        ) : null}

        {/* Top-level Stage Slot for edge panels (eliminates 56px page drop) */}
        <div id="stage-edge-panels" className="absolute inset-0 pointer-events-none z-35" />

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
