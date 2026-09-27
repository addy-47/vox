import React, { memo } from "react";
import { Sparkles, Calendar, Tag } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { FactRecord } from "@/services/memoryService";

export interface LearnedFactsListProps {
  facts: FactRecord[];
  isConsolidating?: boolean;
}

export const LearnedFactsList: React.FC<LearnedFactsListProps> = memo(
  ({ facts, isConsolidating = false }) => {
    return (
      <div className="flex-1 min-h-0 flex flex-col justify-between pt-1">
        {facts.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center p-6 text-center">
            <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] mb-3">
              <Sparkles size={20} />
            </div>
            <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
              {MEMORY_COPY.noActiveFacts}
            </h4>
            <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
              Talk with Vox across conversations to generate identity facts.
            </p>
          </div>
        ) : (
          <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 space-y-2.5">
            {facts.map((fact) => (
              <div
                key={fact.id}
                className={cn(
                  "p-3.5 rounded-xl bg-[rgba(var(--card),0.55)] border border-[rgba(var(--border),0.18)] shadow-sm hover:border-[rgba(var(--accent),0.35)] transition-all flex flex-col gap-2",
                  isConsolidating && "opacity-60 pointer-events-none"
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-1.5">
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[10px] font-mono font-bold uppercase tracking-wider bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))]">
                      <Tag size={10} />
                      {fact.fact_type}
                    </span>
                    {fact.session_id && (
                      <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                        Session #{fact.session_id}
                      </span>
                    )}
                  </div>

                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] flex items-center gap-1">
                    <Calendar size={10} />
                    {new Date(fact.created_at).toLocaleDateString(undefined, {
                      month: "short",
                      day: "numeric",
                    })}
                  </span>
                </div>

                <p className="text-[12.5px] text-[rgb(var(--foreground))] leading-relaxed font-sans select-text">
                  {fact.text}
                </p>
              </div>
            ))}
          </div>
        )}

        {facts.length > 0 && (
          <div className="shrink-0 flex items-center justify-between text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] pt-3 border-t border-[rgba(var(--border),0.08)]">
            <span>
              {facts.length} active {facts.length === 1 ? "fact" : "facts"} queued for integration
            </span>
            <span className="text-[rgb(var(--accent))] font-medium">Ready</span>
          </div>
        )}
      </div>
    );
  }
);

LearnedFactsList.displayName = "LearnedFactsList";
