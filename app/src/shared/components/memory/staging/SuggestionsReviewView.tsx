import React, { memo } from "react";
import { Check, X, RotateCw, TriangleAlert } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import {
  buildReviewDocument,
  type ParsedSectionItem,
  type ReviewEntry,
} from "./reviewModel";
import type { MemoryRevisionView } from "@/services/memoryService";

export type { ParsedSectionItem } from "./reviewModel";

export interface SuggestionsReviewViewProps {
  baseSections: ParsedSectionItem[];
  revisions: MemoryRevisionView[];
  decisions: Record<string, "accept" | "reject">;
  failedRevisionIds?: Set<string>;
  decidedCount: number;
  undecidedCount: number;
  onApplyDecisions: () => void;
  isApplying: boolean;
  onSelectDecision: (id: string, action: "accept" | "reject") => void;
  onRetryFailed?: () => void;
  actionsDisabled?: boolean;
  parseOpPayload: (content?: string) => {
    text?: string;
    section_id?: string;
    block_id?: string;
    title?: string;
    blocks?: Array<{ id?: string; text?: string }>;
  };
}

interface DecisionButtonsProps {
  revisionId: string;
  decisions: Record<string, "accept" | "reject">;
  onSelectDecision: (id: string, action: "accept" | "reject") => void;
  acceptTitle: string;
  rejectTitle: string;
  disabled?: boolean;
  failed?: boolean;
}

const DecisionButtons = memo(function DecisionButtons({
  revisionId,
  decisions,
  onSelectDecision,
  acceptTitle,
  rejectTitle,
  disabled,
  failed,
}: DecisionButtonsProps) {
  const decision = decisions[revisionId];
  return (
    <div className="flex items-center gap-1 shrink-0">
      <button
        type="button"
        onClick={() => onSelectDecision(revisionId, "accept")}
        disabled={disabled}
        aria-pressed={decision === "accept"}
        aria-label={acceptTitle}
        title={acceptTitle}
        className={cn(
          "w-5 h-5 rounded flex items-center justify-center transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed",
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[rgb(var(--accent))]",
          decision === "accept"
            ? "bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.4)]"
            : "border border-[rgba(var(--border),0.2)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.08)]"
        )}
      >
        <Check size={11} strokeWidth={2.5} />
      </button>
      <button
        type="button"
        onClick={() => onSelectDecision(revisionId, "reject")}
        disabled={disabled}
        aria-pressed={decision === "reject"}
        aria-label={rejectTitle}
        title={rejectTitle}
        className={cn(
          "w-5 h-5 rounded flex items-center justify-center transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed",
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[rgb(var(--accent))]",
          decision === "reject"
            ? "bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.3)]"
            : "border border-[rgba(var(--border),0.2)] text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--border),0.4)] hover:bg-[rgba(var(--foreground),0.06)]"
        )}
      >
        <X size={11} strokeWidth={2.5} />
      </button>
      {failed && (
        <TriangleAlert
          size={11}
          className="ml-0.5 text-[rgb(var(--foreground-muted))] shrink-0"
          aria-label={MEMORY_COPY.applyFailed}
        />
      )}
    </div>
  );
});

export const SuggestionsReviewView: React.FC<SuggestionsReviewViewProps> = memo(
  ({
    baseSections,
    revisions,
    decisions,
    failedRevisionIds,
    decidedCount,
    undecidedCount,
    onApplyDecisions,
    isApplying,
    onSelectDecision,
    onRetryFailed,
    actionsDisabled,
    parseOpPayload,
  }) => {
    const doc = React.useMemo(
      () => buildReviewDocument({ baseSections, revisions, parseOpPayload }),
      [baseSections, revisions, parseOpPayload]
    );

    const anyFailed = Boolean(failedRevisionIds && failedRevisionIds.size > 0);

    const renderEntry = (entry: ReviewEntry) => {
      if (entry.kind === "unchanged") {
        return (
          <p
            key={entry.key}
            className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground))]/80 select-text font-sans"
          >
            {entry.text}
          </p>
        );
      }

      const decision = decisions[entry.revisionId];
      const failed = Boolean(failedRevisionIds?.has(entry.revisionId));

      const buttons = (
        <DecisionButtons
          revisionId={entry.revisionId}
          decisions={decisions}
          onSelectDecision={onSelectDecision}
          disabled={actionsDisabled}
          failed={failed}
          acceptTitle={
            entry.kind === "update"
              ? MEMORY_COPY.acceptUpdateTitle
              : entry.kind === "delete"
              ? MEMORY_COPY.acceptDeleteTitle
              : MEMORY_COPY.acceptAddTitle
          }
          rejectTitle={
            entry.kind === "update"
              ? MEMORY_COPY.rejectUpdateTitle
              : entry.kind === "delete"
              ? MEMORY_COPY.rejectDeleteTitle
              : MEMORY_COPY.rejectAddTitle
          }
        />
      );

      if (entry.kind === "update") {
        return (
          <div
            key={entry.key}
            className="group flex items-start justify-between gap-2 py-0.5"
          >
            {/* inline diff text — no card, no border */}
            <p className="flex-1 text-[12.5px] leading-relaxed select-text font-sans text-[rgb(var(--foreground))]/80">
              {entry.tokens.map((token, i) => {
                if (token.type === "same") {
                  return (
                    <span key={i} className="text-[rgb(var(--foreground))]/80">
                      {token.value}
                    </span>
                  );
                }
                if (token.type === "removed") {
                  return (
                    <del
                      key={i}
                      className={cn(
                        "line-through decoration-[rgb(var(--foreground-muted))]",
                        decision === "reject"
                          ? "text-[rgb(var(--foreground-muted))]/40"
                          : "text-[rgb(var(--foreground-muted))]/70"
                      )}
                    >
                      {token.value}
                    </del>
                  );
                }
                return (
                  <ins
                    key={i}
                    className={cn(
                      "no-underline",
                      decision === "reject"
                        ? "line-through text-[rgb(var(--foreground-muted))]/40"
                        : "text-[rgb(var(--foreground))]"
                    )}
                  >
                    {token.value}
                  </ins>
                );
              })}
              <span className="ml-1.5 text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 tracking-wide uppercase align-middle">
                [Replace]
              </span>
            </p>
            <span className="shrink-0 mt-0.5">{buttons}</span>
          </div>
        );
      }

      if (entry.kind === "delete") {
        return (
          <div
            key={entry.key}
            className="group flex items-start justify-between gap-2 py-0.5"
          >
            <p
              className={cn(
                "flex-1 text-[12.5px] leading-relaxed select-text font-sans line-through decoration-[rgb(var(--foreground-muted))]/60",
                decision === "reject"
                  ? "text-[rgb(var(--foreground-muted))]/40"
                  : "text-[rgb(var(--foreground-muted))]/70"
              )}
            >
              {entry.text}
              <span className="ml-1.5 no-underline text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 tracking-wide uppercase align-middle">
                [Delete]
              </span>
            </p>
            <span className="shrink-0 mt-0.5">{buttons}</span>
          </div>
        );
      }

      // add / new fact
      return (
        <div
          key={entry.key}
          className="group flex items-start justify-between gap-2 py-0.5"
        >
          <p
            className={cn(
              "flex-1 text-[12.5px] leading-relaxed select-text font-sans",
              decision === "reject"
                ? "line-through text-[rgb(var(--foreground-muted))]/40 decoration-[rgb(var(--foreground-muted))]/60"
                : "text-[rgb(var(--foreground))]"
            )}
          >
            {entry.text}
            <span className="ml-1.5 text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 tracking-wide uppercase align-middle">
              [New fact]
            </span>
          </p>
          <span className="shrink-0 mt-0.5">{buttons}</span>
        </div>
      );
    };

    return (
      <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
        {/* Document body */}
        <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 pt-3 select-text">
          <div className="space-y-6 pb-2">
            {doc.sections.map((sec) => (
              <section key={sec.key} className="space-y-2">
                <h3 className="text-[12px] font-semibold uppercase tracking-wider text-[rgb(var(--foreground-muted))] border-b border-[rgba(var(--border),0.1)] pb-1.5 flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <span>{sec.title}</span>
                    {sec.isNew && (
                      <span className="text-[10px] font-mono font-normal normal-case tracking-normal text-[rgb(var(--foreground-muted))]/60">
                        [New section]
                      </span>
                    )}
                    {sec.key === "sec_orphaned" && (
                      <span className="text-[10px] font-mono font-normal normal-case tracking-normal text-[rgb(var(--foreground-muted))]/60">
                        {MEMORY_COPY.unanchoredHint}
                      </span>
                    )}
                  </div>
                  {sec.isNew && sec.revisionId && (
                    <DecisionButtons
                      revisionId={sec.revisionId}
                      decisions={decisions}
                      onSelectDecision={onSelectDecision}
                      disabled={actionsDisabled}
                      failed={Boolean(failedRevisionIds?.has(sec.revisionId))}
                      acceptTitle={MEMORY_COPY.acceptSectionTitle}
                      rejectTitle={MEMORY_COPY.rejectSectionTitle}
                    />
                  )}
                </h3>

                <div className="space-y-1.5 pl-0.5">
                  {sec.entries.map(renderEntry)}
                </div>
              </section>
            ))}

            {doc.sections.length === 0 && (
              <p className="text-[rgb(var(--foreground-muted))] text-[13px] font-mono py-12 text-center">
                {MEMORY_COPY.noPersonalMemory}
              </p>
            )}
          </div>
        </div>

        {/* Sticky footer */}
        <div className="shrink-0 border-t border-[rgba(var(--border),0.1)] pt-2.5 mt-1 flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3 flex-wrap">
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] min-w-0">
              {undecidedCount > 0 ? MEMORY_COPY.undecidedCount(undecidedCount) : ""}
              {undecidedCount > 0 && " · "}
              {MEMORY_COPY.undecidedStayPending}
            </span>
            <button
              type="button"
              onClick={onApplyDecisions}
              disabled={actionsDisabled || decidedCount === 0}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono font-medium shrink-0",
                "bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]",
                "hover:bg-[rgba(var(--accent),0.25)] transition-colors",
                "disabled:opacity-40 disabled:cursor-not-allowed",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-1 focus-visible:ring-offset-[rgb(var(--card))]"
              )}
            >
              <Check size={12} className={cn(isApplying && "animate-spin")} />
              <span>
                {isApplying
                  ? MEMORY_COPY.applyingDecisions
                  : decidedCount === 0
                  ? MEMORY_COPY.applyDecisionsNoneSelected
                  : MEMORY_COPY.applyDecisionsCount(decidedCount)}
              </span>
            </button>
          </div>

          {anyFailed && onRetryFailed && (
            <button
              type="button"
              onClick={onRetryFailed}
              disabled={actionsDisabled}
              className="inline-flex items-center gap-1.5 self-start px-2.5 py-1 rounded-lg text-[11px] font-mono border border-[rgba(var(--border),0.3)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <RotateCw size={11} /> {MEMORY_COPY.retryFailed}
            </button>
          )}
        </div>
      </div>
    );
  }
);

SuggestionsReviewView.displayName = "SuggestionsReviewView";