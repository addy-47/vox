import React, { memo, useState, useEffect, useRef } from "react";
import {
  FileText,
  Copy,
  Check,
  RotateCw,
  Zap,
  MessageSquare,
  Tag,
  Layers,
  Sparkles,
  X,
  Filter,
  ChevronDown,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import type { UsePersonalMemoryDrawerReturn } from "@/shared/hooks/usePersonalMemoryDrawer";
import { PersonalMemoryDossierCard } from "./PersonalMemoryDossierCard";
import { PersonalMemoryStagingCard } from "./PersonalMemoryStagingCard";
import { LearnedFactsList } from "./LearnedFactsList";

export type PersonalMemoryModalTab = "memory" | "observations" | "staging";

export interface PersonalMemoryModalViewProps {
  drawer: UsePersonalMemoryDrawerReturn;
  onClose: () => void;
}

/**
 * Dedicated single-card 3-tab layout for Personal Memory inside Modal (compact viewports < 1024px).
 * Features an integrated action header, unified tabs, and instant auto-redirection on state changes.
 */
export const PersonalMemoryModalView: React.FC<PersonalMemoryModalViewProps> = memo(
  ({ drawer, onClose }) => {
    const [activeTab, setActiveTab] = useState<PersonalMemoryModalTab>("memory");
    const [isFilterOpen, setIsFilterOpen] = useState(false);
    const filterMenuRef = useRef<HTMLDivElement>(null);

    // Track state mutations to auto-redirect back to the Memory tab for visual feedback
    const prevVeilCycleRef = useRef(drawer.veilCycle);
    const prevLeftFlashRef = useRef(drawer.leftFlash);
    const prevJustCommittedRef = useRef(drawer.justCommitted);

    // Auto-redirect: If left card flashes or veil cycle triggers (e.g. from restore or suggestion accept)
    useEffect(() => {
      if (drawer.leftFlash && !prevLeftFlashRef.current) {
        setActiveTab("memory");
      }
      prevLeftFlashRef.current = drawer.leftFlash;
    }, [drawer.leftFlash]);

    useEffect(() => {
      if (drawer.veilCycle > prevVeilCycleRef.current) {
        setActiveTab("memory");
      }
      prevVeilCycleRef.current = drawer.veilCycle;
    }, [drawer.veilCycle]);

    useEffect(() => {
      if (drawer.justCommitted && !prevJustCommittedRef.current) {
        setActiveTab("memory");
      }
      prevJustCommittedRef.current = drawer.justCommitted;
    }, [drawer.justCommitted]);

    // Handle outside clicks for status filter dropdown
    useEffect(() => {
      if (!isFilterOpen) return;
      const handleClickOutside = (e: MouseEvent) => {
        if (filterMenuRef.current && !filterMenuRef.current.contains(e.target as Node)) {
          setIsFilterOpen(false);
        }
      };
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [isFilterOpen]);

    // Handle comment selection in Dossier -> Redirect to Staging in comment mode
    const handleCommentFromDossier = () => {
      setActiveTab("staging");
      drawer.setStagingMode("comment");
    };

    const hasPendingStaging =
      drawer.suggestions.length > 0 ||
      drawer.comments.length > 0 ||
      Boolean(drawer.pendingConfirmation);

    return (
      <div className="flex flex-col h-full w-full min-h-0 select-none">
        {/* ── Unified Masthead: Title, Actions & Integrated Underline Tabs ── */}
        <div className="flex flex-col shrink-0 px-6 pt-5 pb-0 border-b border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.35)] select-none">
          {/* Top Row: Title & Global Actions */}
          <div className="flex items-center justify-between gap-3 pb-4 min-h-[40px]">
            {/* Left: Sparkles icon + Title */}
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-8 h-8 rounded-xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] shadow-sm shrink-0">
                <Sparkles size={16} />
              </div>
              <h2 className="text-[14px] font-semibold tracking-wide text-[rgb(var(--foreground))] truncate">
                {MEMORY_COPY.personalMemory}
              </h2>
            </div>

            {/* Right: Global Actions Cluster */}
            <div className="flex items-center gap-1.5 shrink-0">
              {/* Copy Button */}
              <Tooltip label={drawer.copied ? MEMORY_COPY.copied : MEMORY_COPY.copyDocTitle}>
                <button
                  type="button"
                  onClick={drawer.handleCopyDoc}
                  disabled={!drawer.displayedRecord?.markdown && !drawer.displayedRecord?.content}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono border border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] transition-all cursor-pointer shadow-xs disabled:opacity-40"
                  aria-label={MEMORY_COPY.copyDocTitle}
                >
                  {drawer.copied ? <Check size={13} className="text-emerald-400" /> : <Copy size={13} />}
                  <span className="hidden sm:inline">{drawer.copied ? MEMORY_COPY.copied : MEMORY_COPY.copy}</span>
                </button>
              </Tooltip>

              {/* Regenerate from Facts Button */}
              <Tooltip label={drawer.isRegenerating ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerateTooltip}>
                <button
                  type="button"
                  onClick={drawer.handleRegenerateFromFacts}
                  disabled={drawer.isRegenerating || drawer.saving}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono border border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] transition-all cursor-pointer shadow-xs disabled:opacity-40"
                  aria-label={MEMORY_COPY.regenerateTooltip}
                >
                  <RotateCw
                    size={13}
                    className={cn(drawer.isRegenerating && "animate-spin text-[rgb(var(--accent))]")}
                  />
                  <span className="hidden sm:inline">
                    {drawer.isRegenerating ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerate}
                  </span>
                </button>
              </Tooltip>

              {/* Consolidate / Integrate Button */}
              <Tooltip
                label={
                  drawer.unconsolidatedIdentityCount > 0
                    ? MEMORY_COPY.consolidateTooltip
                    : MEMORY_COPY.consolidateTooltipEmpty
                }
              >
                <button
                  type="button"
                  onClick={() => drawer.handleConsolidateNow(false)}
                  disabled={drawer.consolidating || drawer.unconsolidatedIdentityCount === 0}
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono border transition-all shadow-xs disabled:opacity-40 disabled:cursor-not-allowed",
                    drawer.consolidating
                      ? "bg-[rgba(var(--accent),0.25)] border-[rgba(var(--accent),0.5)] text-[rgb(var(--accent))]"
                      : drawer.unconsolidatedIdentityCount > 0
                      ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] cursor-pointer"
                      : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground-muted))]/40"
                  )}
                  aria-label={MEMORY_COPY.consolidate}
                >
                  <Zap
                    size={13}
                    className={cn(drawer.consolidating && "animate-pulse text-[rgb(var(--accent))]")}
                  />
                  <span className="hidden sm:inline">
                    {drawer.consolidating ? MEMORY_COPY.consolidating : MEMORY_COPY.consolidate}
                  </span>
                  {drawer.unconsolidatedIdentityCount > 0 && (
                    <span className="text-[10px] font-mono font-bold">
                      ({drawer.unconsolidatedIdentityCount})
                    </span>
                  )}
                </button>
              </Tooltip>

              {/* Comments Counter (Redirects to Staging tab in comment mode) */}
              {drawer.comments.length > 0 && (
                <Tooltip label={MEMORY_COPY.commentsQueued(drawer.comments.length)}>
                  <button
                    type="button"
                    onClick={handleCommentFromDossier}
                    className="flex items-center gap-1 px-2.5 py-1.5 rounded-xl text-[11px] font-mono border border-[rgba(var(--accent),0.3)] bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)] transition-all cursor-pointer shadow-xs"
                  >
                    <MessageSquare size={13} />
                    <span>{drawer.comments.length}</span>
                  </button>
                </Tooltip>
              )}

              {/* Modal Close Button */}
              <Tooltip label={LAYOUT_COPY.modal.close}>
                <button
                  type="button"
                  onClick={onClose}
                  className="flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer ml-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                  aria-label={LAYOUT_COPY.modal.close}
                >
                  <X size={16} />
                </button>
              </Tooltip>
            </div>
          </div>

          {/* Bottom Row: Underline Tabs with generous breathing room */}
          <div
            data-arrow-nav
            role="tablist"
            aria-label="Personal Memory Tabs"
            className="w-full flex items-center justify-between pt-2 pb-0 select-none overflow-x-auto no-scrollbar -mb-[1px]"
          >
            {/* Memory Tab */}
            <div className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "memory"}
                onClick={() => setActiveTab("memory")}
                className={cn(
                  "w-full flex items-center justify-center gap-2 pb-2.5 border-b-2 transition-all duration-200 bg-transparent text-[11.5px] font-mono font-bold uppercase tracking-[0.06em] outline-none cursor-pointer text-center truncate px-2",
                  activeTab === "memory"
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <FileText size={13} className="shrink-0" />
                <span className="truncate">{MEMORY_COPY.tabMemory}</span>
              </button>
              <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-2.5 shrink-0 px-1 sm:px-2">
                |
              </span>
            </div>

            {/* Observations Tab */}
            <div className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "observations"}
                onClick={() => setActiveTab("observations")}
                className={cn(
                  "w-full flex items-center justify-center gap-2 pb-2.5 border-b-2 transition-all duration-200 bg-transparent text-[11.5px] font-mono font-bold uppercase tracking-[0.06em] outline-none cursor-pointer text-center truncate px-2",
                  activeTab === "observations"
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <Tag size={13} className="shrink-0" />
                <span className="truncate">{MEMORY_COPY.tabObservations}</span>
                {drawer.unconsolidatedIdentityCount > 0 && (
                  <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))] shrink-0">
                    ({drawer.unconsolidatedIdentityCount})
                  </span>
                )}
              </button>
              <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-2.5 shrink-0 px-1 sm:px-2">
                |
              </span>
            </div>

            {/* Staging Tab */}
            <div className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "staging"}
                onClick={() => setActiveTab("staging")}
                className={cn(
                  "w-full flex items-center justify-center gap-2 pb-2.5 border-b-2 transition-all duration-200 bg-transparent text-[11.5px] font-mono font-bold uppercase tracking-[0.06em] outline-none cursor-pointer text-center truncate px-2 relative",
                  activeTab === "staging"
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <Layers size={13} className="shrink-0" />
                <span className="truncate">{MEMORY_COPY.tabStaging}</span>
                {hasPendingStaging && (
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_#34d399] animate-pulse shrink-0" />
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ── Single Full-Height Unified Workspace (One Contiguous Entity) ── */}
        <div className="flex-1 min-h-0 px-6 pb-6 pt-5 overflow-hidden flex flex-col">
          {activeTab === "memory" && (
            <div className="h-full w-full min-h-0 flex flex-col animate-in fade-in duration-200">
              <PersonalMemoryDossierCard
                title={MEMORY_COPY.dossierTitle}
                embedded={true}
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
                onSelectComment={handleCommentFromDossier}
                drawerOpen={drawer.drawerOpen}
                drawerBodyReady={drawer.drawerBodyReady}
                hideActions={true}
              />
            </div>
          )}

          {activeTab === "observations" && (
            <div className="h-full w-full min-h-0 flex flex-col animate-in fade-in duration-200">
              {/* Observations Header Bar: Matches Dossier & Staging parity */}
              <div className="flex items-center justify-between gap-3 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
                <div className="flex items-center gap-3 min-w-0 flex-1">
                  <div className="w-8 h-8 rounded-xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] shadow-sm shrink-0">
                    <Tag size={16} />
                  </div>
                  <div className="flex flex-col min-w-0">
                    <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))] truncate">
                      {MEMORY_COPY.learnedFactsTitle}
                    </span>
                    <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate">
                      {drawer.paginatedObservations.length > 0
                        ? MEMORY_COPY.observationsDesc(drawer.paginatedObservations.length)
                        : MEMORY_COPY.observationsDescBare}
                    </span>
                  </div>
                </div>

                {/* Filter dropdown matching StagingHeader */}
                <div className="relative shrink-0" ref={filterMenuRef}>
                  <button
                    type="button"
                    onClick={() => setIsFilterOpen((prev) => !prev)}
                    className={cn(
                      "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-mono border transition-colors duration-150 cursor-pointer",
                      isFilterOpen || (drawer.obsStatusFilter && drawer.obsStatusFilter !== "all")
                        ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]"
                        : "border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)]"
                    )}
                    title={MEMORY_COPY.filterObservations}
                    aria-expanded={isFilterOpen}
                  >
                    <Filter
                      size={11}
                      className={
                        drawer.obsStatusFilter && drawer.obsStatusFilter !== "all"
                          ? "text-[rgb(var(--accent))]"
                          : undefined
                      }
                    />
                    <span className="capitalize">
                      {drawer.obsStatusFilter === "staged"
                        ? MEMORY_COPY.observationFilterStaged
                        : drawer.obsStatusFilter === "pending"
                        ? MEMORY_COPY.observationFilterPending
                        : drawer.obsStatusFilter === "integrated"
                        ? MEMORY_COPY.observationFilterIntegrated
                        : MEMORY_COPY.observationFilterAll}
                    </span>
                    <ChevronDown
                      size={11}
                      className={cn("transition-transform duration-150 opacity-70", isFilterOpen && "rotate-180")}
                    />
                  </button>

                  {isFilterOpen && (
                    <div className="absolute right-0 top-full mt-1.5 w-36 py-1 rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--border),0.2)] shadow-xl z-50 backdrop-blur-md">
                      {(
                        [
                          { id: "staged", label: MEMORY_COPY.observationFilterStaged },
                          { id: "pending", label: MEMORY_COPY.observationFilterPending },
                          { id: "integrated", label: MEMORY_COPY.observationFilterIntegrated },
                          { id: "all", label: MEMORY_COPY.observationFilterAll },
                        ] as const
                      ).map((item) => (
                        <button
                          key={item.id}
                          type="button"
                          onClick={() => {
                            drawer.setObsStatusFilter(item.id);
                            setIsFilterOpen(false);
                          }}
                          className={cn(
                            "w-full px-3 py-1.5 text-left text-[11px] font-mono flex items-center justify-between transition-colors cursor-pointer",
                            drawer.obsStatusFilter === item.id
                              ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium"
                              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
                          )}
                        >
                          <span>{item.label}</span>
                          {drawer.obsStatusFilter === item.id && (
                            <Check size={11} className="text-[rgb(var(--accent))]" />
                          )}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              {/* Body */}
              <div className="flex-1 min-h-0 pt-4 flex flex-col overflow-hidden">
                <LearnedFactsList
                  observations={drawer.paginatedObservations}
                  statusFilter={drawer.obsStatusFilter}
                  onStatusFilterChange={drawer.setObsStatusFilter}
                  isLoading={drawer.obsLoading}
                  isLoadingMore={drawer.obsLoadingMore}
                  hasMore={drawer.obsHasMore}
                  onLoadMore={drawer.obsLoadMore}
                  isConsolidating={drawer.consolidating}
                />
              </div>
            </div>
          )}

          {activeTab === "staging" && (
            <div className="h-full w-full min-h-0 flex flex-col animate-in fade-in duration-200">
              <PersonalMemoryStagingCard
                embedded={true}
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
                  setActiveTab("memory");
                }}
              />
            </div>
          )}
        </div>
      </div>
    );
  }
);

PersonalMemoryModalView.displayName = "PersonalMemoryModalView";
