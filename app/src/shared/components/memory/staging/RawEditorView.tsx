import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface RawEditorViewProps {
  mode: "edit" | "import";
  draft: string;
  isCommitting: boolean;
  onDraftChange: (val: string) => void;
}

export const RawEditorView: React.FC<RawEditorViewProps> = memo(
  ({ mode, draft, isCommitting, onDraftChange }) => {
    return (
      <div
        className={cn(
          "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
          isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
        )}
      >
        <textarea
          value={draft}
          onChange={(e) => onDraftChange(e.target.value)}
          placeholder={
            mode === "import"
              ? "Paste your markdown directives here (e.g. ## Overview, ## Preferences, etc.)..."
              : undefined
          }
          className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
          spellCheck={false}
          autoFocus
        />
        <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
          <span>
            {mode === "import"
              ? "Supports standard markdown syntax."
              : "Saving streams updates directly to the canonical database."}
          </span>
          <span>~{Math.ceil(draft.length / 4).toLocaleString()} tokens</span>
        </div>
      </div>
    );
  }
);

RawEditorView.displayName = "RawEditorView";
