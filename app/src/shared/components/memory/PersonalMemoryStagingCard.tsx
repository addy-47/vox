import React, { useState, useEffect, useRef, useMemo, memo } from "react";
import {
  Upload,
  Edit3,
  Check,
  X,
  Sparkles,
  ArrowLeft,
  MessageSquare,
  Trash2,
  Layers,
  History,
  CheckCircle2,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { LearnedFactsList } from "./LearnedFactsList";
import { diffWords } from "@/shared/lib/diff";
import type {
  MemoryRevisionView,
  ObservationRecord,
} from "@/services/memoryService";

export type StagingMode =
  | "idle"
  | "import"
  | "edit"
  | "comment"
  | "facts"
  | "suggestions";

export interface MemoryComment {
  id: string;
  line: number;
  quotedText: string;
  text: string;
  top: number;
  createdAt: number;
}

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
}

interface ParsedBlock {
  id?: string;
  text: string;
}

interface ParsedSection {
  id?: string;
  title: string;
  blocks: ParsedBlock[];
}

/**
 * Parses canonical JSON or raw Markdown into structured sections and blocks.
 */
function parseSections(content: string | null, markdown: string | null): ParsedSection[] {
  // 1. Try canonical JSON first
  if (content?.trim()) {
    try {
      const parsed = JSON.parse(content);
      if (parsed && Array.isArray(parsed.sections)) {
        return parsed.sections.map((s: { id?: string; title?: string; blocks?: { id?: string; text?: string }[] }) => ({
          id: s.id,
          title: s.title ?? "Section",
          blocks: (s.blocks ?? []).map((b) => ({
            id: b.id,
            text: b.text ?? "",
          })),
        }));
      }
    } catch {
      // Fall through to markdown
    }
  }

  // 2. Fall back to parsing rendered Markdown
  const textToParse = markdown || content || "";
  if (!textToParse.trim()) return [];

  const lines = textToParse.split("\n");
  const sections: ParsedSection[] = [];
  let currentSection: ParsedSection | null = null;
  let currentParagraphLines: string[] = [];

  const flushParagraph = () => {
    if (currentParagraphLines.length > 0) {
      const text = currentParagraphLines.join(" ").trim();
      if (text) {
        if (!currentSection) {
          currentSection = { title: "Overview", blocks: [] };
          sections.push(currentSection);
        }
        currentSection.blocks.push({ text });
      }
      currentParagraphLines = [];
    }
  };

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (line.startsWith("## ")) {
      flushParagraph();
      const title = line.replace(/^##\s+/, "").trim();
      currentSection = { title, blocks: [] };
      sections.push(currentSection);
    } else if (line.length === 0) {
      flushParagraph();
    } else {
      currentParagraphLines.push(line);
    }
  }
  flushParagraph();
  return sections;
}

interface ResolvedOpPayload {
  op?: string;
  title?: string;
  blocks?: string[];
  section_id?: string;
  block_id?: string;
  text?: string;
}

function parseOpPayload(json: string): ResolvedOpPayload {
  try {
    return JSON.parse(json);
  } catch {
    return {};
  }
}

export const PersonalMemoryStagingCard: React.FC<PersonalMemoryStagingCardProps> = memo(
  ({
    canonicalContent,
    canonicalMarkdown,
    activeVersion = 1,
    mode,
    onModeChange,
    onSave,
    onRegenerateWithComments,
    comments = [],
    onDeleteComment,
    onUpdateComment,
    onClearComments,
    unconsolidatedCount: _unconsolidatedCount,
    isSaving,
    isConsolidating = false,
    isCommitting,
    suggestions = [],
    onApplySuggestions,
    isApplyingSuggestions = false,
    candidateFacts = [],
    justCommitted = false,
    onViewVersionHistory,
  }) => {
    const [draft, setDraft] = useState("");
    const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
    const [editingCommentText, setEditingCommentText] = useState("");
    const [decisions, setDecisions] = useState<Record<string, "accept" | "reject">>({});

    const prevModeRef = useRef(mode);

    // Sync draft only when mode changes
    useEffect(() => {
      const entered = prevModeRef.current !== mode;
      prevModeRef.current = mode;
      if (!entered) return;
      if (mode === "edit") {
        setDraft(canonicalMarkdown || canonicalContent || "");
      } else if (mode === "import") {
        setDraft("");
      }
    }, [mode, canonicalMarkdown, canonicalContent]);

    // Parse current base sections
    const baseSections = useMemo(
      () => parseSections(canonicalContent, canonicalMarkdown),
      [canonicalContent, canonicalMarkdown]
    );

    // Group suggestions by target
    const {
      updateRevisionsByTarget,
      deleteRevisionsByTarget,
      createRevisionsBySection,
      newSectionRevisions,
    } = useMemo(() => {
      const updates = new Map<string, MemoryRevisionView>();
      const deletes = new Map<string, MemoryRevisionView>();
      const creates = new Map<string, MemoryRevisionView[]>();
      const newSections: MemoryRevisionView[] = [];

      for (const sug of suggestions) {
        const payload = parseOpPayload(sug.content);
        if (sug.op === "update_block") {
          const target = sug.target_id || payload.block_id || "";
          if (target) updates.set(target, sug);
          if (sug.old_text) updates.set(sug.old_text.trim(), sug);
        } else if (sug.op === "delete_block") {
          const target = sug.target_id || payload.block_id || "";
          if (target) deletes.set(target, sug);
          if (sug.old_text) deletes.set(sug.old_text.trim(), sug);
        } else if (sug.op === "create_block") {
          const secTarget = sug.target_id || payload.section_id || "";
          const list = creates.get(secTarget) || [];
          list.push(sug);
          creates.set(secTarget, list);
        } else if (sug.op === "create_section") {
          newSections.push(sug);
        }
      }

      return {
        updateRevisionsByTarget: updates,
        deleteRevisionsByTarget: deletes,
        createRevisionsBySection: creates,
        newSectionRevisions: newSections,
      };
    }, [suggestions]);

    // Calculate decision breakdown
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
        return { ...prev, [id]: action };
      });
    };

    const handleAcceptAll = () => {
      const all: Record<string, "accept" | "reject"> = {};
      for (const s of suggestions) {
        all[s.id] = "accept";
      }
      setDecisions(all);
    };

    const handleRejectAll = () => {
      const all: Record<string, "accept" | "reject"> = {};
      for (const s of suggestions) {
        all[s.id] = "reject";
      }
      setDecisions(all);
    };

    const handleApplyAllDecisions = async () => {
      if (!onApplySuggestions || suggestions.length === 0) return;
      // Default unselected decisions to 'accept'
      const finalDecisions: Record<string, "accept" | "reject"> = {};
      for (const sug of suggestions) {
        finalDecisions[sug.id] = decisions[sug.id] ?? "accept";
      }
      await onApplySuggestions(finalDecisions);
      setDecisions({});
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
          (isSaving || isConsolidating || isCommitting || isApplyingSuggestions) &&
            "opacity-50 pointer-events-none select-none"
        )}
      >
        {/* Dynamic Header Bar */}
        <div className="flex items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
          <div className="flex items-center gap-3">
            <div
              className={cn(
                "w-8 h-8 rounded-xl border flex items-center justify-center transition-colors shadow-sm",
                isSuggestionsActive
                  ? "bg-[rgba(var(--accent),0.18)] border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))]"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]"
              )}
            >
              {justCommitted ? (
                <CheckCircle2 size={16} className="text-emerald-400" />
              ) : isSuggestionsActive ? (
                <Sparkles size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "comment" ? (
                <MessageSquare size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "edit" ? (
                <Edit3 size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "import" ? (
                <Upload size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "facts" ? (
                <Sparkles size={16} className="text-[rgb(var(--accent))]" />
              ) : (
                <Layers size={16} />
              )}
            </div>

            <div className="flex flex-col">
              <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))]">
                {MEMORY_COPY.stagingMirror}
              </span>
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                {justCommitted
                  ? "All changes integrated"
                  : isSuggestionsActive
                  ? `${decisionStats.total} ${
                      decisionStats.total === 1
                        ? MEMORY_COPY.suggestedChangeCount
                        : MEMORY_COPY.suggestedChangesCount
                    } (${decisionStats.accepted} accepted, ${decisionStats.rejected} rejected, ${
                      decisionStats.pending
                    } pending)`
                  : mode === "comment"
                  ? `${comments.length} line-anchored comments queued`
                  : mode === "edit"
                  ? "Direct in-place edits"
                  : mode === "import"
                  ? "Paste markdown to replace current profile"
                  : mode === "facts"
                  ? `${candidateFacts.length} active identity observations queued`
                  : "No suggestions"}
              </span>
            </div>
          </div>

          {/* Action controls in header */}
          <div className="flex items-center gap-2">
            {isSuggestionsActive ? (
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleAcceptAll}
                  disabled={isApplyingSuggestions}
                  className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-emerald-400 hover:bg-emerald-500/10 transition-colors cursor-pointer"
                >
                  {MEMORY_COPY.acceptAll}
                </button>
                <button
                  type="button"
                  onClick={handleRejectAll}
                  disabled={isApplyingSuggestions}
                  className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-rose-400 hover:bg-rose-500/10 transition-colors cursor-pointer"
                >
                  {MEMORY_COPY.rejectAll}
                </button>
                <button
                  type="button"
                  onClick={handleApplyAllDecisions}
                  disabled={isApplyingSuggestions}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono font-medium bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.45)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-all cursor-pointer shadow-sm disabled:opacity-40"
                >
                  <Check size={12} className={cn(isApplyingSuggestions && "animate-spin")} />
                  <span>
                    {isApplyingSuggestions
                      ? MEMORY_COPY.applyingDecisions
                      : MEMORY_COPY.consolidate}
                  </span>
                </button>
              </div>
            ) : mode === "comment" ? (
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => handleRegenerate()}
                  disabled={isSaving || comments.length === 0}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  <Sparkles size={13} className={cn(isSaving && "animate-spin")} />
                  <span>{isSaving ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerate}</span>
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onClearComments?.();
                    onModeChange("idle");
                  }}
                  disabled={isSaving}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  <X size={13} /> {MEMORY_COPY.cancel}
                </button>
              </div>
            ) : mode === "edit" || mode === "import" ? (
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleCommit}
                  disabled={isSaving || !draft.trim()}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  <ArrowLeft size={13} className={cn(isSaving && "animate-pulse")} />
                  <span>{isSaving ? "Saving…" : "Save & Commit"}</span>
                </button>
                <button
                  type="button"
                  onClick={() => onModeChange("idle")}
                  disabled={isSaving}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  <X size={13} /> {MEMORY_COPY.cancel}
                </button>
              </div>
            ) : mode === "facts" ? (
              <button
                type="button"
                onClick={() => onModeChange("idle")}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <X size={13} /> {MEMORY_COPY.backToActions}
              </button>
            ) : null}
          </div>
        </div>

        {/* ── Mode 1: Successful Integration Splash (Step 6) ── */}
        {justCommitted ? (
          <div className="flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center animate-in fade-in duration-300">
            <div className="w-14 h-14 rounded-full bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center text-emerald-400 mb-4 shadow-lg">
              <Check size={26} strokeWidth={2.5} />
            </div>
            <h3 className="text-[15px] font-semibold tracking-wide text-[rgb(var(--foreground))] mb-1.5">
              {MEMORY_COPY.allChangesIntegrated}
            </h3>
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm mb-6 font-mono leading-relaxed">
              {MEMORY_COPY.allChangesIntegratedDesc.replace("{v}", String(activeVersion))}
            </p>
            {onViewVersionHistory && (
              <button
                type="button"
                onClick={onViewVersionHistory}
                className="flex items-center gap-2 px-4 py-2 rounded-xl text-[11.5px] font-mono bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.18)] text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer shadow-sm"
              >
                <History size={14} className="text-[rgb(var(--accent))]" />
                <span>{MEMORY_COPY.viewVersionHistory}</span>
              </button>
            )}
          </div>
        ) : isSuggestionsActive ? (
          /* ── Mode 2: Google Docs Inline Suggestions Review (Steps 2, 3, 4, 5) ── */
          <div className="flex-1 min-h-0 flex flex-col pt-3 overflow-y-auto custom-scrollbar pr-1 select-text">
            <div className="space-y-6 pb-4">
              {baseSections.map((sec, secIdx) => {
                // Collect created blocks for this section
                const createdForSec =
                  (sec.id ? createRevisionsBySection.get(sec.id) : undefined) ||
                  createRevisionsBySection.get(sec.title) ||
                  [];

                return (
                  <div key={sec.id || `sec_${secIdx}`} className="space-y-3">
                    {/* Section Title */}
                    <h3 className="text-[13px] font-display font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5 border-b border-[rgba(var(--border),0.08)] pb-1.5">
                      <span>## {sec.title}</span>
                    </h3>

                    {/* Section Blocks with Inline Diffs */}
                    <div className="space-y-3 pl-1">
                      {sec.blocks.map((blk, blkIdx) => {
                        const targetKey = blk.id || blk.text.trim();
                        const updateRev = updateRevisionsByTarget.get(targetKey);
                        const deleteRev = deleteRevisionsByTarget.get(targetKey);

                        // 1. Update suggestion: word-level diff
                        if (updateRev) {
                          const payload = parseOpPayload(updateRev.content);
                          const newText = payload.text || "";
                          const oldText = updateRev.old_text || blk.text;
                          const tokens = diffWords(oldText, newText);
                          const decision = decisions[updateRev.id];

                          return (
                            <div
                              key={blk.id || `blk_${blkIdx}`}
                              className="group relative flex items-start justify-between gap-3 p-2.5 rounded-xl border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--card),0.5)] hover:border-[rgba(var(--accent),0.4)] transition-all"
                            >
                              <div className="flex-1 text-[12.5px] leading-relaxed select-text font-sans">
                                {tokens.map((token, tIdx) => {
                                  if (token.type === "same") {
                                    return (
                                      <span key={tIdx} className="text-[rgb(var(--foreground))]">
                                        {token.value}
                                      </span>
                                    );
                                  }
                                  if (token.type === "removed") {
                                    return (
                                      <del
                                        key={tIdx}
                                        className={cn(
                                          "line-through px-0.5 rounded text-rose-400 font-medium",
                                          decision === "reject"
                                            ? "opacity-40"
                                            : "bg-rose-500/10"
                                        )}
                                      >
                                        {token.value}
                                      </del>
                                    );
                                  }
                                  return (
                                    <ins
                                      key={tIdx}
                                      className={cn(
                                        "no-underline px-0.5 rounded text-emerald-400 font-semibold",
                                        decision === "reject"
                                          ? "line-through opacity-40 text-rose-400/80"
                                          : "bg-emerald-500/10"
                                      )}
                                    >
                                      {token.value}
                                    </ins>
                                  );
                                })}
                              </div>

                              {/* Google Docs Tick / Cross Decision Buttons */}
                              <div className="flex items-center gap-1 shrink-0 pt-0.5">
                                <button
                                  type="button"
                                  onClick={() => handleSelectDecision(updateRev.id, "accept")}
                                  className={cn(
                                    "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                    decision === "accept"
                                      ? "bg-emerald-500 text-black border border-emerald-400 scale-105"
                                      : "border border-emerald-500/35 text-emerald-400/70 hover:text-emerald-400 hover:border-emerald-500/70 hover:bg-emerald-500/10"
                                  )}
                                  title="Accept suggestion"
                                >
                                  <Check size={12} strokeWidth={2.5} />
                                </button>
                                <button
                                  type="button"
                                  onClick={() => handleSelectDecision(updateRev.id, "reject")}
                                  className={cn(
                                    "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                    decision === "reject"
                                      ? "bg-rose-500 text-white border border-rose-400 scale-105"
                                      : "border border-rose-500/35 text-rose-400/70 hover:text-rose-400 hover:border-rose-500/70 hover:bg-rose-500/10"
                                  )}
                                  title="Reject suggestion"
                                >
                                  <X size={12} strokeWidth={2.5} />
                                </button>
                              </div>
                            </div>
                          );
                        }

                        // 2. Delete suggestion: red strikethrough block
                        if (deleteRev) {
                          const decision = decisions[deleteRev.id];
                          return (
                            <div
                              key={blk.id || `blk_${blkIdx}`}
                              className="group relative flex items-start justify-between gap-3 p-2.5 rounded-xl border border-rose-500/30 bg-rose-500/5 hover:border-rose-500/50 transition-all"
                            >
                              <p
                                className={cn(
                                  "flex-1 text-[12.5px] leading-relaxed font-sans select-text text-rose-300",
                                  decision === "reject" ? "opacity-50" : "line-through"
                                )}
                              >
                                {blk.text}
                              </p>

                              <div className="flex items-center gap-1 shrink-0 pt-0.5">
                                <button
                                  type="button"
                                  onClick={() => handleSelectDecision(deleteRev.id, "accept")}
                                  className={cn(
                                    "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                    decision === "accept"
                                      ? "bg-emerald-500 text-black border border-emerald-400 scale-105"
                                      : "border border-emerald-500/35 text-emerald-400/70 hover:text-emerald-400 hover:border-emerald-500/70 hover:bg-emerald-500/10"
                                  )}
                                  title="Accept deletion"
                                >
                                  <Check size={12} strokeWidth={2.5} />
                                </button>
                                <button
                                  type="button"
                                  onClick={() => handleSelectDecision(deleteRev.id, "reject")}
                                  className={cn(
                                    "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                    decision === "reject"
                                      ? "bg-rose-500 text-white border border-rose-400 scale-105"
                                      : "border border-rose-500/35 text-rose-400/70 hover:text-rose-400 hover:border-rose-500/70 hover:bg-rose-500/10"
                                  )}
                                  title="Reject deletion (keep block)"
                                >
                                  <X size={12} strokeWidth={2.5} />
                                </button>
                              </div>
                            </div>
                          );
                        }

                        // 3. Normal unchanged block
                        return (
                          <div
                            key={blk.id || `blk_${blkIdx}`}
                            className="p-1 text-[12.5px] leading-relaxed text-[rgb(var(--foreground))] select-text font-sans"
                          >
                            {blk.text}
                          </div>
                        );
                      })}

                      {/* Appended CreateBlock suggestions for this section */}
                      {createdForSec.map((createRev) => {
                        const payload = parseOpPayload(createRev.content);
                        const newText = payload.text || createRev.preview;
                        const decision = decisions[createRev.id];

                        return (
                          <div
                            key={createRev.id}
                            className="group relative flex items-start justify-between gap-3 p-2.5 rounded-xl border border-emerald-500/30 bg-emerald-500/5 hover:border-emerald-500/50 transition-all"
                          >
                            <p
                              className={cn(
                                "flex-1 text-[12.5px] leading-relaxed font-sans select-text text-emerald-300 font-medium",
                                decision === "reject" && "line-through opacity-40 text-rose-400"
                              )}
                            >
                              {newText}
                            </p>

                            <div className="flex items-center gap-1 shrink-0 pt-0.5">
                              <button
                                type="button"
                                onClick={() => handleSelectDecision(createRev.id, "accept")}
                                className={cn(
                                  "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                  decision === "accept"
                                    ? "bg-emerald-500 text-black border border-emerald-400 scale-105"
                                    : "border border-emerald-500/35 text-emerald-400/70 hover:text-emerald-400 hover:border-emerald-500/70 hover:bg-emerald-500/10"
                                )}
                                title="Accept addition"
                              >
                                <Check size={12} strokeWidth={2.5} />
                              </button>
                              <button
                                type="button"
                                onClick={() => handleSelectDecision(createRev.id, "reject")}
                                className={cn(
                                  "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                                  decision === "reject"
                                    ? "bg-rose-500 text-white border border-rose-400 scale-105"
                                    : "border border-rose-500/35 text-rose-400/70 hover:text-rose-400 hover:border-rose-500/70 hover:bg-rose-500/10"
                                )}
                                title="Reject addition"
                              >
                                <X size={12} strokeWidth={2.5} />
                              </button>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}

              {/* Entirely New Sections proposed */}
              {newSectionRevisions.map((secRev) => {
                const payload = parseOpPayload(secRev.content);
                const title = payload.title || "New Section";
                const blocks = payload.blocks || [];
                const decision = decisions[secRev.id];

                return (
                  <div
                    key={secRev.id}
                    className="p-3.5 rounded-xl border border-emerald-500/35 bg-emerald-500/5 space-y-2.5"
                  >
                    <div className="flex items-center justify-between gap-3 border-b border-emerald-500/20 pb-2">
                      <h3 className="text-[13px] font-display font-bold uppercase tracking-wider text-emerald-400">
                        ## {title}
                      </h3>
                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          type="button"
                          onClick={() => handleSelectDecision(secRev.id, "accept")}
                          className={cn(
                            "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                            decision === "accept"
                              ? "bg-emerald-500 text-black border border-emerald-400 scale-105"
                              : "border border-emerald-500/35 text-emerald-400/70 hover:text-emerald-400 hover:border-emerald-500/70 hover:bg-emerald-500/10"
                          )}
                          title="Accept new section"
                        >
                          <Check size={12} strokeWidth={2.5} />
                        </button>
                        <button
                          type="button"
                          onClick={() => handleSelectDecision(secRev.id, "reject")}
                          className={cn(
                            "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                            decision === "reject"
                              ? "bg-rose-500 text-white border border-rose-400 scale-105"
                              : "border border-rose-500/35 text-rose-400/70 hover:text-rose-400 hover:border-rose-500/70 hover:bg-rose-500/10"
                          )}
                          title="Reject new section"
                        >
                          <X size={12} strokeWidth={2.5} />
                        </button>
                      </div>
                    </div>

                    <div className="space-y-2 pl-1">
                      {blocks.map((blkText, i) => (
                        <p
                          key={i}
                          className={cn(
                            "text-[12.5px] leading-relaxed font-sans text-emerald-300",
                            decision === "reject" && "line-through opacity-40 text-rose-400"
                          )}
                        >
                          {blkText}
                        </p>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        ) : mode === "idle" ? (
          /* ── Mode 3: Clean Centered Action Hub (Step 1) ── */
          <div className="flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center animate-in fade-in duration-300">
            <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] mb-3.5 shadow-sm">
              <Layers size={20} />
            </div>

            <h3 className="text-[14px] font-semibold tracking-wide text-[rgb(var(--foreground))] mb-1">
              {MEMORY_COPY.noPendingSuggestionsTitle}
            </h3>
            <p className="text-[11.5px] text-[rgb(var(--foreground-muted))] max-w-xs mb-6 font-mono leading-relaxed">
              {MEMORY_COPY.noPendingSuggestionsDesc}
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 w-full max-w-sm">
              <button
                type="button"
                onClick={() => onModeChange("comment")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <MessageSquare size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Comment
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  {comments.length > 0 ? `${comments.length} ready` : "Line directives"}
                </span>
              </button>

              <button
                type="button"
                onClick={() => onModeChange("import")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <Upload size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Import
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  Paste markdown
                </span>
              </button>

              <button
                type="button"
                onClick={() => onModeChange("edit")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <Edit3 size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Edit
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  In-place editor
                </span>
              </button>
            </div>
          </div>
        ) : mode === "comment" ? (
          /* ── Mode 4: Comment Queue Slate ── */
          <div className="flex-1 min-h-0 flex flex-col justify-between pt-3 transition-opacity duration-500">
            {comments.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center p-6 text-center">
                <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] mb-3">
                  <MessageSquare size={20} />
                </div>
                <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
                  {MEMORY_COPY.noComments}
                </h4>
                <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
                  {MEMORY_COPY.selectTextPrompt}
                </p>
              </div>
            ) : (
              <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 space-y-2.5">
                {comments.map((c) => (
                  <div
                    key={c.id}
                    className="p-3 rounded-xl bg-[rgba(var(--card),0.6)] border border-[rgba(var(--accent),0.22)] shadow-sm hover:border-[rgba(var(--accent),0.4)] transition-all flex flex-col gap-1.5 group"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex items-center gap-2">
                        <span className="px-2 py-0.5 rounded-md text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
                          {MEMORY_COPY.linePrefix} {c.line}
                        </span>
                        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate max-w-[180px] sm:max-w-[240px]">
                          &ldquo;{c.quotedText}&rdquo;
                        </span>
                      </div>

                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          type="button"
                          onClick={() => {
                            setEditingCommentId(c.id);
                            setEditingCommentText(c.text);
                          }}
                          className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] opacity-60 group-hover:opacity-100 transition-all cursor-pointer"
                          title="Edit comment"
                        >
                          <Edit3 size={13} />
                        </button>
                        {onDeleteComment && (
                          <button
                            type="button"
                            onClick={() => {
                              if (editingCommentId === c.id) setEditingCommentId(null);
                              onDeleteComment(c.id);
                            }}
                            className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-500/10 opacity-60 group-hover:opacity-100 transition-all cursor-pointer"
                            title={MEMORY_COPY.deleteCommentTooltip}
                          >
                            <Trash2 size={13} />
                          </button>
                        )}
                      </div>
                    </div>

                    {editingCommentId === c.id ? (
                      <div className="flex flex-col gap-2 pt-1 animate-in fade-in duration-150">
                        <textarea
                          autoFocus
                          value={editingCommentText}
                          onChange={(e) => setEditingCommentText(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                              if (editingCommentText.trim() && onUpdateComment) {
                                onUpdateComment(c.id, editingCommentText.trim());
                              }
                              setEditingCommentId(null);
                            } else if (e.key === "Escape") {
                              setEditingCommentId(null);
                            }
                          }}
                          rows={2}
                          className="w-full bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.4)] focus:border-[rgb(var(--accent))] rounded-lg p-2 text-[12px] text-[rgb(var(--foreground))] focus:outline-none leading-relaxed font-sans resize-none transition-colors"
                        />
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            type="button"
                            onClick={() => setEditingCommentId(null)}
                            className="px-2 py-0.5 rounded text-[11px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] cursor-pointer transition-colors"
                          >
                            Cancel
                          </button>
                          <button
                            type="button"
                            onClick={() => {
                              if (editingCommentText.trim() && onUpdateComment) {
                                onUpdateComment(c.id, editingCommentText.trim());
                              }
                              setEditingCommentId(null);
                            }}
                            disabled={!editingCommentText.trim()}
                            className="px-2.5 py-0.5 rounded text-[11px] font-mono font-medium bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] disabled:opacity-40 cursor-pointer transition-colors shadow-xs"
                          >
                            Save
                          </button>
                        </div>
                      </div>
                    ) : (
                      <p className="text-[12px] text-[rgb(var(--foreground))] leading-relaxed pl-0.5 font-sans">
                        {c.text}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            )}

            {comments.length > 0 && (
              <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-3 border-t border-[rgba(var(--border),0.08)]">
                <span>{comments.length} comments queued</span>
                {onClearComments && (
                  <button
                    type="button"
                    onClick={onClearComments}
                    className="text-[11px] text-red-400 hover:underline cursor-pointer"
                  >
                    {MEMORY_COPY.clearAllComments}
                  </button>
                )}
              </div>
            )}
          </div>
        ) : mode === "import" ? (
          /* ── Mode 5: Import Markdown Textarea ── */
          <div
            className={cn(
              "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
              isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
            )}
          >
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="Paste your markdown directives here (e.g. ## Overview, ## Preferences, etc.)..."
              className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
              spellCheck={false}
              autoFocus
            />
            <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
              <span>Supports standard markdown syntax.</span>
              <span>~{Math.ceil(draft.length / 4).toLocaleString()} tokens</span>
            </div>
          </div>
        ) : mode === "edit" ? (
          /* ── Mode 6: Direct In-Place Editor ── */
          <div
            className={cn(
              "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
              isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
            )}
          >
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
              spellCheck={false}
              autoFocus
            />
            <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
              <span>Saving streams updates directly to the canonical database.</span>
              <span>~{Math.ceil(draft.length / 4).toLocaleString()} tokens</span>
            </div>
          </div>
        ) : mode === "facts" ? (
          /* ── Mode 7: Candidate Observations Inspector ── */
          <div className="flex-1 min-h-0 flex flex-col pt-3 transition-opacity duration-500">
            <LearnedFactsList
              facts={candidateFacts}
              isConsolidating={isConsolidating}
            />
          </div>
        ) : null}
      </div>
    );
  }
);

PersonalMemoryStagingCard.displayName = "PersonalMemoryStagingCard";
