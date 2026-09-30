import React, { memo } from "react";
import { Check, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { diffWords } from "@/shared/lib/diff";
import type { MemoryRevisionView } from "@/services/memoryService";

export interface ParsedBlockItem {
  id?: string;
  text: string;
}

export interface ParsedSectionItem {
  id?: string;
  title: string;
  blocks: ParsedBlockItem[];
}

export interface SuggestionsReviewViewProps {
  baseSections: ParsedSectionItem[];
  createRevisionsBySection: Map<string, MemoryRevisionView[]>;
  updateRevisionsByTarget: Map<string, MemoryRevisionView>;
  deleteRevisionsByTarget: Map<string, MemoryRevisionView>;
  newSectionRevisions: MemoryRevisionView[];
  decisions: Record<string, "accept" | "reject">;
  onSelectDecision: (id: string, action: "accept" | "reject") => void;
  parseOpPayload: (content?: string) => {
    text?: string;
    section_id?: string;
    block_id?: string;
    title?: string;
    blocks?: Array<{ id?: string; text?: string }>;
  };
}

export const SuggestionsReviewView: React.FC<SuggestionsReviewViewProps> = memo(
  ({
    baseSections,
    createRevisionsBySection,
    updateRevisionsByTarget,
    deleteRevisionsByTarget,
    newSectionRevisions,
    decisions,
    onSelectDecision,
    parseOpPayload,
  }) => {
    return (
      <div className="flex-1 min-h-0 flex flex-col pt-3 overflow-y-auto custom-scrollbar pr-1 select-text">
        <div className="space-y-6 pb-4">
          {baseSections.map((sec, secIdx) => {
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

                          {/* Decision Buttons */}
                          <div className="flex items-center gap-1 shrink-0 pt-0.5">
                            <button
                              type="button"
                              onClick={() => onSelectDecision(updateRev.id, "accept")}
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
                              onClick={() => onSelectDecision(updateRev.id, "reject")}
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
                              onClick={() => onSelectDecision(deleteRev.id, "accept")}
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
                              onClick={() => onSelectDecision(deleteRev.id, "reject")}
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
                            onClick={() => onSelectDecision(createRev.id, "accept")}
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
                            onClick={() => onSelectDecision(createRev.id, "reject")}
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
                      onClick={() => onSelectDecision(secRev.id, "accept")}
                      className={cn(
                        "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                        decision === "accept"
                          ? "bg-emerald-500 text-black border border-emerald-400 scale-105"
                          : "border border-emerald-500/35 text-emerald-400/70 hover:text-emerald-400 hover:border-emerald-500/70 hover:bg-emerald-500/10"
                      )}
                      title="Accept entire new section"
                    >
                      <Check size={12} strokeWidth={2.5} />
                    </button>
                    <button
                      type="button"
                      onClick={() => onSelectDecision(secRev.id, "reject")}
                      className={cn(
                        "w-6 h-6 rounded-full flex items-center justify-center transition-all cursor-pointer shadow-xs",
                        decision === "reject"
                          ? "bg-rose-500 text-white border border-rose-400 scale-105"
                          : "border border-rose-500/35 text-rose-400/70 hover:text-rose-400 hover:border-rose-500/70 hover:bg-rose-500/10"
                      )}
                      title="Reject entire new section"
                    >
                      <X size={12} strokeWidth={2.5} />
                    </button>
                  </div>
                </div>

                <div className="space-y-2 pl-1">
                  {blocks.map((b, bIdx) => (
                    <p
                      key={b.id || `new_blk_${bIdx}`}
                      className={cn(
                        "text-[12.5px] leading-relaxed text-emerald-300 font-sans select-text",
                        decision === "reject" && "line-through opacity-40 text-rose-400"
                      )}
                    >
                      {b.text}
                    </p>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    );
  }
);

SuggestionsReviewView.displayName = "SuggestionsReviewView";
