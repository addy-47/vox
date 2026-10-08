import React, { memo } from "react";
import { Sparkles, Hand } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface MemoryErrorStateProps {
  error: string;
  onRetry: () => void;
}

export const MemoryErrorState: React.FC<MemoryErrorStateProps> = memo(({ error, onRetry }) => (
  <div
    className="absolute left-1/2 flex flex-col items-center justify-center pointer-events-none z-20"
    style={{
      top: "calc(50% - 36px)",
      transform: "translate(-50%, -50%)",
    }}
  >
    <div className="rounded-2xl bg-[rgba(var(--card),0.85)] border border-[rgba(var(--border),0.12)] backdrop-blur-xl p-8 max-w-sm text-center shadow-2xl">
      <h3 className="font-display text-[14px] font-bold text-[rgb(var(--foreground))] mb-1">
        {MEMORY_COPY.loadFailedTitle}
      </h3>
      <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed mb-4">
        {error}
      </p>
      <button
        type="button"
        onClick={onRetry}
        className="pointer-events-auto px-4 py-2 rounded-xl text-[12px] font-bold bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.2)] transition-colors cursor-pointer"
      >
        {MEMORY_COPY.loadFailedRetry}
      </button>
    </div>
  </div>
));

MemoryErrorState.displayName = "MemoryErrorState";

export const MemoryEmptyState: React.FC = memo(() => (
  <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
    <div className="rounded-2xl bg-[rgba(var(--card),0.85)] border border-[rgba(var(--border),0.12)] backdrop-blur-xl p-8 max-w-sm text-center shadow-2xl">
      <Sparkles size={28} className="mx-auto text-[rgb(var(--accent))] mb-3 opacity-80" />
      <h3 className="font-display text-[14px] font-bold text-[rgb(var(--foreground))] mb-1">
        {MEMORY_COPY.emptyFactsTitle}
      </h3>
      <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
        {MEMORY_COPY.emptyFactsDesc}
      </p>
    </div>
  </div>
));

MemoryEmptyState.displayName = "MemoryEmptyState";

export const MemoryHint: React.FC = memo(() => (
  <div className="absolute bottom-24 left-1/2 -translate-x-1/2 z-30 flex flex-col items-center gap-2 pointer-events-none">
    <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))] opacity-60">
      <Hand size={12} className="text-[rgb(var(--accent))]" />
      <span>{MEMORY_COPY.memoryHint}</span>
    </div>
  </div>
));

MemoryHint.displayName = "MemoryHint";
