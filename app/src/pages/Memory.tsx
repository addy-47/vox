import React, {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
  memo,
} from "react";
import { createPortal } from "react-dom";
import {
  PanelLeft,
} from "lucide-react";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { subscribeViewportFrame, subscribeViewportGate } from "@/layout/viewportResize";
import { BREAKPOINT_OPPOSITE_COLLISION_MAX } from "@/layout/breakpoints";
import {
  getActiveObservations,
  type ObservationRecord,
} from "@/services/memoryService";
import {
  getSessions,
  resolveSessionTitle,
  type SessionRow,
} from "@/services/historyService";
import { onSessionsChanged } from "@/services/eventsService";
import { usePersonalMemoryDrawer } from "@/shared/hooks/usePersonalMemoryDrawer";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { Modal } from "@/shared/ui/Modal";
import { EdgePanel, Tooltip, BottomDockFeather, ThemeToggleButton } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import { AnimatePresence, motion } from "framer-motion";
import {
  MemoryGraph,
  MemoryGraphRef,
  MemoryLegendOverlay,
  MemoryTimeline,
  MemoryNodeTooltip,
  SessionNodeTooltip,
  SearchBar,
  GraphControlDock,
  MemoryCategory,
  toMemoryCategory,
  PersonalMemoryHeaderActions,
  PersonalMemoryBody,
  PersonalMemoryModalView,
  MemoryErrorState,
  MemoryEmptyState,
  MemoryHint,
} from "@/shared/components/memory";

export const Memory: React.FC = memo(() => {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<MemoryGraphRef>(null);

  // Viewport dimensions
  const [dims, setDims] = useState<{ w: number; h: number }>({ w: 0, h: 0 });

  // Data state
  const [facts, setFacts] = useState<ObservationRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  // Load failure is a first-class state (never rendered as "no memories").
  const [loadError, setLoadError] = useState<string | null>(null);

  // Loader display state: enters immediately, exits with a fade, and never
  // flashes for less than MIN_LOADER_MS (avoids a sub-frame spinner blink on
  // warm-cache loads, and a mid-fade hard cut on slow ones).
  const MIN_LOADER_MS = 400;
  const [loaderShown, setLoaderShown] = useState(false);
  const loaderShownAtRef = useRef(0);
  useEffect(() => {
    if (loading) {
      loaderShownAtRef.current = Date.now();
      setLoaderShown(true);
      return;
    }
    const wait = Math.max(0, MIN_LOADER_MS - (Date.now() - loaderShownAtRef.current));
    if (wait === 0) {
      setLoaderShown(false);
      return;
    }
    const id = setTimeout(() => setLoaderShown(false), wait);
    return () => clearTimeout(id);
  }, [loading]);

  // Filtering & Selection
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState("all");
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [selectedFact, setSelectedFact] = useState<ObservationRecord | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [selectedSessionNode, setSelectedSessionNode] = useState<{
    session: SessionRow;
    pos: { x: number; y: number };
  } | null>(null);
  const { isPanelOpen, closePanel, openPanel, togglePanel, rightPanel } = usePanelStateContext();
  const sessionRailOpen = isPanelOpen("sessions");
  const isRightPanelOpen = isPanelOpen("help") || isPanelOpen("notifications");
  const setSessionRailOpen = (v: boolean) => {
    if (!v) closePanel("sessions");
  };
  const [selectModeEnabled, setSelectModeEnabled] = useState(false);
  const [isLightMode, setIsLightMode] = useState(false);

  // Fetch session list for title resolution and session tooltips
  const fetchSessions = useCallback(async () => {
    try {
      const data = await getSessions();
      if (mountedRef.current) setSessions(data);
    } catch (err) {
      console.warn("[Memory] Failed to load sessions:", err);
    }
  }, []);

  useEffect(() => {
    fetchSessions();
    const unlisten = onSessionsChanged(() => {
      fetchSessions();
    });
    return () => {
      unlisten();
    };
  }, [fetchSessions]);

  const sessionsMap = useMemo(() => {
    const map = new Map<number, SessionRow>();
    for (const s of sessions) {
      map.set(s.id, s);
    }
    return map;
  }, [sessions]);

  const sessionFactsMap = useMemo(() => {
    const map = new Map<number, ObservationRecord[]>();
    for (const f of facts) {
      if (f.session_id !== null) {
        const list = map.get(f.session_id) || [];
        list.push(f);
        map.set(f.session_id, list);
      }
    }
    return map;
  }, [facts]);

  const selectedFactSessionTitle = useMemo(() => {
    if (!selectedFact || selectedFact.session_id === null) return null;
    const s = sessionsMap.get(selectedFact.session_id);
    return s ? resolveSessionTitle(s) : `${MEMORY_COPY.sessionPrefix}${selectedFact.session_id}`;
  }, [selectedFact, sessionsMap]);

  // Dynamic collision threshold between right panel and left triggers
  const isNarrowCollision = dims.w > 0 ? dims.w < BREAKPOINT_OPPOSITE_COLLISION_MAX : (typeof window !== "undefined" ? window.innerWidth < BREAKPOINT_OPPOSITE_COLLISION_MAX : false);
  const hideLeftCluster = Boolean(rightPanel) && isNarrowCollision;

  useEffect(() => {
    const checkTheme = () => {
      setIsLightMode(document.documentElement.getAttribute("data-theme") === "light");
    };
    checkTheme();
    const observer = new MutationObserver(checkTheme);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);

  // ── Measure Container ──────────────────────────────────────────────────────
  // The graph mounts once real dimensions exist (dims stay > 0 afterwards,
  // so no separate "has mounted" flag is needed — and ref writes do not
  // belong in the render body).

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    let rAfId: number | null = null;
    const updateDims = () => {
      const rect = el.getBoundingClientRect();
      const w = Math.round(rect.width);
      const h = Math.round(rect.height);
      if (w > 0 && h > 0) {
        setDims((prev) => (prev.w === w && prev.h === h ? prev : { w, h }));
      }
    };

    updateDims();

    const obs = new ResizeObserver((entries) => {
      if (entries.length > 0) {
        const { width, height } = entries[0].contentRect;
        const w = Math.round(width);
        const h = Math.round(height);
        if (w > 0 && h > 0) {
          if (rAfId !== null) cancelAnimationFrame(rAfId);
          rAfId = requestAnimationFrame(() => {
            setDims((prev) => (prev.w === w && prev.h === h ? prev : { w, h }));
          });
        }
      }
    });
    obs.observe(el);

    const unsubFrame = subscribeViewportFrame(updateDims);
    const unsubGate = subscribeViewportGate((active) => {
      if (!active) updateDims();
    });

    return () => {
      obs.disconnect();
      unsubFrame();
      unsubGate();
      if (rAfId !== null) cancelAnimationFrame(rAfId);
    };
  }, []);

  // Mounted guard for the async load below (style-guide §4.3).
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // ── Load Memory Data ───────────────────────────────────────────────────────
  const refresh = useCallback(async (isSilent = false) => {
    if (!isSilent) setLoading(true);
    setRefreshing(true);
    try {
      const activeFacts = await getActiveObservations();
      if (!mountedRef.current) return;
      setFacts(activeFacts);
      setLoadError(null);
    } catch (e) {
      console.error("[Memory] Failed to load data:", e);
      // Failure is an error state, never an empty state.
      if (!mountedRef.current) return;
      setLoadError(MEMORY_COPY.loadFailedDesc);
    } finally {
      if (!mountedRef.current) return;
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // ── Personal Memory Drawer Hook ──────────────────────────────────────────
  const drawer = usePersonalMemoryDrawer({
    facts,
    onRefreshFacts: refresh,
    onError: setLoadError,
  });
  const { drawerOpen } = drawer;

  // Compact viewports render Personal Memory as a centered modal (Tier 2b);
  // wide viewports keep the bottom drawer. Single source: the viewport gate.
  const { layout } = useViewportResize();
  const isCompactLayout = layout === "compact";

  const personalMemoryTitle = (
    <div className="flex items-center gap-2.5">
      <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
        {MEMORY_COPY.personalMemory}
      </span>
    </div>
  );
  const personalMemorySubtitle = (
    <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
      {MEMORY_COPY.drawerSubtitle}
    </span>
  );

  // Counts per category for the Legend
  const categoryCounts = useMemo(() => {
    const counts: Partial<Record<MemoryCategory, number>> = {};
    for (const f of facts) {
      const cat = toMemoryCategory(f.fact_type);
      if (cat !== undefined) counts[cat] = (counts[cat] || 0) + 1;
    }
    return counts;
  }, [facts]);

  // ── Node & Core Click Handlers ─────────────────────────────────────────────
  const handleSelectNode = useCallback((fact: ObservationRecord | null, pos?: { x: number; y: number }) => {
    setSelectedFact(fact);
    setSelectedSessionNode(null);
    if (fact && pos) {
      setTooltipPos(pos);
    } else {
      setTooltipPos(null);
    }
  }, []);

  const handleSelectSessionNode = useCallback((sessionId: string, pos?: { x: number; y: number }) => {
    setSelectedFact(null);
    setTooltipPos(null);
    const sNum = Number(sessionId);
    const sess = sessions.find((s) => s.id === sNum);
    if (sess && pos) {
      setSelectedSessionNode({ session: sess, pos });
    } else {
      setSelectedSessionNode(null);
    }
  }, [sessions]);

  const handleSelectSession = useCallback((sId: string | null) => {
    setSelectedSessionId(sId);
    if (sId === null) {
      setSelectedSessionNode(null);
    } else {
      graphRef.current?.flyToSession(sId);
    }
  }, []);

  const handleSelectFactFromRail = useCallback((fact: ObservationRecord) => {
    setSelectedSessionNode(null);
    setSelectedFact(fact);
    setTooltipPos({ x: window.innerWidth / 2, y: window.innerHeight / 2 });
    graphRef.current?.flyToNode(fact.id);
  }, []);

  const handleCloseSessionRail = useCallback(() => {
    setSessionRailOpen(false);
  }, []);

  const handleCoreClick = useCallback(() => {
    // Dismiss floating tooltips before opening drawer to release overlay Escape listener
    setSelectedFact(null);
    setSelectedSessionNode(null);
    setTooltipPos(null);
    drawer.openDrawer();
  }, [drawer]);

  const handleRecenter = useCallback(() => graphRef.current?.recenter(), []);
  const handleZoomIn = useCallback(() => graphRef.current?.zoomIn(), []);
  const handleZoomOut = useCallback(() => graphRef.current?.zoomOut(), []);
  const handleRefreshDock = useCallback(() => {
    setSelectedSessionId(null);
    setSelectedSessionNode(null);
    setSelectedFact(null);
    setTooltipPos(null);
    graphRef.current?.recenter();
    refresh(true);
    drawer.refreshPersonalMemory(true);
  }, [refresh, drawer]);
  const handleFocusCore = useCallback(() => graphRef.current?.focusCore(), []);
  const handleToggleSelectMode = useCallback(() => setSelectModeEnabled((prev) => !prev), []);
  const handleSelectSearchNode = useCallback(
    (factId: string | null) => {
      if (!factId) return;
      const f = facts.find((fact) => fact.id === factId);
      if (f) {
        handleSelectNode(f, { x: window.innerWidth / 2, y: window.innerHeight / 2 });
        graphRef.current?.flyToNode(f.id);
      }
    },
    [facts, handleSelectNode]
  );

  const handleTooltipClose = useCallback(() => {
    setSelectedFact(null);
    setTooltipPos(null);
  }, []);

  // Keyboard shortcuts specific to Memory page (Ctrl+K search, Ctrl+R recenter, Shift+= zoom in, Shift+- zoom out)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      const tag = activeEl?.tagName.toLowerCase();
      const isEditable =
        tag === "input" ||
        tag === "textarea" ||
        tag === "select" ||
        activeEl?.getAttribute("contenteditable") === "true";

      const mod = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;

      // Ctrl + K -> Focus Search Bar
      if (mod && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        const searchInput = document.getElementById("memory-search-input") as HTMLInputElement | null;
        if (searchInput) {
          searchInput.focus();
          searchInput.select();
        }
        return;
      }

      if (isEditable) return;

      // Ctrl + R -> Recenter Graph
      if (mod && (e.key === "r" || e.key === "R")) {
        e.preventDefault();
        handleRecenter();
        return;
      }

      // Shift + = or + -> Zoom In
      if (shift && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        handleZoomIn();
        return;
      }

      // Shift + - or _ -> Zoom Out
      if (shift && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        handleZoomOut();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleRecenter, handleZoomIn, handleZoomOut]);

  return (
    <div
      ref={containerRef}
      className="relative flex-1 flex flex-col min-w-0 min-h-0 h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* NOTE: no page-level AmbientBackground here. ResponsiveLayout renders one
          app-wide instance (standardised origin); a second frozen copy used to
          mount here, doubling overdraw with a competing ripple centre. */}

      {/* ── Top Header Bar: Unified flex container with relative balance and guaranteed gap ── */}
      <header className="absolute top-0 left-0 right-0 h-16 px-4 sm:px-5 flex items-center justify-between z-40 pointer-events-none">
        {/* Left: Session Rail Trigger & Theme Toggle */}
        <div
          className={cn(
            "flex items-center gap-2.5 shrink-0 z-[60] transition-opacity duration-200",
            hideLeftCluster
              ? "opacity-0 pointer-events-none invisible"
              : "pointer-events-auto"
          )}
        >
          <Tooltip label="Memory Queue" side="bottom">
            <button
              onClick={() => togglePanel("sessions")}
              aria-label="Open session rail"
              aria-expanded={sessionRailOpen}
              data-edge-trigger="left"
              className={cn(
                "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                sessionRailOpen
                  ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)]"
                  : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
              )}
            >
              <PanelLeft size={14} strokeWidth={1.75} />
            </button>
          </Tooltip>
          <ThemeToggleButton />
        </div>

        {/* Center: Search Bar (Responsive width, strictly centered between corner clusters with safety gap) */}
        <div className="flex-1 flex justify-center px-3 sm:px-6 min-w-0 pointer-events-auto">
          <SearchBar
            facts={facts}
            isLightMode={isLightMode}
            onCommitSearch={setSearchQuery}
            onSelectNode={handleSelectSearchNode}
            dropdownPlacement="bottom"
            className="w-full max-w-[280px]"
          />
        </div>

        {/* Right Spacer: Symmetrically mirrors TopRightCluster width so center stays dead-center */}
        <div className="w-[76px] shrink-0 pointer-events-none" aria-hidden="true" />
      </header>



      {/* ── Right Edge: Floating Graph Control Dock ── */}
      <GraphControlDock
        onRecenter={handleRecenter}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onRefresh={handleRefreshDock}
        onFocusCore={handleFocusCore}
        refreshing={refreshing}
        selectModeEnabled={selectModeEnabled}
        onToggleSelectMode={handleToggleSelectMode}
      />

      {/* ── Bottom Right: Category Legend Overlay — matches ModelStatusOverlay dock position & feather ── */}
      {!drawerOpen && typeof document !== "undefined" &&
        createPortal(
          <div className="hidden lg:flex fixed bottom-4 right-4 z-40 pointer-events-none items-center">
            {isRightPanelOpen && (
              <BottomDockFeather className="absolute -right-4 -bottom-4 -top-12 w-[340px] pointer-events-none" />
            )}
            <div className="relative pointer-events-auto flex items-center px-3 lg:px-4 py-2.5 bg-transparent border-transparent shadow-none">
              <MemoryLegendOverlay
                selectedCollection={selectedCollection}
                onSelectCollection={setSelectedCollection}
                counts={categoryCounts}
                isLightMode={isLightMode}
              />
            </div>
          </div>,
          document.body
        )}

      {/* ── Left Edge Rail: Ingestion Timeline & Session Episodic Memories ── */}
      <EdgePanel side="left" open={sessionRailOpen} onClose={handleCloseSessionRail} minimalHeader>
        <ErrorBoundary name="MemoryTimeline">
          <MemoryTimeline
            facts={facts}
            selectedSessionId={selectedSessionId}
            selectedFactId={selectedFact?.id ?? null}
            onSelectSession={handleSelectSession}
            onSelectFact={handleSelectFactFromRail}
            onClose={handleCloseSessionRail}
          />
        </ErrorBoundary>
      </EdgePanel>

      {/* ── 3D Dynamic WebGL Graph Canvas ── */}
      {dims.w > 0 && (
        <motion.div
          className={cn(
            "absolute inset-0 w-full h-full",
            loaderShown ? "pointer-events-none" : "pointer-events-auto"
          )}
          initial={{ opacity: 0 }}
          animate={{ opacity: loaderShown ? 0 : 1 }}
          transition={{
            duration: 0.55,
            ease: [0.16, 1, 0.3, 1],
          }}
        >
          <ErrorBoundary name="Memory3DGraph">
            <MemoryGraph
              ref={graphRef}
              facts={facts}
              width={Math.max(dims.w, 1)}
              height={Math.max(dims.h, 1)}
              searchQuery={searchQuery}
              selectedCollection={selectedCollection}
              selectedFactId={selectedFact?.id ?? null}
              selectedSessionId={selectedSessionId}
              onSelectNode={handleSelectNode}
              onSelectSessionNode={handleSelectSessionNode}
              onCoreClick={handleCoreClick}
              selectModeEnabled={selectModeEnabled}
              paused={drawerOpen || sessionRailOpen}
            />
          </ErrorBoundary>
        </motion.div>
      )}

      {/* ── Floating Fact Detail Tooltip ── */}
      {selectedFact && tooltipPos && !drawerOpen && (
        <MemoryNodeTooltip
          factDetail={selectedFact}
          pos={tooltipPos}
          sessionTitle={selectedFactSessionTitle}
          onClose={handleTooltipClose}
          isLightMode={isLightMode}
        />
      )}

      {/* ── Floating Session Node Detail Tooltip ── */}
      {selectedSessionNode && !drawerOpen && (
        <SessionNodeTooltip
          session={selectedSessionNode.session}
          facts={sessionFactsMap.get(selectedSessionNode.session.id) || []}
          pos={selectedSessionNode.pos}
          onClose={() => setSelectedSessionNode(null)}
          onFocusSession={(sId) => {
            setSelectedSessionId(sId);
            openPanel("sessions");
          }}
          isLightMode={isLightMode}
        />
      )}

      {/* ── Ambient Orbital Loading State (enter + exit, strictly centered at 3D camera projection offset) ── */}
      <div
        className="absolute left-1/2 flex flex-col items-center justify-center pointer-events-none z-20"
        style={{
          top: "calc(50% - 36px)",
          transform: "translate(-50%, -50%)",
        }}
      >
        <AnimatePresence>
          {loaderShown && (
            <motion.div
              key="memory-loader"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.35, ease: "easeOut" }}
              className="flex flex-col items-center justify-center"
            >
              <OrbitalLoader size="md" />
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* ── Load Error State ── */}
      {!loading && loadError && (
        <MemoryErrorState error={loadError} onRetry={refresh} />
      )}

      {/* ── Empty State ── */}
      {!loading && !loadError && facts.length === 0 && (
        <MemoryEmptyState />
      )}

      {/* ── Memory Page Footnote Hint ── */}
      {!drawerOpen && !loading && (
        <MemoryHint />
      )}

      {/* ── Personal Memory Surface: bottom drawer on wide, centered modal on compact ── */}
      {isCompactLayout ? (
        <Modal
          open={drawerOpen}
          onClose={drawer.closeDrawer}
          position="global"
          ariaLabel={MEMORY_COPY.personalMemory}
          className="w-[min(920px,94vw)] h-[min(780px,88vh)]"
          bodyClassName="h-full flex flex-col min-h-0 overflow-hidden p-0"
        >
          <ErrorBoundary name="MemoryModalContent">
            <PersonalMemoryModalView drawer={drawer} onClose={drawer.closeDrawer} />
          </ErrorBoundary>
        </Modal>
      ) : (
        <Drawer
          open={drawerOpen}
          onClose={drawer.closeDrawer}
          position="global"
          ariaLabel={MEMORY_COPY.personalMemory}
          height={65}
          title={personalMemoryTitle}
          subtitle={personalMemorySubtitle}
          headerActions={<PersonalMemoryHeaderActions drawer={drawer} />}
          bodyClassName="px-4 sm:px-6 py-4 overflow-y-auto lg:overflow-hidden h-full flex flex-col min-h-0"
        >
          <ErrorBoundary name="MemoryDrawerContent">
            <PersonalMemoryBody drawer={drawer} />
          </ErrorBoundary>
        </Drawer>
      )}

    </div>
  );
});

Memory.displayName = "Memory";
export default Memory;
