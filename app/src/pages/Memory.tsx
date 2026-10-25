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
  Ghost,
} from "lucide-react";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { subscribeViewportFrame, subscribeViewportGate } from "@/layout/viewportResize";
import {
  getActiveObservations,
  type ObservationRecord,
} from "@/services/memoryService";
import {
  getSessions,
  refreshSessions,
  resolveSessionTitle,
  type SessionRow,
} from "@/services/persistenceService";
import { onSessionsChanged } from "@/services/eventsService";
import { usePersonalMemoryDrawer } from "@/shared/hooks/usePersonalMemoryDrawer";
import { ErrorBoundary, OrbitalLoader, EmptyState } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { Modal } from "@/shared/ui/Modal";
import { EdgePanel, BottomDockFeather } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/context/PanelStateContext";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import { isRealMobileDevice } from "@/layout/breakpoints";
import { stageTop } from "@/layout/stageMetrics";
import { AnimatePresence, motion } from "framer-motion";
import {
  type MemoryGraphRef,
  MemoryLegendOverlay,
  MemoryTimeline,
  MemoryNodeTooltip,
  SessionNodeTooltip,
  MemorySearchBar,
  GraphControlDock,
  MemoryCategory,
  toMemoryCategory,
  PersonalMemoryHeaderActions,
  PersonalMemoryBody,
  PersonalMemoryModalView,
  MemoryErrorState,
  MemoryHint,
} from "@/shared/components/memory";

const MemoryGraph = React.lazy(() =>
  import("@/shared/components/memory/MemoryGraph").then((m) => ({ default: m.MemoryGraph }))
);

let isMemoryWarmLoaded = false;
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

  // Loader display state: on initial cold load, enforce MIN_LOADER_MS to prevent
  // sub-frame flash; on subsequent warm visits, resolve immediately without delay.
  const MIN_LOADER_MS = isMemoryWarmLoaded ? 0 : 350;
  const [loaderShown, setLoaderShown] = useState(false);
  const loaderShownAtRef = useRef(0);
  useEffect(() => {
    if (loading) {
      loaderShownAtRef.current = Date.now();
      setLoaderShown(true);
      return;
    }
    isMemoryWarmLoaded = true;
    const wait = Math.max(0, MIN_LOADER_MS - (Date.now() - loaderShownAtRef.current));
    if (wait === 0) {
      setLoaderShown(false);
      return;
    }
    const id = setTimeout(() => setLoaderShown(false), wait);
    return () => clearTimeout(id);
  }, [loading, MIN_LOADER_MS]);

  // Filtering & Selection
  const [searchQuery, setSearchQuery] = useState("");
  const [searchModalOpen, setSearchModalOpen] = useState(false);
  const [selectedCollection, setSelectedCollection] = useState("all");
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [selectedFact, setSelectedFact] = useState<ObservationRecord | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [selectedSessionNode, setSelectedSessionNode] = useState<{
    session: SessionRow;
    pos: { x: number; y: number };
  } | null>(null);
  const { isPanelOpen, closePanel, openPanel } = usePanelStateContext();
  const sessionRailOpen = isPanelOpen("sessions");
  const isRightPanelOpen = isPanelOpen("help") || isPanelOpen("notifications");
  const setSessionRailOpen = useCallback((v: boolean) => {
    if (!v) closePanel("sessions");
  }, [closePanel]);
  const [selectModeEnabled, setSelectModeEnabled] = useState(false);
  const [isLightMode, setIsLightMode] = useState(
    () => typeof document !== "undefined" && document.documentElement.getAttribute("data-theme") === "light"
  );

  // Fetch session list for title resolution and session tooltips.
  // Mount path serves the stale-while-revalidate cache; the sessions_changed
  // handler is an invalidation path and must bypass it.
  const fetchSessions = useCallback(async (force = false) => {
    try {
      const data = force ? await refreshSessions() : await getSessions();
      if (mountedRef.current) setSessions(data);
    } catch (err) {
      console.warn("[Memory] Failed to load sessions:", err);
    }
  }, []);

  useEffect(() => {
    void fetchSessions();
    const unlisten = onSessionsChanged(() => {
      void fetchSessions(true);
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
  // `force` bypasses the stale-while-revalidate read — used by mutation
  // paths (drawer refresh after consolidation); plain mounts serve the cache.
  const refresh = useCallback(async (isSilent = false, force = false) => {
    if (!isSilent) setLoading(true);
    setRefreshing(true);
    try {
      const activeFacts = await getActiveObservations({ force });
      if (!mountedRef.current) return;
      setFacts(activeFacts);
      setLoadError(null);
    } catch (e) {
      console.error("[Memory] Failed to load data:", e);
      // Failure is an error state, never an empty state.
      if (!mountedRef.current) return;
      setLoadError(MEMORY_COPY.loadFailedDesc);
    } finally {
      if (mountedRef.current) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleRefreshFacts = useCallback((silent = false) => refresh(silent, true), [refresh]);

  // ── Personal Memory Drawer Hook ──────────────────────────────────────────
  const drawer = usePersonalMemoryDrawer({
    facts,
    // Post-mutation path: force a fresh observation read, never the stale cache.
    onRefreshFacts: handleRefreshFacts,
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
  }, [setSessionRailOpen]);

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
    setSearchQuery("");
    setSelectedCollection("all");
    graphRef.current?.recenter();
    void refresh(true);
    void drawer.refreshPersonalMemory(true);
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

      // Shift + ArrowUp -> Focus Core Personal Memory
      if (shift && e.key === "ArrowUp") {
        e.preventDefault();
        handleCoreClick();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleRecenter, handleZoomIn, handleZoomOut, handleCoreClick]);

  const headerCenterSlot =
    !isCompactLayout && typeof document !== "undefined"
      ? document.getElementById("app-header-center-slot")
      : null;

  return (
    <div
      ref={containerRef}
      className="relative flex-1 flex flex-col min-w-0 min-h-0 h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* NOTE: no page-level AmbientBackground here. ResponsiveLayout renders one
          app-wide instance (standardised origin); a second frozen copy used to
          mount here, doubling overdraw with a competing ripple centre. */}

      {/* ── Center Search Bar: Portaled into AppHeader center slot for desktop main layout ── */}
      {headerCenterSlot &&
        createPortal(
          <MemorySearchBar
            facts={facts}
            searchQuery={searchQuery}
            isLightMode={isLightMode}
            onCommitSearch={setSearchQuery}
            onSelectNode={handleSelectSearchNode}
            dropdownPlacement="bottom"
            className="w-full max-w-[280px]"
          />,
          headerCenterSlot
        )}




      {/* ── Right Edge: Floating Graph Control Dock (Rendered when memories exist) ── */}
      {facts.length > 0 && (
        <GraphControlDock
          onRecenter={handleRecenter}
          onZoomIn={handleZoomIn}
          onZoomOut={handleZoomOut}
          onRefresh={handleRefreshDock}
          onFocusCore={handleFocusCore}
          refreshing={refreshing}
          selectModeEnabled={selectModeEnabled}
          onToggleSelectMode={handleToggleSelectMode}
          onOpenSearch={isCompactLayout ? () => setSearchModalOpen(true) : undefined}
        />
      )}

      {/* ── Bottom Right: Category Legend Overlay — matches ModelStatusOverlay dock position & feather ── */}
      {!drawerOpen && typeof document !== "undefined" && facts.length > 0 &&
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
            sessions={sessions}
            isLightMode={isLightMode}
            selectedSessionId={selectedSessionId}
            selectedFactId={selectedFact?.id ?? null}
            onSelectSession={handleSelectSession}
            onSelectFact={handleSelectFactFromRail}
            onClose={handleCloseSessionRail}
          />
        </ErrorBoundary>
      </EdgePanel>

      {/* ── 3D Dynamic WebGL Graph Canvas (Rendered when memories exist) ── */}
      {dims.w > 0 && facts.length > 0 && (
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
            <React.Suspense fallback={null}>
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
            </React.Suspense>
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
          top: stageTop(isRealMobileDevice(), 36),
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

      {/* ── Empty State (Matches History.tsx placement, UI, size, icon, and styling) ── */}
      {!loading && !loadError && facts.length === 0 && (
        <div className="absolute inset-0 z-20 pointer-events-none">
          <div
            className="absolute left-1/2 flex items-center justify-center pointer-events-none"
            style={{
              top: stageTop(isRealMobileDevice(), 36),
              transform: "translate(-50%, -50%)",
              padding: "2rem",
            }}
          >
            <EmptyState
              icon={Ghost}
              title={MEMORY_COPY.emptyFactsTitle}
              description={MEMORY_COPY.emptyFactsDesc}
              className="max-w-sm border-0 bg-transparent"
            />
          </div>
        </div>
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
          className="w-full max-w-[calc(100vw-16px)] h-[calc(100dvh-32px)] sm:w-[min(920px,86vw)] sm:max-w-[calc(100vw-36px)] sm:h-[min(740px,76vh)] sm:max-h-[calc(100dvh-120px)]"
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

      {/* ── Search Modal for Mobile / Compact Viewports ── */}
      {isCompactLayout && (
        <Modal
          open={searchModalOpen}
          onClose={() => setSearchModalOpen(false)}
          position="global"
          title={MEMORY_COPY.searchMemories}
          ariaLabel={MEMORY_COPY.searchMemories}
          className="w-[min(480px,92vw)] h-[min(540px,72vh)] p-4 rounded-2xl bg-[rgba(var(--card),0.92)] border border-[rgba(var(--border),0.2)] shadow-xl"
          bodyClassName="h-full flex flex-col min-h-0 overflow-hidden"
        >
          <div className="flex flex-col h-full min-h-0 pt-1">
            <MemorySearchBar
              facts={facts}
              searchQuery={searchQuery}
              isLightMode={isLightMode}
              variant="modal"
              autoFocus
              onCommitSearch={(q) => {
                setSearchQuery(q);
              }}
              onSelectNode={(factId) => {
                handleSelectSearchNode(factId);
                setSearchModalOpen(false);
              }}
              className="w-full flex-1 min-h-0"
            />
          </div>
        </Modal>
      )}

    </div>
  );
});

Memory.displayName = "Memory";
export default Memory;
