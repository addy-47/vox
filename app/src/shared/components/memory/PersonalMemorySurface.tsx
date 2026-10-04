import { Zap, Sparkles, Tag } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { UsePersonalMemoryDrawerReturn } from "@/shared/hooks/usePersonalMemoryDrawer";
import { PersonalMemoryDossierCard } from "./PersonalMemoryDossierCard";
import { PersonalMemoryStagingCard } from "./PersonalMemoryStagingCard";

interface PersonalMemorySurfaceProps {
  drawer: UsePersonalMemoryDrawerReturn;
}

interface PersonalMemoryHeaderActionsProps extends PersonalMemorySurfaceProps {
  /**
   * "wrap" — chips wrap inside the wide drawer header. "scroll" — a single
   * non-wrapping row for the modal's sticky toolbar (scrolls horizontally).
   */
  layout?: "wrap" | "scroll";
}

/**
 * Mode actions shared by both Personal Memory presentations (bottom Drawer
 * header on wide viewports, sticky toolbar on compact ones): observations
 * toggle, suggestions review entry, and the consolidate trigger.
 */
export function PersonalMemoryHeaderActions({ drawer, layout = "wrap" }: PersonalMemoryHeaderActionsProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-2",
        layout === "wrap" ? "flex-wrap" : "flex-nowrap w-max [&>*]:shrink-0"
      )}
    >
      <Tooltip label={MEMORY_COPY.viewObservationsTooltip}>
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
        <Tooltip label={MEMORY_COPY.reviewSuggestionsTooltip}>
          <button
            type="button"
            onClick={() => drawer.setStagingMode("suggestions")}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono border border-emerald-500/40 bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 transition-all cursor-pointer shadow-sm animate-pulse"
          >
            <Sparkles size={12} />
            <span>{MEMORY_COPY.reviewSuggestions}</span>
            <span className="text-[10.5px] font-mono text-emerald-400">
              ({drawer.suggestions.length})
            </span>
          </button>
        </Tooltip>
      )}

      <Tooltip label={drawer.unconsolidatedIdentityCount > 0 ? MEMORY_COPY.consolidateTooltip : MEMORY_COPY.consolidateTooltipEmpty}>
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
  );
}

interface PersonalMemoryBodyProps extends PersonalMemorySurfaceProps {
  /**
   * Compact presentations stack the dossier and staging cards in one
   * scrollable column; wide presentations keep the side-by-side grid.
   * (Tailwind `lg:` is viewport-based, so the wide grid must be opt-in —
   * a compact modal on a wide viewport would otherwise still split.)
   */
  singleColumn?: boolean;
}

/**
 * The dossier + staging body shared by both Personal Memory presentations.
 * Layout-agnostic glass cards; the sheet/modal owns the geometry.
 */
export function PersonalMemoryBody({ drawer, singleColumn = false }: PersonalMemoryBodyProps) {
  return (
    <div className="w-full h-full flex-1 min-h-0">
      <div className={cn(
        "grid gap-5 h-full min-h-0 w-full items-stretch",
        singleColumn ? "grid-cols-1" : "grid-cols-1 lg:grid-cols-2"
      )}>
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
          drawerOpen={drawer.drawerOpen}
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
  );
}
