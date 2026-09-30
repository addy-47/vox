import React, { memo } from "react";
import { Plus, Replace, Trash2, Check, X, Layers } from "lucide-react";
import { cn } from "@/shared/lib/utils";
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
            ? "border-emerald-500/40 bg-emerald-500/5 shadow-xs"
            : decision === "reject"
            ? "border-rose-500/40 bg-rose-500/5 shadow-xs"
            : "border-[rgba(var(--border),0.18)] bg-[rgba(var(--card),0.5)] hover:border-[rgba(var(--accent),0.3)]"
        )}
      >
        {/* Header: Op tag & Action toggles */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5">
            {op === "create_block" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-emerald-500/15 border border-emerald-500/30 text-emerald-400">
                <Plus size={11} strokeWidth={2.5} />
                Create Block
              </span>
            ) : op === "create_section" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-emerald-500/15 border border-emerald-500/30 text-emerald-400">
                <Layers size={11} strokeWidth={2.5} />
                Create Section
              </span>
            ) : op === "update_block" ? (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-sky-500/15 border border-sky-500/30 text-sky-400">
                <Replace size={11} strokeWidth={2.5} />
                Update Block
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10.5px] font-mono font-semibold bg-rose-500/15 border border-rose-500/30 text-rose-400">
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
                  ? "bg-emerald-500/20 border-emerald-500/60 text-emerald-400 shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-emerald-400 hover:border-emerald-500/40"
              )}
              title="Accept suggestion"
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
                  ? "bg-rose-500/20 border-rose-500/60 text-rose-400 shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:border-rose-500/40"
              )}
              title="Reject suggestion"
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
              ? "bg-emerald-500/10 border-emerald-500/20 text-emerald-200"
              : op === "update_block"
              ? "bg-sky-500/10 border-sky-500/20 text-sky-200"
              : "bg-rose-500/10 border-rose-500/20 text-rose-300 line-through opacity-80"
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
                ? "text-emerald-400"
                : decision === "reject"
                ? "text-rose-400"
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
