import React, { memo } from "react";
import { Layers, MessageSquare, Upload, Edit3 } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { StagingMode } from "../stagingTypes";

export interface ActionHubViewProps {
  commentsCount: number;
  onModeChange: (mode: StagingMode) => void;
}

export const ActionHubView: React.FC<ActionHubViewProps> = memo(
  ({ commentsCount, onModeChange }) => {
    return (
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
              {commentsCount > 0 ? `${commentsCount} ready` : "Line directives"}
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
    );
  }
);

ActionHubView.displayName = "ActionHubView";
