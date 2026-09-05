import { memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { togglePipelineProcessing } from "@/services/memoryService";
import { Archive, Brain, Workflow } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile } from "@/shared/ui";
import { MemoryConfigDesk } from "./MemoryConfigDesk";
import { MEMORY_CONFIG_DESK_COPY } from "@/data/settingsCopy";

interface MemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const MemoryCard = memo(({ layoutMode = "full-max" }: MemoryCardProps) => {
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  const contextRetrievalEnabled = memory?.context_retrieval_enabled ?? true;
  const pipelineProcessingEnabled = memory?.pipeline_processing_enabled ?? true;

  const handleToggleRetrieval = useCallback(() => {
    updateDraft("memory", "context_retrieval_enabled", !contextRetrievalEnabled);
  }, [contextRetrievalEnabled, updateDraft]);

  const handleTogglePipeline = useCallback(async () => {
    try {
      const nextState = await togglePipelineProcessing(!pipelineProcessingEnabled);
      updateDraft("memory", "pipeline_processing_enabled", nextState);
      await commitChanges();
    } catch (e) {
      console.error("[MemoryCard] Toggle pipeline processing error:", e);
    }
  }, [pipelineProcessingEnabled, updateDraft, commitChanges]);

  if (!memory) return null;

  const copy = MEMORY_CONFIG_DESK_COPY;

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none transform-gpu",
        isSmall
          ? "bg-transparent p-0 h-auto"
          : cn(
              "glass-card p-5 lg:h-[340px] justify-between transition-all duration-300",
              isMin ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
            )
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <Archive className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {copy.cardTitle}
          </span>
        </div>
      </div>

      {/* Card Body */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-0.5 gap-2.5">
        {/* Layer 1: Top Row Two Toggle Tiles */}
        <div
          className={cn(
            "grid gap-2 shrink-0",
            isSmall ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          <ToggleTile
            title={copy.recallToggle.title}
            active={contextRetrievalEnabled}
            activeLabel={copy.recallToggle.activeLabel}
            inactiveLabel={copy.recallToggle.inactiveLabel}
            activeSublabel={copy.recallToggle.activeSublabel}
            inactiveSublabel={copy.recallToggle.inactiveSublabel}
            icon={Brain}
            onToggle={handleToggleRetrieval}
            layoutMode={layoutMode}
          />
          <ToggleTile
            title={copy.pipelineToggle.title}
            active={pipelineProcessingEnabled}
            activeLabel={copy.pipelineToggle.activeLabel}
            inactiveLabel={copy.pipelineToggle.inactiveLabel}
            activeSublabel={copy.pipelineToggle.activeSublabel}
            inactiveSublabel={copy.pipelineToggle.inactiveSublabel}
            icon={Workflow}
            onToggle={handleTogglePipeline}
            layoutMode={layoutMode}
          />
        </div>

        {/* Layer 2: Dedicated Memory Config Desk with 5 Subtabs */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between",
            isSmall ? "h-auto" : "h-full"
          )}
        >
          <MemoryConfigDesk layoutMode={layoutMode} />
        </div>
      </div>
    </Card>
  );
});

MemoryCard.displayName = "MemoryCard";
