import { memo } from "react";
import {
  Clock,
  Activity,
  Check,
  RotateCcw,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { type IngestionStats } from "@/services/memoryService";

export interface TimelineQueueSectionProps {
  stats: IngestionStats | null;
  pipelineProcessingEnabled: boolean;
}

export const TimelineQueueSection = memo<TimelineQueueSectionProps>(({
  stats,
  pipelineProcessingEnabled,
}) => {
  const pendingCount = stats?.pending ?? 0;
  const processingCount = stats?.processing ?? 0;
  const completedCount = stats?.completed ?? 0;
  const failedCount = stats?.failed ?? 0;

  return (
    <section className="shrink-0 flex flex-col gap-4 px-2 pt-1.5 pb-5 border-b border-[rgba(var(--border),0.06)]">
      {/* Header: Title + Pipeline State */}
      <div className="flex items-center justify-between py-1">
        <span className="text-[12px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/60">
          {MEMORY_COPY.timeline.ingestionTitle}
        </span>
        {!pipelineProcessingEnabled ? (
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--foreground-muted))]/40" />
            <span>Disabled</span>
          </div>
        ) : processingCount > 0 ? (
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))]">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
            <span>Processing</span>
          </div>
        ) : pendingCount > 0 ? (
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgba(var(--accent),0.85)]">
            <Clock size={11} />
            <span>Queued ({pendingCount})</span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgba(var(--accent),0.65)]">
            <Check size={11} strokeWidth={2.5} className="text-[rgba(var(--accent),0.75)]" />
            <span>Up to date</span>
          </div>
        )}
      </div>

      {/* Continuous Pipeline Timeline Spine */}
      <div
        className={cn(
          "flex flex-col px-0.5 pt-1.5 pb-1 transition-all duration-300",
          !pipelineProcessingEnabled && "opacity-35 grayscale pointer-events-none"
        )}
      >
        {/* Stage 1: Ingestion Queue (Pending) */}
        <div className="flex gap-3 items-stretch min-w-0">
          <div className="flex flex-col items-center shrink-0 w-4.5">
            <div className="w-4.5 h-4.5 rounded-full border border-[rgba(var(--accent),0.35)] bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-[rgb(var(--foreground-muted))]/70">
              <Clock size={9} strokeWidth={2} />
            </div>
            <div className="w-[1.5px] flex-1 my-1 bg-[rgba(var(--accent),0.2)]" />
          </div>
          <div className="flex-1 min-w-0 pb-4 flex items-start justify-between">
            <div className="flex flex-col min-w-0 pr-2">
              <span className="text-[13px] font-sans font-medium leading-snug text-[rgb(var(--foreground))]">
                {MEMORY_COPY.timeline.pipelineStagePending}
              </span>
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                {MEMORY_COPY.timeline.pipelineStagePendingDesc}
              </span>
            </div>
            <span className="font-mono text-[11.5px] text-[rgb(var(--foreground-muted))]/70 shrink-0 pt-0.5">
              {pendingCount}
            </span>
          </div>
        </div>

        {/* Stage 2: Semantic Extraction (Processing) */}
        <div className="flex gap-3 items-stretch min-w-0">
          <div className="flex flex-col items-center shrink-0 w-4.5">
            <div
              className={cn(
                "w-4.5 h-4.5 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 transition-all",
                processingCount > 0
                  ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.45)]"
                  : "border-[rgba(var(--accent),0.35)] text-[rgb(var(--foreground-muted))]/60"
              )}
            >
              <Activity size={9} className={cn(processingCount > 0 && "animate-pulse")} strokeWidth={2.5} />
            </div>
            <div className={cn("w-[1.5px] flex-1 my-1", processingCount > 0 ? "bg-[rgb(var(--accent))]/40" : "bg-[rgba(var(--accent),0.2)]")} />
          </div>
          <div className="flex-1 min-w-0 pb-4 flex items-start justify-between">
            <div className="flex flex-col min-w-0 pr-2">
              <span
                className={cn(
                  "text-[13px] font-sans font-medium leading-snug",
                  processingCount > 0 ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                )}
              >
                {MEMORY_COPY.timeline.pipelineStageProcessing}
              </span>
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                {MEMORY_COPY.timeline.pipelineStageProcessingDesc}
              </span>
            </div>
            <span
              className={cn(
                "font-mono text-[11.5px] shrink-0 pt-0.5",
                processingCount > 0 ? "text-[rgb(var(--accent))] font-bold" : "text-[rgb(var(--foreground-muted))]/70"
              )}
            >
              {processingCount}
            </span>
          </div>
        </div>

        {/* Stage 3: Graph Indexed (Completed) */}
        <div className="flex gap-3 items-stretch min-w-0">
          <div className="flex flex-col items-center shrink-0 w-4.5">
            <div className="w-4.5 h-4.5 rounded-full border border-[rgb(var(--accent))] bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-[rgb(var(--accent))]">
              <Check size={9} strokeWidth={2.5} />
            </div>
            {failedCount > 0 && (
              <div className="w-[1.5px] flex-1 my-1 bg-[rgba(var(--accent),0.2)]" />
            )}
          </div>
          <div className={cn("flex-1 min-w-0 flex items-start justify-between", failedCount > 0 && "pb-4")}>
            <div className="flex flex-col min-w-0 pr-2">
              <span className="text-[13px] font-sans font-medium leading-snug text-[rgb(var(--foreground))]">
                {MEMORY_COPY.timeline.pipelineStageCompleted}
              </span>
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                {MEMORY_COPY.timeline.pipelineStageCompletedDesc}
              </span>
            </div>
            <span className="font-mono text-[11.5px] font-medium text-[rgb(var(--accent))] shrink-0 pt-0.5">
              {completedCount}
            </span>
          </div>
        </div>

        {/* Stage 4: Failed (if present) */}
        {failedCount > 0 && (
          <div className="flex gap-3 items-stretch min-w-0">
            <div className="flex flex-col items-center shrink-0 w-4.5">
              <div className="w-4.5 h-4.5 rounded-full border border-[rgb(var(--danger))] bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-[rgb(var(--danger))]">
                <RotateCcw size={9} strokeWidth={2} />
              </div>
            </div>
            <div className="flex-1 min-w-0 flex items-start justify-between">
              <div className="flex flex-col min-w-0 pr-2">
                <span className="text-[13px] font-sans font-medium leading-snug text-[rgb(var(--danger))]">
                  {MEMORY_COPY.timeline.pipelineStageFailed}
                </span>
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                  {MEMORY_COPY.timeline.pipelineStageFailedDesc}
                </span>
              </div>
              <span className="font-mono text-[11.5px] font-medium text-[rgb(var(--danger))] shrink-0 pt-0.5">
                {failedCount}
              </span>
            </div>
          </div>
        )}
      </div>
    </section>
  );
});

TimelineQueueSection.displayName = "TimelineQueueSection";
