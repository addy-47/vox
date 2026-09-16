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
  Edit3,
  Download,
  Upload,
  Zap,
  Sparkles,
  PanelLeft,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import {
  getPersonalMemory,
  savePersonalMemory,
  consolidatePersonalMemory,
  exportPersonalMemory,
  importPersonalMemory,
  getActiveFacts,
  type PersonalMemoryRecord,
  type FactRecord,
} from "@/services/memoryService";
import { AmbientBackground, ErrorBoundary } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { EdgePanel, Tooltip } from "@/shared/ui";
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

  // Drawer & Edit mode state
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draftContent, setDraftContent] = useState("");
  const [saving, setSaving] = useState(false);
  const [consolidating, setConsolidating] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);

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
    setEditing(false);
    setDrawerOpen(true);
  }, []);

  // ── Drawer Handlers ────────────────────────────────────────────────────────
  const handleStartEdit = useCallback(() => {
    setDraftContent(personalMemory?.content ?? "");
    setEditing(true);
  }, [personalMemory]);

  const handleCancelEdit = useCallback(() => {
    setEditing(false);
  }, []);

  const handleSaveMemory = useCallback(async () => {
    if (!personalMemory) return;
    setSaving(true);
    try {
      const updated = await savePersonalMemory(draftContent, personalMemory.version);
      setPersonalMemory(updated);
      setEditing(false);
    } catch (e) {
      console.error("[Memory] Save failed:", e);
    } finally {
      setSaving(false);
    }
  }, [personalMemory, draftContent]);

  const handleConsolidateNow = useCallback(async () => {
    setConsolidating(true);
    try {
      const updated = await consolidatePersonalMemory();
      setPersonalMemory(updated);
      await refresh(true);
    } catch (e) {
      console.error("[Memory] Consolidate failed:", e);
    } finally {
      setConsolidating(false);
    }
  }, [refresh]);

  const handleExportDoc = useCallback(async () => {
    try {
      await exportPersonalMemory("~/personal_memory.md");
      setExportMessage("Exported to ~/personal_memory.md");
      setTimeout(() => setExportMessage(null), 3500);
    } catch (e) {
      console.error("[Memory] Export failed:", e);
      setExportMessage("Export failed.");
      setTimeout(() => setExportMessage(null), 3500);
    }
  }, []);

  const handleImportDoc = useCallback(async () => {
    try {
      const updated = await importPersonalMemory("~/personal_memory.md");
      setPersonalMemory(updated);
      setExportMessage("Imported successfully.");
      setTimeout(() => setExportMessage(null), 3500);
    } catch (e) {
      console.error("[Memory] Import failed:", e);
      setExportMessage("Import failed.");
      setTimeout(() => setExportMessage(null), 3500);
    }
  }, []);

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
          setEditing(false);
        }}
        position="global"
        ariaLabel={MEMORY_COPY.personalMemory}
        height={65}
        title={
          <div className="flex items-center gap-2.5">
            <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
              {MEMORY_COPY.personalMemory}
            </span>
            {personalMemory && (
              <span className="px-2 py-0.5 rounded-full text-[10px] font-mono bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] font-medium">
                {`v${personalMemory.version}`}
              </span>
            )}
          </div>
        }
        subtitle={
          <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
            {MEMORY_COPY.drawerSubtitle}
          </span>
        }
        headerActions={
          <div className="flex items-center gap-2 flex-wrap">
            {exportMessage && (
              <span className="text-[11px] font-mono text-[rgb(var(--accent))] px-2 py-1 rounded bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)]">
                {exportMessage}
              </span>
            )}

            {!editing ? (
              <button
                type="button"
                onClick={handleStartEdit}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)] transition-colors cursor-pointer"
              >
                <Edit3 size={12} /> {MEMORY_COPY.edit}
              </button>
            ) : (
              <>
                <button
                  type="button"
                  onClick={handleSaveMemory}
                  disabled={saving}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-50 cursor-pointer"
                >
                  {saving ? MEMORY_COPY.saving : MEMORY_COPY.save}
                </button>
                <button
                  type="button"
                  onClick={handleCancelEdit}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  {MEMORY_COPY.cancel}
                </button>
              </>
            )}

            <button
              type="button"
              onClick={handleConsolidateNow}
              disabled={consolidating}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors disabled:opacity-50 cursor-pointer"
            >
              <Zap size={12} className={cn(consolidating && "animate-pulse text-[rgb(var(--accent))]")} />
              {consolidating ? MEMORY_COPY.consolidating : MEMORY_COPY.consolidate}
            </button>

            <button
              type="button"
              onClick={handleExportDoc}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
            >
              <Download size={12} /> {MEMORY_COPY.export}
            </button>

            <button
              type="button"
              onClick={handleImportDoc}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
            >
              <Upload size={12} /> {MEMORY_COPY.import}
            </button>
          </div>
        }
        bodyClassName="px-8 py-5 max-h-[60vh] overflow-y-auto"
      >
        <ErrorBoundary name="MemoryDrawerContent">
          {editing ? (
            <div className="flex flex-col gap-2">
              <textarea
                value={draftContent}
                onChange={(e) => setDraftContent(e.target.value)}
                className="w-full h-[360px] bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.14)] rounded-2xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.45)] transition-colors"
                spellCheck={false}
              />
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                Tip: Supports GitHub Flavored Markdown headers, lists, and code blocks.
              </span>
            </div>
          ) : (
            <div className="prose dark:prose-invert prose-sm max-w-none text-[rgb(var(--foreground))] leading-relaxed select-text">
              {personalMemory?.content ? (
                <ReactMarkdown>{personalMemory.content}</ReactMarkdown>
              ) : (
                <p className="text-[rgb(var(--foreground-muted))] text-[13px] font-mono py-8 text-center">
                  {MEMORY_COPY.noPersonalMemory}
                </p>
              )}
            </div>
          )}

          {/* Document Provenance Metadata Footer */}
          {personalMemory && (
            <div className="mt-8 pt-3 border-t border-[rgba(var(--border),0.10)] flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
              <span>
                {MEMORY_COPY.version} {personalMemory.version}
              </span>
              <span>
                {MEMORY_COPY.lastUpdated}: {new Date(personalMemory.updated_at).toLocaleString()}
              </span>
            </div>
          )}
        </ErrorBoundary>
      </Drawer>

    </div>
  );
});

Memory.displayName = "Memory";
export default Memory;
