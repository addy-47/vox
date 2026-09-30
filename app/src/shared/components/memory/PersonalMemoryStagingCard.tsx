import React, { useState, useEffect, useMemo, memo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Sparkles } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import type {
  MemoryRevisionView,
  ObservationRecord,
} from "@/services/memoryService";
import type { StagingMode, MemoryComment, PendingConfirmation } from "./stagingTypes";
import type { ObservationFilter } from "@/shared/hooks/useObservationsList";

import { StagingHeader } from "./staging/StagingHeader";
import { CommitSuccessView } from "./staging/CommitSuccessView";
import { SuggestionsReviewView } from "./staging/SuggestionsReviewView";
import { ActionHubView } from "./staging/ActionHubView";
import { CommentsQueueView } from "./staging/CommentsQueueView";
import { RawEditorView } from "./staging/RawEditorView";
import { LearnedFactsList } from "./LearnedFactsList";
import { PendingConfirmationBanner } from "./staging/PendingConfirmationBanner";
import { PixelSynthesisCanvas } from "./PixelSynthesisCanvas";

export * from "./stagingTypes";

export interface PersonalMemoryStagingCardProps {
  canonicalContent: string | null;
  canonicalMarkdown: string | null;
  activeVersion?: number;
  mode: StagingMode;
  onModeChange: (mode: StagingMode) => void;
  onSave: (content: string) => Promise<void>;
  onRegenerateWithComments?: (
    comments: MemoryComment[],
    forced?: boolean
  ) => Promise<void>;
  comments?: MemoryComment[];
  onDeleteComment?: (id: string) => void;
  onUpdateComment?: (id: string, text: string) => void;
  onClearComments?: () => void;
  unconsolidatedCount: number;
  isSaving: boolean;
  isConsolidating?: boolean;
  isCommitting: boolean;
  suggestions?: MemoryRevisionView[];
  onApplySuggestions?: (
    decisions: Record<string, "accept" | "reject">
  ) => Promise<void>;
  isApplyingSuggestions?: boolean;
  candidateFacts?: ObservationRecord[];
  justCommitted?: boolean;
  onViewVersionHistory?: () => void;
  // Observation pagination props
  observations?: ObservationRecord[];
  observationFilter?: ObservationFilter;
  onObservationFilterChange?: (filter: ObservationFilter) => void;
  isLoadingObservations?: boolean;
  isLoadingMoreObservations?: boolean;
  hasMoreObservations?: boolean;
  onLoadMoreObservations?: () => void;
  // Pending confirmation props
  pendingConfirmation?: PendingConfirmation | null;
  onConfirmPendingIntegration?: () => void;
  onCancelPendingConfirmation?: () => void;
}

function parseOpPayload(content?: string): {
  text?: string;
  section_id?: string;
  block_id?: string;
  title?: string;
  blocks?: Array<{ id?: string; text?: string }>;
} {
  if (!content) return {};
  try {
    return JSON.parse(content);
  } catch {
    return { text: content };
  }
}

function parseSections(
  canonicalContent: string | null,
  canonicalMarkdown: string | null
): Array<{ id?: string; title: string; blocks: Array<{ id?: string; text: string }> }> {
  if (canonicalContent) {
    try {
      const parsed = JSON.parse(canonicalContent);
      if (Array.isArray(parsed?.sections)) {
        return parsed.sections.map((s: { id?: string; title?: string; blocks?: Array<{ id?: string; text?: string }> }) => ({
          id: s.id,
          title: s.title || "Untitled",
          blocks: (s.blocks || []).map((b) => ({
            id: b.id,
            text: b.text || "",
          })),
        }));
      }
    } catch {
      // Fall through to markdown
    }
  }

  const raw = canonicalMarkdown || "";
  const lines = raw.split("\n");
  const sections: Array<{ id?: string; title: string; blocks: Array<{ id?: string; text: string }> }> = [];
  let currentSec: { id?: string; title: string; blocks: Array<{ id?: string; text: string }> } | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith("## ")) {
      if (currentSec) sections.push(currentSec);
      currentSec = { title: trimmed.slice(3).trim(), blocks: [] };
    } else if (trimmed.length > 0) {
      if (!currentSec) {
        currentSec = { title: "Overview", blocks: [] };
      }
      currentSec.blocks.push({ text: trimmed });
    }
  }
  if (currentSec) sections.push(currentSec);
  return sections;
}

export const PersonalMemoryStagingCard: React.FC<PersonalMemoryStagingCardProps> = memo(
  ({
    canonicalContent,
    canonicalMarkdown,
    activeVersion,
    mode,
    onModeChange,
    onSave,
    onRegenerateWithComments,
    comments = [],
    onDeleteComment,
    onUpdateComment,
    onClearComments,
    isSaving,
    isConsolidating = false,
    isCommitting,
    suggestions = [],
    onApplySuggestions,
    isApplyingSuggestions,
    candidateFacts = [],
    justCommitted = false,
    onViewVersionHistory,
    observations = [],
    observationFilter = "staged",
    onObservationFilterChange,
    isLoadingObservations = false,
    isLoadingMoreObservations = false,
    hasMoreObservations = false,
    onLoadMoreObservations,
    pendingConfirmation = null,
    onConfirmPendingIntegration,
    onCancelPendingConfirmation,
  }) => {
    const [draft, setDraft] = useState("");
    const [decisions, setDecisions] = useState<Record<string, "accept" | "reject">>({});
    const [failedRevisionIds, setFailedRevisionIds] = useState<Set<string>>(new Set());

    useEffect(() => {
      if (mode === "edit") {
        setDraft(canonicalMarkdown || "");
      } else if (mode === "import") {
        setDraft("");
      }
    }, [mode, canonicalMarkdown, canonicalContent]);

    const baseSections = useMemo(
      () => parseSections(canonicalContent, canonicalMarkdown),
      [canonicalContent, canonicalMarkdown]
    );

    useEffect(() => {
      setFailedRevisionIds(new Set());
    }, [suggestions]);

    const decisionStats = useMemo(() => {
      let accepted = 0;
      let rejected = 0;
      for (const sug of suggestions) {
        const d = decisions[sug.id];
        if (d === "accept") accepted++;
        else if (d === "reject") rejected++;
      }
      const pending = suggestions.length - accepted - rejected;
      return { accepted, rejected, pending, total: suggestions.length };
    }, [suggestions, decisions]);

    const handleSelectDecision = (id: string, action: "accept" | "reject") => {
      setDecisions((prev) => {
        if (prev[id] === action) {
          const next = { ...prev };
          delete next[id];
          return next;
        }
        const next = { ...prev, [id]: action };
        const allDecided =
          suggestions.length > 0 &&
          suggestions.every((s) => next[s.id] === "accept" || next[s.id] === "reject");

        if (allDecided && onApplySuggestions) {
          setTimeout(() => {
            void onApplySuggestions(next)
              .then(() => {
                setDecisions({});
                setFailedRevisionIds(new Set());
              })
              .catch((e) => {
                console.error("[PersonalMemoryStagingCard] Auto-finalising suggestions failed:", e);
                setFailedRevisionIds(new Set(Object.keys(next)));
              });
          }, 200);
        }

        return next;
      });
    };

    const handleAcceptAll = async () => {
      const all: Record<string, "accept" | "reject"> = {};
      for (const s of suggestions) {
        all[s.id] = "accept";
      }
      setDecisions(all);
      if (onApplySuggestions && suggestions.length > 0) {
        try {
          await onApplySuggestions(all);
          setDecisions({});
          setFailedRevisionIds(new Set());
        } catch (e) {
          console.error("[PersonalMemoryStagingCard] Accept all failed:", e);
          setFailedRevisionIds(new Set(Object.keys(all)));
        }
      }
    };

    const handleRejectAll = async () => {
      const all: Record<string, "accept" | "reject"> = {};
      for (const s of suggestions) {
        all[s.id] = "reject";
      }
      setDecisions(all);
      if (onApplySuggestions && suggestions.length > 0) {
        try {
          await onApplySuggestions(all);
          setDecisions({});
          setFailedRevisionIds(new Set());
        } catch (e) {
          console.error("[PersonalMemoryStagingCard] Reject all failed:", e);
          setFailedRevisionIds(new Set(Object.keys(all)));
        }
      }
    };

    const handleApplySelectedDecisions = async () => {
      if (!onApplySuggestions || suggestions.length === 0) return;

      const selected: Record<string, "accept" | "reject"> = {};
      for (const sug of suggestions) {
        const decision = decisions[sug.id];
        if (decision === "accept" || decision === "reject") {
          selected[sug.id] = decision;
        }
      }
      if (Object.keys(selected).length === 0) return;

      try {
        await onApplySuggestions(selected);
        setDecisions((prev) => {
          const next = { ...prev };
          for (const id of Object.keys(selected)) delete next[id];
          return next;
        });
        setFailedRevisionIds(new Set());
      } catch (e) {
        console.error("[PersonalMemoryStagingCard] Applying decisions failed:", e);
        setFailedRevisionIds(new Set(Object.keys(selected)));
      }
    };

    const handleRetryFailed = () => {
      if (failedRevisionIds.size === 0) return;
      void handleApplySelectedDecisions();
    };

    const handleCommit = async () => {
      if (!draft.trim()) return;
      await onSave(draft);
    };

    const handleRegenerate = async (forced?: boolean) => {
      if (!onRegenerateWithComments || comments.length === 0) return;
      try {
        await onRegenerateWithComments(comments, forced);
      } catch (e) {
        console.error("[PersonalMemoryStagingCard] Regenerate failed:", e);
      }
    };

    const isSuggestionsActive = mode === "suggestions" && suggestions.length > 0;

    return (
      <div
        className={cn(
          "relative w-full h-full min-h-0 rounded-2xl p-5 sm:p-6 flex flex-col transition-all duration-500 overflow-hidden",
          "glass-card border bg-[rgba(var(--card),0.45)] backdrop-blur-sm contain-paint transform-gpu",
          isSuggestionsActive
            ? "border-[rgba(var(--accent),0.35)] shadow-xl"
            : "border-[rgba(var(--accent),0.18)] hover:border-[rgba(var(--accent),0.35)] shadow-2xl",
          (isSaving || isCommitting || isApplyingSuggestions) &&
            "opacity-50 pointer-events-none select-none"
        )}
      >
        {/* Organic Synthesis Overlay on Right Card while consolidating/integrating */}
        <AnimatePresence>
          {isConsolidating && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.35, ease: "easeInOut" }}
              className="absolute inset-0 z-40 rounded-2xl overflow-hidden bg-[rgba(var(--card),0.85)] backdrop-blur-md flex flex-col items-center justify-center pointer-events-auto"
            >
              <PixelSynthesisCanvas active={isConsolidating} />
              <div className="relative z-10 flex flex-col items-center text-center p-6">
                <div className="w-10 h-10 rounded-xl bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] mb-3 shadow-lg animate-pulse">
                  <Sparkles size={20} />
                </div>
                <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
                  Synthesizing Profile Updates…
                </h4>
                <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
                  Analyzing observations against your personal memory structure.
                </p>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
        <StagingHeader
          mode={mode}
          onModeChange={onModeChange}
          justCommitted={justCommitted}
          isSuggestionsActive={isSuggestionsActive}
          decisionStats={decisionStats}
          commentsCount={comments.length}
          isApplyingSuggestions={isApplyingSuggestions}
          isSaving={isSaving}
          draftEmpty={!draft.trim()}
          onAcceptAll={handleAcceptAll}
          onRejectAll={handleRejectAll}
          onRegenerate={() => handleRegenerate()}
          onClearComments={onClearComments}
          onCommit={handleCommit}
          observationFilter={observationFilter}
          onObservationFilterChange={onObservationFilterChange}
          observationCount={(observations.length > 0 ? observations : candidateFacts).length}
        />

        {pendingConfirmation && (
          <PendingConfirmationBanner
            confirmation={pendingConfirmation}
            isConsolidating={isConsolidating}
            onConfirm={onConfirmPendingIntegration ?? (() => {})}
            onDismiss={onCancelPendingConfirmation ?? (() => {})}
          />
        )}

        {justCommitted ? (
          <CommitSuccessView
            activeVersion={activeVersion}
            onViewVersionHistory={onViewVersionHistory}
          />
        ) : isSuggestionsActive ? (
          <SuggestionsReviewView
            baseSections={baseSections}
            revisions={suggestions}
            decisions={decisions}
            decidedCount={decisionStats.accepted + decisionStats.rejected}
            undecidedCount={decisionStats.pending}
            isApplying={Boolean(isApplyingSuggestions)}
            actionsDisabled={Boolean(isApplyingSuggestions)}
            failedRevisionIds={failedRevisionIds}
            onRetryFailed={handleRetryFailed}
            onApplyDecisions={handleApplySelectedDecisions}
            onSelectDecision={handleSelectDecision}
            parseOpPayload={parseOpPayload}
          />
        ) : mode === "idle" ? (
          <ActionHubView
            commentsCount={comments.length}
            onModeChange={onModeChange}
          />
        ) : mode === "comment" ? (
          <CommentsQueueView
            comments={comments}
            onDeleteComment={onDeleteComment}
            onUpdateComment={onUpdateComment}
            onClearComments={onClearComments}
          />
        ) : mode === "import" || mode === "edit" ? (
          <RawEditorView
            mode={mode}
            draft={draft}
            isCommitting={isCommitting}
            onDraftChange={setDraft}
          />
        ) : mode === "facts" ? (
          <div className="flex-1 min-h-0 flex flex-col pt-3 transition-opacity duration-500">
            <LearnedFactsList
              observations={observations.length > 0 ? observations : candidateFacts}
              statusFilter={observationFilter}
              onStatusFilterChange={onObservationFilterChange ?? (() => {})}
              isLoading={isLoadingObservations}
              isLoadingMore={isLoadingMoreObservations}
              hasMore={hasMoreObservations}
              onLoadMore={onLoadMoreObservations}
              isConsolidating={isConsolidating}
            />
          </div>
        ) : null}
      </div>
    );
  }
);

PersonalMemoryStagingCard.displayName = "PersonalMemoryStagingCard";
