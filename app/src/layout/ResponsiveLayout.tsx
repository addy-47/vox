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
import { usePanelStateContext } from "@/shared/context/PanelStateContext";
import { useLocation, useNavigate, useOutlet, type NavigateOptions } from "react-router-dom";
import { motion } from "framer-motion";
import { Activity } from "lucide-react";
import { useVoxFootprint } from "@/shared/hooks/useRuntimeSnapshot";
import { cn } from "@/shared/lib/utils";
import { ModelStatusOverlay } from "@/shared/components/settings/ModelStatusOverlay";
import { preloadSettingsCardChunks } from "@/shared/components/settings/settingsCardChunks";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { usePageDrawer } from "@/shared/context/PageDrawerContext";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { useSessionStore } from "@/store/sessionStore";
import { useRemoteStore } from "@/store/remoteStore";
import { getStackSize } from "@/shared/lib/overlayStack";
import { isCompactWidth, isRealMobileDevice } from "./breakpoints";
import { stageTop } from "./stageMetrics";
import { isCoarsePointer } from "@/lib/capabilities";
import { RouteNavigationProvider } from "./RouteNavigationContext";

const Monitoring = lazy(() => import("@/pages/Monitoring").then((m) => ({ default: m.Monitoring })));
const ROUTE_REVEAL_MS = 150;

async function prepareRoute(pathname: string): Promise<void> {
  switch (pathname) {
    case "/history":
      await import("@/pages/History");
      return;
    case "/memory":
      await import("@/pages/Memory");
      return;
    case "/settings": {
      await import("@/pages/Settings");
      await preloadSettingsCardChunks();
      return;
    }
    case "/monitoring":
      await import("@/pages/Monitoring");
      return;
    default:
      return;
  }
}

interface PendingNavigation {
  pathname: string;
  phase: "loading" | "revealing";
  onReady?: () => void;
  pageReady: boolean;
  chunksReady: boolean;
  startedAt: number;
}

// Preload the monitoring chunk on the first idle frame so the popover
// opens without any JS-module loading lag on first click.
if (typeof window !== "undefined") {
  const preloadMonitoring = () => import("@/pages/Monitoring");
  const idleWindow = window as Window & {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number;
  };
  if (typeof idleWindow.requestIdleCallback === "function") {
    idleWindow.requestIdleCallback(() => { void preloadMonitoring(); }, { timeout: 2000 });
  } else {
    setTimeout(() => { void preloadMonitoring(); }, 1500);
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
  const [pendingNavigation, setPendingNavigation] = useState<PendingNavigation | null>(null);
  const pendingNavigationRef = useRef<PendingNavigation | null>(null);
  const navigationGenerationRef = useRef(0);
  const routeRevealTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [monitorOpen, setMonitorOpen] = useState(false);
  const monitorBtnRef = useRef<HTMLButtonElement>(null);
  const { voxCpu, voxRam, isReady } = useVoxFootprint();
  const { openProfiler } = useProfilerDrawer();
  const { openActiveDrawer, closeActiveDrawer } = usePageDrawer();
  const { isPanelOpen, closePanel, openPanel, togglePanel } = usePanelStateContext();
  const historyDisplayMode = useHistoryFilterStore((s) => s.displayMode);
  const interactionState = useSessionStore((s) => s.interactionState);
  const remoteLinkState = useRemoteStore((s) => s.linkState);

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

  const cancelPendingNavigation = useCallback(() => {
    navigationGenerationRef.current += 1;
    if (routeRevealTimerRef.current !== null) {
      clearTimeout(routeRevealTimerRef.current);
      routeRevealTimerRef.current = null;
    }
    pendingNavigationRef.current = null;
    setPendingNavigation(null);
  }, []);

  const revealReadyPage = useCallback((pending: PendingNavigation) => {
    if (pendingNavigationRef.current !== pending || !pending.pageReady || !pending.chunksReady) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const showPage = () => {
      if (pendingNavigationRef.current !== pending) return;
      const revealing: PendingNavigation = { ...pending, phase: "revealing" };
      pendingNavigationRef.current = revealing;
      setPendingNavigation(revealing);

      routeRevealTimerRef.current = setTimeout(() => {
        if (pendingNavigationRef.current !== revealing) return;
        pendingNavigationRef.current = null;
        routeRevealTimerRef.current = null;
        setPendingNavigation(null);
        revealing.onReady?.();
      }, reducedMotion ? 0 : ROUTE_REVEAL_MS);
    };

    const minimumLoaderMs = reducedMotion ? 0 : 100;
    const remainingLoaderMs = minimumLoaderMs - (performance.now() - pending.startedAt);
    routeRevealTimerRef.current = setTimeout(showPage, Math.max(0, remainingLoaderMs));
  }, []);

  const navigateWithFeedback = useCallback((to: string, onReady?: () => void, options?: NavigateOptions) => {
    const targetPathname = new URL(to, window.location.href).pathname;
    if (targetPathname === location.pathname) {
      const pending = pendingNavigationRef.current;
      if (pending?.pathname === targetPathname) {
        if (onReady) {
          const previousOnReady = pending.onReady;
          pending.onReady = () => {
            previousOnReady?.();
            onReady();
          };
        }
        return;
      }
      if (pending) cancelPendingNavigation();
      void navigate(to, options);
      if (onReady) requestAnimationFrame(onReady);
      return;
    }

    const generation = ++navigationGenerationRef.current;
    if (routeRevealTimerRef.current !== null) {
      clearTimeout(routeRevealTimerRef.current);
      routeRevealTimerRef.current = null;
    }

    const pending: PendingNavigation = {
      pathname: targetPathname,
      phase: "loading",
      onReady,
      pageReady: false,
      chunksReady: false,
      startedAt: performance.now(),
    };
    pendingNavigationRef.current = pending;
    setPendingNavigation(pending);

    // Commit immediately so the previous page unmounts. The shell-level loader
    // stays opaque while the destination and its render chunks prepare.
    void navigate(to, options);
    void prepareRoute(targetPathname)
      .then(() => {
        if (navigationGenerationRef.current !== generation) return;
        pending.chunksReady = true;
        revealReadyPage(pending);
      })
      .catch((error: unknown) => {
        console.error(`[navigation] Failed to prepare ${targetPathname}:`, error);
        if (navigationGenerationRef.current !== generation) return;
        pendingNavigationRef.current = null;
        setPendingNavigation(null);
      });
  }, [cancelPendingNavigation, location.pathname, navigate, revealReadyPage]);

  const handlePageReady = useCallback((pathname: string) => {
    const pending = pendingNavigationRef.current;
    if (!pending || pending.pathname !== pathname) return;

    pending.pageReady = true;
    revealReadyPage(pending);
  }, [revealReadyPage]);

  const routeNavigation = useMemo(
    () => ({ navigate: navigateWithFeedback, pageReady: handlePageReady }),
    [handlePageReady, navigateWithFeedback]
  );

  useEffect(() => () => {
    if (routeRevealTimerRef.current !== null) clearTimeout(routeRevealTimerRef.current);
  }, []);

  useEffect(() => {
    const pending = pendingNavigationRef.current;
    if (pending && location.pathname !== pending.pathname) {
      cancelPendingNavigation();
    }
  }, [cancelPendingNavigation, location.pathname]);


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
    // A new gesture supersedes any previous swipe direction — otherwise a
    // later tap-nav would replay the stale slide offset from an old swipe.
    if (swipeDirection !== 0) setSwipeDirection(0);
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
          navigateWithFeedback(PRIMARY_ROUTES[currentIdx + 1]);
        } else if (deltaX > 0 && currentIdx > 0) {
          setSwipeDirection(-1);
          navigateWithFeedback(PRIMARY_ROUTES[currentIdx - 1]);
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
    const path = location.pathname;
    document.title = PAGE_TITLES[path] ?? "Vox";

    // Scroll reset is a low-priority UI chore; defer it to the next paint so we
    // do not force a synchronous layout during route commits. This keeps route
    // changes smooth without keeping every page mounted in memory.
    const scrollFrame = window.requestAnimationFrame(() => {
      window.scrollTo({ top: 0, left: 0, behavior: "auto" });
    });

    // Dev-only route-nav tracing is intentionally off by default. The old
    // version imported the persistence service and read computed styles on every
    // navigation, which forces a full style recalc and adds visible jank in the
    // hot path. Keep the probe explicitly opt-in instead of running it blindly.
    const shouldTrace =
      import.meta.env.DEV &&
      (window as typeof window & { __VOX_ROUTE_TRACE__?: boolean }).__VOX_ROUTE_TRACE__ === true;
    if (!shouldTrace) {
      return () => cancelAnimationFrame(scrollFrame);
    }

    void import("@/services/persistenceService")
      .then(({ getTurnsCacheSize }) => {
        try {
          const root = document.documentElement;
          const cs = getComputedStyle(root);
          const docTheme = root.hasAttribute("data-theme") ? root.getAttribute("data-theme") : null;
          const cssAccentRaw = cs.getPropertyValue("--accent").trim();
          const inlineAccentRaw = root.style.getPropertyValue("--accent").trim();
          const docThemeLabel = docTheme ?? "none";
          const cssAccent = cssAccentRaw.length > 0 ? cssAccentRaw : "EMPTY";
          const inlineAccent = inlineAccentRaw.length > 0 ? inlineAccentRaw : "EMPTY";
          console.info(
            `[route-nav] -> ${path} turnsCache=${getTurnsCacheSize()} docTheme=${docThemeLabel} cssAccent=[${cssAccent}] inlineAccent=[${inlineAccent}]`
          );
        } catch (error) {
          console.warn(`[route-nav] Failed to read trace details for ${path}:`, error);
          console.info(`[route-nav] -> ${path} turnsCache=${getTurnsCacheSize()}`);
        }
      })
      .catch((error: unknown) => {
        console.warn(`[route-nav] Failed to load trace details for ${path}:`, error);
      });

    return () => cancelAnimationFrame(scrollFrame);
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
          navigateWithFeedback("/", () => setMonitorOpen(true), { replace: true });
        }
      } else {
        // Full-max → Compact: switch from popover to route page
        if (monitorOpenRef.current && pathnameRef.current !== "/monitoring") {
          setMonitorOpen(false);
          navigateWithFeedback("/monitoring");
        }
      }
    });

    return () => {
      unsubFrame();
      unsubGate();
    };
  }, [navigateWithFeedback]);

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
        } else {
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
          void getCurrentWindow().close();
        }).catch((error: unknown) => {
          console.error("[layout] Failed to close the window:", error);
        });
        return;
      }
      if (!isCoarsePointer() && mod && (key === "q" || key === "Q")) {
        e.preventDefault();
        import("@tauri-apps/plugin-process").then(({ exit }) => {
          void exit(0);
        }).catch((error: unknown) => {
          console.error("[layout] Failed to quit the application:", error);
        });
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
        // Read through the ref, not the closure — keeps this listener out of
        // the pathname dependency so it isn't torn down and re-added per nav.
        const currentIndex = routes.indexOf(pathnameRef.current);
        if (currentIndex !== -1) {
          setMonitorOpen(false);
          if (key === "ArrowRight") {
            navigateWithFeedback(routes[(currentIndex + 1) % routes.length]);
          } else {
            navigateWithFeedback(routes[(currentIndex - 1 + routes.length) % routes.length]);
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
  }, [navigate, navigateWithFeedback, togglePanel, openPanel, openActiveDrawer, closeActiveDrawer, closeHelp]);

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
    <RouteNavigationProvider value={routeNavigation}>
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
            paused={!["Idle", "Ready", "Paused", "Sleeping"].includes(interactionState) || isResizing}
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
          aria-busy={pendingNavigation !== null}
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
            paddingBottom: isMobile && isHome ? "calc(60px + env(safe-area-inset-bottom))" : undefined,
          }}
        >
          {/* Unified App Header for all layouts */}
          <AppHeader />

          {/* Controller Ambient Rim Glow during active remote session */}
          {isRealMobileDevice() && remoteLinkState === "Connected" && (
            <div
              className="pointer-events-none fixed inset-0 z-50 transition-opacity duration-700"
              style={{
                boxShadow: "inset 0 0 24px rgba(var(--accent), 0.15)",
              }}
            />
          )}

          <Suspense
            fallback={
              <DelayedFallback delayMs={150}>
                <div className="flex-1 w-full h-full flex flex-col items-center justify-center p-6 animate-in fade-in duration-200">
                  <OrbitalLoader size="md" />
                </div>
              </DelayedFallback>
            }
          >
            {(children ?? outlet) && (
              <motion.div
                key={location.pathname}
                // Entrance motion is reserved for the mobile swipe gesture,
                // where the slide communicates lateral tab movement. Click,
                // keyboard, and tap navs commit instantly (`initial={false}`):
                // a fade-from-invisible on every route swap is pure perceived
                // latency. Swipe direction is reset on the next touch start.
                initial={
                  isMobile && swipeDirection !== 0
                    ? { opacity: 0.8, x: swipeDirection > 0 ? "18%" : "-18%" }
                    : false
                }
                animate={{ opacity: 1, x: 0 }}
                transition={{
                  duration: isMobile ? 0.22 : 0.15,
                  ease: [0.16, 1, 0.3, 1],
                }}
                className="flex-1 w-full min-h-0 overflow-hidden flex flex-col transform-gpu"
              >
                {children ?? outlet}
              </motion.div>
            )}
          </Suspense>
          {pendingNavigation && (
            <div
              role="status"
              aria-label={LAYOUT_COPY.nav.loadingPage}
              className={`absolute inset-0 z-50 transition-opacity duration-150 ease-out ${
                pendingNavigation.phase === "revealing" ? "pointer-events-none opacity-0" : "opacity-100"
              }`}
              style={{ backgroundColor: "rgb(var(--background))" }}
            >
              <div
                className="absolute left-1/2 flex items-center justify-center"
                style={{ top: stageTop(isMobile, 0), transform: "translate(-50%, -50%)" }}
              >
                <OrbitalLoader size="md" />
              </div>
            </div>
          )}
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
    </RouteNavigationProvider>
  );
};
