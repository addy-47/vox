import { memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Archive, Brain, Workflow } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile } from "@/shared/ui";
import { PersonalMemoryConfigDesk } from "./PersonalMemoryConfigDesk";
import { PERSONAL_MEMORY_CONFIG_DESK_COPY } from "@/data/settingsCopy";

interface PersonalMemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PersonalMemoryCard = memo(({ layoutMode = "full-max" }: PersonalMemoryCardProps) => {
  const personalMemory = useSettingsStore((s) => s.draftSettings?.personal_memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  const contextRetrievalEnabled = personalMemory?.context_retrieval_enabled ?? true;
  const pipelineProcessingEnabled = personalMemory?.pipeline_processing_enabled ?? true;

  const handleToggleRetrieval = useCallback(() => {
    updateDraft("personal_memory", "context_retrieval_enabled", !contextRetrievalEnabled);
  }, [contextRetrievalEnabled, updateDraft]);

  const handleTogglePipeline = useCallback(() => {
    updateDraft("personal_memory", "pipeline_processing_enabled", !pipelineProcessingEnabled);
  }, [pipelineProcessingEnabled, updateDraft]);

  if (!personalMemory) return null;

  const copy = PERSONAL_MEMORY_CONFIG_DESK_COPY;

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

        {/* Layer 2: Dedicated Personal Memory Config Desk with 3 Subtabs */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between",
            isSmall ? "h-auto" : "h-full"
          )}
        >
          <PersonalMemoryConfigDesk layoutMode={layoutMode} />
        </div>
      </div>
    </Card>
  );
});

PersonalMemoryCard.displayName = "PersonalMemoryCard";
