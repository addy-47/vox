import { useState, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { BookOpenText, Brain, FoldVertical, Workflow } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile, SegmentedControl } from "@/shared/ui";
import { MEMORY_CARD_COPY } from "@/data/settingsCopy";
import { CardTitleBar } from "../shared/CardTitleBar";
import { SETTINGS_DOMAIN_CARD_HEIGHT_CLASS } from "../shared/settingsLayout";
import { MemoryConfigDesk } from "./MemoryConfigDesk";

interface MemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type MemoryView = "basic" | "advanced";

const VIEW_OPTIONS = [
  { id: "basic" as const, label: MEMORY_CARD_COPY.viewBasic },
  { id: "advanced" as const, label: MEMORY_CARD_COPY.viewAdvanced },
];

export const MemoryCard = memo(({ layoutMode = "full-max" }: MemoryCardProps) => {
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [view, setView] = useState<MemoryView>("basic");

  const isSmall = layoutMode === "small";

  const contextRetrievalEnabled = memory?.context_retrieval_enabled ?? true;
  const pipelineProcessingEnabled = memory?.pipeline_processing_enabled ?? true;

  const handleToggleRetrieval = useCallback(() => {
    updateDraft("memory", "context_retrieval_enabled", !contextRetrievalEnabled);
  }, [contextRetrievalEnabled, updateDraft]);

  const handleTogglePipeline = useCallback(() => {
    updateDraft("memory", "pipeline_processing_enabled", !pipelineProcessingEnabled);
  }, [pipelineProcessingEnabled, updateDraft]);

  if (!memory) return null;

  const copy = MEMORY_CARD_COPY;
  const isBasic = view === "basic";

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none transform-gpu",
        isSmall
          ? "bg-transparent p-0 h-auto"
          : cn("glass-card p-5 justify-between", SETTINGS_DOMAIN_CARD_HEIGHT_CLASS)
      )}
    >
      {/* Header */}
      <CardTitleBar
        icon={<BookOpenText size={17} />}
        title={copy.cardTitle}
        action={
          <SegmentedControl
            options={VIEW_OPTIONS}
            value={view}
            onChange={setView}
            size="sm"
          />
        }
      />

      {/* Card Body */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-0.5 gap-2.5">
        {/* Layer 1: Toggle Tiles (group depends on the active view) */}
        <div
          className={cn(
            "grid gap-2 shrink-0",
            isBasic ? "grid-cols-2" : "grid-cols-1"
          )}
        >
          {isBasic ? (
            <>
              <ToggleTile
                title={copy.tiles.recall.title}
                active={contextRetrievalEnabled}
                activeLabel={copy.tiles.recall.activeLabel}
                inactiveLabel={copy.tiles.recall.inactiveLabel}
                activeSublabel={copy.tiles.recall.activeSublabel}
                inactiveSublabel={copy.tiles.recall.inactiveSublabel}
                icon={Brain}
                onToggle={handleToggleRetrieval}
                layoutMode={layoutMode}
              />
              <ToggleTile
                title={copy.tiles.learning.title}
                active={pipelineProcessingEnabled}
                activeLabel={copy.tiles.learning.activeLabel}
                inactiveLabel={copy.tiles.learning.inactiveLabel}
                activeSublabel={copy.tiles.learning.activeSublabel}
                inactiveSublabel={copy.tiles.learning.inactiveSublabel}
                icon={Workflow}
                onToggle={handleTogglePipeline}
                layoutMode={layoutMode}
              />
            </>
          ) : (
            <ToggleTile
              title={copy.tiles.autoCompaction.title}
              active={memory.auto_compaction}
              activeLabel={copy.tiles.autoCompaction.activeLabel}
              inactiveLabel={copy.tiles.autoCompaction.inactiveLabel}
              activeSublabel={copy.tiles.autoCompaction.activeSublabel}
              inactiveSublabel={copy.tiles.autoCompaction.inactiveSublabel}
              icon={FoldVertical}
              onToggle={() => updateDraft("memory", "auto_compaction", !memory.auto_compaction)}
              layoutMode={layoutMode}
            />
          )}
        </div>

        {/* Layer 2: Memory Config Desk */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between",
            isSmall ? "h-auto min-h-[160px]" : "h-full"
          )}
        >
          <MemoryConfigDesk view={view} layoutMode={layoutMode} />
        </div>
      </div>
    </Card>
  );
});

MemoryCard.displayName = "MemoryCard";
