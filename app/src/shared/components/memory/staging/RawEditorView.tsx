import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

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
            mode === "import" ? MEMORY_COPY.importPlaceholder : undefined
          }
          className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
          spellCheck={false}
          autoFocus
        />
        <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
          <span>
            {mode === "import" ? MEMORY_COPY.importHint : MEMORY_COPY.editHint}
          </span>
          <span>{MEMORY_COPY.approxTokens(Math.ceil(draft.length / 4))}</span>
        </div>
      </div>
    );
  }
);

RawEditorView.displayName = "RawEditorView";
