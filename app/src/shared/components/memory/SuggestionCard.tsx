import React, { memo } from "react";
import { Plus, Replace, Trash2, Check, X, Layers } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { MemoryRevisionView } from "@/services/memoryService";

export interface SuggestionCardProps {
  suggestion: MemoryRevisionView;
  decision?: "accept" | "reject";
  onSelectDecision: (id: string, decision: "accept" | "reject") => void;
  disabled?: boolean;
}

export const SuggestionCard: React.FC<SuggestionCardProps> = memo(
  ({ suggestion, decision, onSelectDecision, disabled = false }) => {
    const op = suggestion.op.toLowerCase();

    return (
      <div
        className={cn(
          "rounded-xl p-3.5 transition-all duration-200 border flex flex-col gap-2.5 text-left",
          decision === "accept"
            ? "border-[rgb(var(--success))]/40 bg-[rgb(var(--success))]/5 shadow-xs"
            : decision === "reject"
            ? "border-[rgb(var(--danger))]/40 bg-[rgb(var(--danger))]/5 shadow-xs"
            : "border-[rgba(var(--border),0.18)] bg-[rgba(var(--card),0.5)] hover:border-[rgba(var(--accent),0.3)]"
        )}
      >
        {/* Header: Op tag & Action toggles */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5">
            {op === "create_block" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-[rgb(var(--success))]/15 border border-[rgb(var(--success))]/30 text-[rgb(var(--success))]">
                <Plus size={11} strokeWidth={2.5} />
                Create Block
              </span>
            ) : op === "create_section" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-[rgb(var(--success))]/15 border border-[rgb(var(--success))]/30 text-[rgb(var(--success))]">
                <Layers size={11} strokeWidth={2.5} />
                Create Section
              </span>
            ) : op === "update_block" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-[rgb(var(--info))]/15 border border-[rgb(var(--info))]/30 text-[rgb(var(--info))]">
                <Replace size={11} strokeWidth={2.5} />
                Update Block
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-[rgb(var(--danger))]/15 border border-[rgb(var(--danger))]/30 text-[rgb(var(--danger))]">
                <Trash2 size={11} strokeWidth={2.5} />
                Delete Block
              </span>
            )}
            {suggestion.target_id && (
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                [{suggestion.target_id.slice(-6)}]
              </span>
            )}
          </div>

          {/* Accept / Reject Toggles (Google Docs style) */}
          <div className="flex items-center gap-1">
            <button
              type="button"
              disabled={disabled}
              onClick={() => onSelectDecision(suggestion.id, "accept")}
              className={cn(
                "p-1.5 rounded-lg border transition-all cursor-pointer",
                decision === "accept"
                  ? "bg-[rgb(var(--success))]/20 border-[rgb(var(--success))]/60 text-[rgb(var(--success))] shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--success))] hover:border-[rgb(var(--success))]/40"
              )}
              title={MEMORY_COPY.acceptSuggestionTitle}
            >
              <Check size={13} strokeWidth={2.5} />
            </button>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onSelectDecision(suggestion.id, "reject")}
              className={cn(
                "p-1.5 rounded-lg border transition-all cursor-pointer",
                decision === "reject"
                  ? "bg-[rgb(var(--danger))]/20 border-[rgb(var(--danger))]/60 text-[rgb(var(--danger))] shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:border-[rgb(var(--danger))]/40"
              )}
              title={MEMORY_COPY.rejectSuggestionTitle}
            >
              <X size={13} strokeWidth={2.5} />
            </button>
          </div>
        </div>

        {/* Diff Content Preview */}
        <div
          className={cn(
            "p-2.5 rounded-lg text-[12px] font-mono leading-relaxed border select-text break-words whitespace-pre-wrap",
            op === "create_block" || op === "create_section"
              ? "bg-[rgb(var(--success))]/10 border-[rgb(var(--success))]/20 text-[rgb(var(--success))]/80"
              : op === "update_block"
              ? "bg-[rgb(var(--info))]/10 border-[rgb(var(--info))]/20 text-[rgb(var(--info))]/80"
              : "bg-[rgb(var(--danger))]/10 border-[rgb(var(--danger))]/20 text-[rgb(var(--danger))]/80 line-through opacity-80"
          )}
        >
          {suggestion.preview}
        </div>

        {/* Footer State */}
        <div className="flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-0.5">
          <span>{suggestion.target_id || "Global"}</span>
          <span
            className={cn(
              "font-medium",
              decision === "accept"
                ? "text-[rgb(var(--success))]"
                : decision === "reject"
                ? "text-[rgb(var(--danger))]"
                : "text-[rgb(var(--foreground-muted))]/60"
            )}
          >
            {decision === "accept"
              ? "Will be accepted"
              : decision === "reject"
              ? "Will be rejected"
              : "No decision staged"}
          </span>
        </div>
      </div>
    );
  }
);

SuggestionCard.displayName = "SuggestionCard";
