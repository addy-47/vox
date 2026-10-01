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
  Zap,
  Sparkles,
  PanelLeft,
  Tag,
} from "lucide-react";
import {
  getActiveObservations,
  type ObservationRecord,
} from "@/services/memoryService";
import { usePersonalMemoryDrawer } from "@/shared/hooks/usePersonalMemoryDrawer";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { EdgePanel, Tooltip, BottomDockFeather } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import { AnimatePresence, motion } from "framer-motion";
import {
  MemoryGraph,
  MemoryGraphRef,
  MemoryLegendOverlay,
  MemorySessionRail,
  MemoryNodeTooltip,
  SearchBar,
  GraphControlDock,
  MemoryCategory,
  toMemoryCategory,
  PersonalMemoryDossierCard,
  PersonalMemoryStagingCard,
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
  const { isPanelOpen, closePanel, togglePanel } = usePanelStateContext();
  const sessionRailOpen = isPanelOpen("sessions");
  const isRightPanelOpen = isPanelOpen("help") || isPanelOpen("notifications");
  const setSessionRailOpen = (v: boolean) => {
    if (!v) closePanel("sessions");
  };
  const [selectModeEnabled, setSelectModeEnabled] = useState(false);
  const [isLightMode, setIsLightMode] = useState(false);

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
    const obs = new ResizeObserver((entries) => {
      if (entries.length > 0) {
        const { width, height } = entries[0].contentRect;
        if (width > 0 && height > 0) {
          if (rAfId !== null) cancelAnimationFrame(rAfId);
          rAfId = requestAnimationFrame(() => {
            setDims({ w: width, h: height });
          });
        }
      }
    });
    obs.observe(el);
    return () => {
      obs.disconnect();
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
    if (fact && pos) {
      setTooltipPos(pos);
    } else {
      setTooltipPos(null);
    }
  }, []);

  const handleSelectSession = useCallback((sId: string | null) => {
    setSelectedSessionId(sId);
    if (sId) {
      graphRef.current?.flyToSession(sId);
    }
  }, []);

  const handleSelectFactFromRail = useCallback((fact: ObservationRecord) => {
    setSelectedFact(fact);
    setTooltipPos({ x: window.innerWidth / 2, y: window.innerHeight / 2 });
    graphRef.current?.flyToNode(fact.id);
  }, []);


  const handleCloseSessionRail = useCallback(() => {
    setSessionRailOpen(false);
  }, []);

  const handleCoreClick = useCallback(() => {
    // Dismiss floating tooltip before opening drawer to release its overlay Escape listener
    setSelectedFact(null);
    setTooltipPos(null);
    drawer.openDrawer();
  }, [drawer]);

  const handleRecenter = useCallback(() => graphRef.current?.recenter(), []);
  const handleZoomIn = useCallback(() => graphRef.current?.zoomIn(), []);
  const handleZoomOut = useCallback(() => graphRef.current?.zoomOut(), []);
  const handleRefreshDock = useCallback(() => {
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
      className="relative flex-1 flex flex-col h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* NOTE: no page-level AmbientBackground here. ResponsiveLayout renders one
          app-wide instance (standardised origin); a second frozen copy used to
          mount here, doubling overdraw with a competing ripple centre. */}

      {/* ── Top Bar Search: Dynamic width with generous gap to triggers on both sides ── */}
      <div className="absolute top-4 left-24 right-32 z-30 pointer-events-auto flex justify-center">
        <SearchBar
          facts={facts}
          isLightMode={isLightMode}
          onCommitSearch={setSearchQuery}
          onSelectNode={handleSelectSearchNode}
          dropdownPlacement="bottom"
          className="w-full max-w-[280px]"
        />
      </div>

      {/* ── Top Left: Session Rail Trigger — mirrors Home trigger, self-contained in Memory ── */}
      <div className="absolute top-4 left-5 z-[60] pointer-events-auto">
        <Tooltip label="Session history" side="bottom">
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
      </div>



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

      {/* ── Left Edge Rail: Memory Session History & Compactions — minimal header like SessionPanel ── */}
      <EdgePanel side="left" open={sessionRailOpen} onClose={handleCloseSessionRail} minimalHeader>
        <ErrorBoundary name="MemorySessionRail">
          <MemorySessionRail
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
              onCoreClick={handleCoreClick}
              selectModeEnabled={selectModeEnabled}
              paused={false}
            />
          </ErrorBoundary>
        </motion.div>
      )}

      {/* ── Floating Fact Detail Tooltip ── */}
      {selectedFact && tooltipPos && !drawerOpen && (
        <MemoryNodeTooltip
          factDetail={selectedFact}
          pos={tooltipPos}
          onClose={handleTooltipClose}
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
              <OrbitalLoader
                size="md"
                title={MEMORY_COPY.graphLoadingTitle}
                subtitle={MEMORY_COPY.graphLoadingSubtitle}
              />
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

      {/* ── Bottom-Sheet Personal Memory Drawer ── */}
      <Drawer
        open={drawerOpen}
        onClose={drawer.closeDrawer}
        position="global"
        ariaLabel={MEMORY_COPY.personalMemory}
        height={65}
        title={
          <div className="flex items-center gap-2.5">
            <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
              {MEMORY_COPY.personalMemory}
            </span>
          </div>
        }
        subtitle={
          <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
            {MEMORY_COPY.drawerSubtitle}
          </span>
        }
        headerActions={
          <div className="flex items-center gap-2 flex-wrap">
            <Tooltip label="View candidate and historical observations">
              <button
                type="button"
                onClick={() =>
                  drawer.setStagingMode((prev) => (prev === "facts" ? "idle" : "facts"))
                }
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono border transition-all cursor-pointer shadow-sm",
                  drawer.stagingMode === "facts"
                    ? "bg-[rgba(var(--accent),0.25)] border-[rgba(var(--accent),0.55)] text-[rgb(var(--accent))]"
                    : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.18)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)]"
                )}
              >
                <Tag
                  size={12}
                  className={drawer.stagingMode === "facts" ? "text-[rgb(var(--accent))]" : ""}
                />
                <span>{MEMORY_COPY.viewObservations}</span>
                {drawer.unconsolidatedIdentityCount > 0 && (
                  <span className="text-[10.5px] font-mono text-[rgb(var(--accent))]">
                    ({drawer.unconsolidatedIdentityCount})
                  </span>
                )}
              </button>
            </Tooltip>

            {drawer.suggestions.length > 0 && drawer.stagingMode !== "suggestions" && (
              <Tooltip label="Review proposed profile updates">
                <button
                  type="button"
                  onClick={() => drawer.setStagingMode("suggestions")}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono border border-emerald-500/40 bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 transition-all cursor-pointer shadow-sm animate-pulse"
                >
                  <Sparkles size={12} />
                  <span>Review Suggestions</span>
                  <span className="text-[10.5px] font-mono text-emerald-400">
                    ({drawer.suggestions.length})
                  </span>
                </button>
              </Tooltip>
            )}

            <Tooltip label={drawer.unconsolidatedIdentityCount > 0 ? "Integrate staged observations into personal profile" : "No new observations to integrate"}>
              <button
                type="button"
                onClick={() => drawer.handleConsolidateNow(false)}
                disabled={drawer.consolidating || drawer.unconsolidatedIdentityCount === 0}
                className={cn(
                  "flex items-center gap-2 px-3 py-1.5 rounded-xl text-[11px] font-mono border transition-all disabled:opacity-40 disabled:cursor-not-allowed shadow-sm",
                  drawer.consolidating
                    ? "bg-[rgba(var(--accent),0.25)] border-[rgba(var(--accent),0.5)] text-[rgb(var(--accent))]"
                    : drawer.unconsolidatedIdentityCount > 0
                    ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)] cursor-pointer"
                    : "bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground-muted))]/40"
                )}
              >
                <Zap
                  size={12}
                  className={cn(drawer.consolidating && "animate-pulse text-[rgb(var(--accent))]")}
                />
                <span>
                  {drawer.consolidating
                    ? MEMORY_COPY.consolidating
                    : MEMORY_COPY.consolidate}
                </span>
                {drawer.unconsolidatedIdentityCount > 0 && (
                  <span className="text-[10.5px] font-mono text-[rgb(var(--accent))]">
                    ({drawer.unconsolidatedIdentityCount})
                  </span>
                )}
              </button>
            </Tooltip>
          </div>
        }
        bodyClassName="px-4 sm:px-6 py-4 overflow-y-auto lg:overflow-hidden h-full flex flex-col min-h-0"
      >
        <ErrorBoundary name="MemoryDrawerContent">
          <div className="w-full h-full flex-1 min-h-0">
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-5 h-full min-h-0 w-full items-stretch">
              {/* Left Column: Canonical Persistent Memory (DB Ground Truth) */}
              <PersonalMemoryDossierCard
                personalMemory={drawer.personalMemory}
                displayedRecord={drawer.displayedRecord}
                versions={drawer.versions}
                onSelectVersion={drawer.handleSelectVersion}
                onCommitActiveVersion={drawer.handleRestoreActive}
                isRestoringVersion={drawer.isRestoringVersion}
                onCopyDoc={drawer.handleCopyDoc}
                copied={drawer.copied}
                onRegenerateFromFacts={drawer.handleRegenerateFromFacts}
                isRegenerating={drawer.isRegenerating}
                saving={drawer.saving}
                leftFlash={drawer.leftFlash}
                veilCycle={drawer.veilCycle}
                onVeilReady={drawer.handleVeilReady}
                dossierContainerRef={drawer.dossierContainerRef}
                selectionAnchor={drawer.selectionAnchor}
                onAddComment={drawer.handleAddComment}
                onCancelComment={drawer.handleCancelComment}
                setIsComposingComment={drawer.setIsComposingComment}
                comments={drawer.comments}
                onSelectComment={() => drawer.setStagingMode("comment")}
                drawerOpen={drawerOpen}
                drawerBodyReady={drawer.drawerBodyReady}
              />

              {/* Right Column: Dynamic Workspace / Staging Slate */}
              <PersonalMemoryStagingCard
                canonicalContent={drawer.displayedRecord?.content ?? ""}
                canonicalMarkdown={drawer.displayedRecord?.markdown ?? ""}
                activeVersion={drawer.personalMemory?.version ?? 1}
                mode={drawer.stagingMode}
                onModeChange={drawer.handleStagingModeChange}
                onSave={drawer.handleSaveStaging}
                onRegenerateWithComments={drawer.handleRegenerateWithComments}
                comments={drawer.comments}
                onDeleteComment={drawer.handleDeleteComment}
                onUpdateComment={drawer.handleUpdateComment}
                onClearComments={drawer.handleClearComments}
                unconsolidatedCount={drawer.unconsolidatedIdentityCount}
                isSaving={drawer.saving}
                isConsolidating={drawer.consolidating}
                isCommitting={drawer.isCommitting}
                suggestions={drawer.suggestions}
                onApplySuggestions={drawer.handleApplySuggestions}
                onDismissCommitted={() => drawer.setJustCommitted(false)}
                isApplyingSuggestions={drawer.isApplyingSuggestions}
                candidateFacts={drawer.identityCandidateFacts}
                observations={drawer.paginatedObservations}
                observationFilter={drawer.obsStatusFilter}
                onObservationFilterChange={drawer.setObsStatusFilter}
                isLoadingObservations={drawer.obsLoading}
                isLoadingMoreObservations={drawer.obsLoadingMore}
                hasMoreObservations={drawer.obsHasMore}
                onLoadMoreObservations={drawer.obsLoadMore}
                pendingConfirmation={drawer.pendingConfirmation}
                onConfirmPendingIntegration={drawer.handleConfirmPendingIntegration}
                onCancelPendingConfirmation={drawer.handleCancelPendingConfirmation}
                justCommitted={drawer.justCommitted}
                onViewVersionHistory={() => {
                  drawer.setJustCommitted(false);
                }}
              />
            </div>
          </div>
        </ErrorBoundary>
      </Drawer>

    </div>
  );
});

Memory.displayName = "Memory";
export default Memory;
