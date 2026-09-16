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
  Copy,
  Check,
  Zap,
  Sparkles,
  PanelLeft,
  FileText,
} from "lucide-react";
import {
  getPersonalMemory,
  savePersonalMemory,
  consolidatePersonalMemory,
  getActiveFacts,
  type PersonalMemoryRecord,
  type FactRecord,
} from "@/services/memoryService";
import { AmbientBackground, ErrorBoundary } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { EdgePanel, Tooltip, Markdown } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import {
  MemoryGraph,
  MemoryGraphRef,
  MemoryLegendOverlay,
  MemorySessionRail,
  MemoryNodeTooltip,
  SearchBar,
  GraphControlDock,
  MemoryCategory,
  PersonalMemoryStagingCard,
  type StagingMode,
  PixelSynthesisCanvas,
} from "@/shared/components/memory";

export const Memory: React.FC = memo(() => {
  const containerRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<MemoryGraphRef>(null);

  // Viewport dimensions
  const [dims, setDims] = useState<{ w: number; h: number }>({ w: 0, h: 0 });

  // Data state
  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryRecord | null>(null);
  const [facts, setFacts] = useState<FactRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  // Filtering & Selection
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCollection, setSelectedCollection] = useState("all");
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [selectedFact, setSelectedFact] = useState<FactRecord | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const { isPanelOpen, closePanel, togglePanel } = usePanelStateContext();
  const sessionRailOpen = isPanelOpen("sessions");
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

  // Drawer & Staging mode state
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [stagingMode, setStagingMode] = useState<StagingMode>("idle");
  const [saving, setSaving] = useState(false);
  const [consolidating, setConsolidating] = useState(false);
  const [isCommitting, setIsCommitting] = useState(false);
  const [leftFlash, setLeftFlash] = useState(false);
  const [copied, setCopied] = useState(false);

  // ── Measure Container ──────────────────────────────────────────────────────
  const hasMountedRef = useRef(false);
  if (dims.w > 0) {
    hasMountedRef.current = true;
  }

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

  // ── Load Memory Data ───────────────────────────────────────────────────────
  const refresh = useCallback(async (isSilent = false) => {
    if (!isSilent) setLoading(true);
    setRefreshing(true);
    try {
      const [mem, allFacts] = await Promise.all([
        getPersonalMemory(),
        getActiveFacts(),
      ]);
      setPersonalMemory(mem);
      setFacts(allFacts);
    } catch (e) {
      console.error("[Memory] Failed to load data:", e);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Counts per category for the Legend
  const categoryCounts = useMemo(() => {
    const counts: Partial<Record<MemoryCategory, number>> = {};
    for (const f of facts) {
      const cat = f.fact_type as MemoryCategory;
      counts[cat] = (counts[cat] || 0) + 1;
    }
    return counts;
  }, [facts]);

  // Unconsolidated personal identity facts count
  const unconsolidatedIdentityCount = useMemo(() => {
    return facts.filter((f) => f.fact_type === "personal").length;
  }, [facts]);

  // ── Node & Core Click Handlers ─────────────────────────────────────────────
  const handleSelectNode = useCallback((fact: FactRecord | null, pos?: { x: number; y: number }) => {
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

  const handleSelectFactFromRail = useCallback((fact: FactRecord) => {
    setSelectedFact(fact);
    setTooltipPos({ x: window.innerWidth / 2, y: window.innerHeight / 2 });
    graphRef.current?.flyToNode(fact.id);
  }, []);


  const handleCloseSessionRail = useCallback(() => {
    setSessionRailOpen(false);
  }, []);

  const handleCoreClick = useCallback(() => {
    setStagingMode("idle");
    setDrawerOpen(true);
  }, []);

  // ── Drawer Handlers ────────────────────────────────────────────────────────
  const handleSaveStaging = useCallback(
    async (content: string) => {
      if (!personalMemory) return;
      setSaving(true);
      try {
        const updated = await savePersonalMemory(content, personalMemory.version);
        setPersonalMemory(updated);
        // Activate left card pixel reconstruction layer & right card committed indicator
        setIsCommitting(true);
        setLeftFlash(true);
        // Fade right card editor back to skeleton gently at 500ms
        setTimeout(() => {
          setStagingMode("idle");
        }, 500);
        // Conclude pixel assimilation on left card at 1200ms
        setTimeout(() => {
          setIsCommitting(false);
          setLeftFlash(false);
        }, 1200);
      } catch (e) {
        console.error("[Memory] Save failed:", e);
        setIsCommitting(false);
        setLeftFlash(false);
      } finally {
        setSaving(false);
      }
    },
    [personalMemory]
  );

  const handleConsolidateNow = useCallback(async () => {
    setConsolidating(true);
    setStagingMode("consolidating");
    try {
      const updated = await consolidatePersonalMemory();
      setPersonalMemory(updated);
      await refresh(true);
      // Seamlessly settle with left card pixel synthesis
      setIsCommitting(true);
      setLeftFlash(true);
      setTimeout(() => {
        setStagingMode("idle");
      }, 500);
      setTimeout(() => {
        setIsCommitting(false);
        setLeftFlash(false);
      }, 1200);
    } catch (e) {
      console.error("[Memory] Consolidate failed:", e);
      setStagingMode("idle");
    } finally {
      setConsolidating(false);
    }
  }, [refresh]);

  const handleCopyDoc = useCallback(() => {
    if (!personalMemory?.content) return;
    navigator.clipboard.writeText(personalMemory.content).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [personalMemory?.content]);

  const handleRecenter = useCallback(() => graphRef.current?.recenter(), []);
  const handleZoomIn = useCallback(() => graphRef.current?.zoomIn(), []);
  const handleZoomOut = useCallback(() => graphRef.current?.zoomOut(), []);
  const handleRefreshDock = useCallback(() => refresh(true), [refresh]);
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

  return (
    <div
      ref={containerRef}
      className="relative flex-1 flex flex-col h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* Sentient Liquid Space Ambient Background */}
      <AmbientBackground originX="50%" originY="50%" rippleSpeedMultiplier={1.0} paused />


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

      {/* ── Bottom Right: Category Legend Overlay (3x2 Ambient Grid) — portal to document.body so above global EdgeNav feather (z-[38]) and all dock layers ── */}
      {typeof document !== "undefined" &&
        createPortal(
          <div className="fixed bottom-4 right-6 z-[55] pointer-events-auto">
            <MemoryLegendOverlay
              selectedCollection={selectedCollection}
              onSelectCollection={setSelectedCollection}
              counts={categoryCounts}
              isLightMode={isLightMode}
            />
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
      {(hasMountedRef.current || dims.w > 0) && (
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
          />
        </ErrorBoundary>
      )}

      {/* ── Floating Fact Detail Tooltip ── */}
      {selectedFact && tooltipPos && (
        <MemoryNodeTooltip
          factDetail={selectedFact}
          pos={tooltipPos}
          onClose={() => {
            setSelectedFact(null);
            setTooltipPos(null);
          }}
        />
      )}

      {/* ── Empty State ── */}
      {!loading && facts.length === 0 && (
        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
          <div className="rounded-3xl bg-[rgba(var(--card),0.85)] border border-[rgba(var(--border),0.12)] backdrop-blur-xl p-8 max-w-sm text-center shadow-2xl">
            <Sparkles size={28} className="mx-auto text-[rgb(var(--accent))] mb-3 opacity-80" />
            <h3 className="font-display text-[14px] font-bold text-[rgb(var(--foreground))] mb-1">
              {MEMORY_COPY.emptyFactsTitle}
            </h3>
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
              {MEMORY_COPY.emptyFactsDesc}
            </p>
          </div>
        </div>
      )}

      {/* ── Bottom-Sheet Personal Memory Drawer ── */}
      <Drawer
        open={drawerOpen}
        onClose={() => {
          setDrawerOpen(false);
          setStagingMode("idle");
        }}
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
            <button
              type="button"
              onClick={handleConsolidateNow}
              disabled={consolidating || stagingMode === "consolidating"}
              className={cn(
                "flex items-center gap-2 px-3 py-1.5 rounded-xl text-[11px] font-mono border transition-all disabled:opacity-50 cursor-pointer shadow-sm",
                stagingMode === "consolidating" || consolidating
                  ? "bg-[rgba(var(--accent),0.25)] border-[rgba(var(--accent),0.5)] text-[rgb(var(--accent))]"
                  : unconsolidatedIdentityCount > 0
                  ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)]"
                  : "bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <Zap
                size={12}
                className={cn((consolidating || stagingMode === "consolidating") && "animate-pulse text-[rgb(var(--accent))]")}
              />
              <span>
                {consolidating || stagingMode === "consolidating"
                  ? MEMORY_COPY.consolidating
                  : MEMORY_COPY.consolidate}
              </span>
              {unconsolidatedIdentityCount > 0 && (
                <span className="px-1.5 py-0.2 rounded-full text-[10px] font-mono font-medium text-[rgb(var(--accent))]]">
                  {unconsolidatedIdentityCount}
                </span>
              )}
            </button>
          </div>
        }
        bodyClassName="px-4 sm:px-6 py-4 overflow-y-auto lg:overflow-hidden h-full flex flex-col min-h-0"
      >
        <ErrorBoundary name="MemoryDrawerContent">
          <div className="w-full h-full flex-1 min-h-0">
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-5 h-full min-h-0 w-full items-stretch">
              {/* Left Column: Canonical Persistent Memory (DB Ground Truth) */}
              <div
                className={cn(
                  "w-full h-full min-h-0 flex flex-col glass-card rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgba(var(--card),0.65)] backdrop-blur-xl p-5 sm:p-6 transition-all duration-500 overflow-hidden",
                  leftFlash
                    ? "ring-2 ring-[rgb(var(--accent))] shadow-[0_0_35px_rgba(var(--accent),0.35)] scale-[1.008]"
                    : "shadow-2xl"
                )}
              >
                {/* Dossier Header Bar */}
                <div className="flex items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded-xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] shadow-sm">
                      <FileText size={16} />
                    </div>
                    <div className="flex flex-col">
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))]">
                          {MEMORY_COPY.personalMemory}
                        </span>
                      </div>
                      <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                        {personalMemory
                          ? `${MEMORY_COPY.lastUpdated} ${new Date(personalMemory.updated_at).toLocaleDateString(undefined, {
                              month: "short",
                              day: "numeric",
                              year: "numeric",
                              hour: "2-digit",
                              minute: "2-digit",
                            })}`
                          : MEMORY_COPY.identityLayer}
                      </span>
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    {personalMemory && (
                      <span className="px-2.5 py-1 rounded-full text-[10px] font-mono font-medium tracking-wide bg-[rgba(var(--accent),0.10)] border border-[rgba(var(--accent),0.22)] text-[rgb(var(--accent))]">
                        {MEMORY_COPY.version} {personalMemory.version}
                      </span>
                    )}

                    <button
                      type="button"
                      onClick={handleCopyDoc}
                      disabled={!personalMemory?.content}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                      title="Copy personal memory markdown to clipboard"
                    >
                      {copied ? <Check size={12} className="text-[rgb(var(--accent))]" /> : <Copy size={12} />}
                      {copied ? MEMORY_COPY.copied : MEMORY_COPY.copy}
                    </button>
                  </div>
                </div>

                {/* Dossier Document Content with Inner Scrolling */}
                <div className="relative flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-2 pt-3 leading-relaxed max-w-none">
                  {/* Computational Pixel Reconstruction Overlay */}
                  <div
                    className={cn(
                      "absolute inset-0 z-20 rounded-xl overflow-hidden bg-[rgba(var(--card),0.92)] backdrop-blur-md transition-all duration-500 pointer-events-none flex flex-col justify-between p-6",
                      leftFlash
                        ? "opacity-100 scale-100"
                        : "opacity-0 scale-[0.98] pointer-events-none select-none invisible"
                    )}
                  >
                    <div className="absolute inset-0 z-0">
                      <PixelSynthesisCanvas active={leftFlash} />
                    </div>
                    <div className="relative z-10 flex items-center justify-between">
                      <span className="text-[11px] font-mono text-[rgb(var(--accent))] animate-pulse font-medium">
                        Rebuilding Personal Memory
                      </span>
                    </div>
                    <div className="relative z-10 text-right text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                      Updating Version
                    </div>
                  </div>

                  {personalMemory?.content ? (
                    <Markdown
                      content={personalMemory.content}
                      variant="document"
                      autoHeadings
                    />
                  ) : (
                    <p className="text-[rgb(var(--foreground-muted))] text-[13px] font-mono py-12 text-center">
                      {MEMORY_COPY.noPersonalMemory}
                    </p>
                  )}
                </div>
              </div>

              {/* Right Column: Dynamic Workspace / Staging Slate */}
              <PersonalMemoryStagingCard
                canonicalContent={personalMemory?.content ?? ""}
                mode={stagingMode}
                onModeChange={setStagingMode}
                onSave={handleSaveStaging}
                unconsolidatedCount={unconsolidatedIdentityCount}
                isSaving={saving}
                isCommitting={isCommitting}
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
