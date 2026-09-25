import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { History, ShieldOff, FoldVertical } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile } from "@/shared/ui";
import { WORKING_MEMORY_SETTINGS_COPY } from "@/data/settingsCopy";
import { WorkingMemoryConfigDesk } from "./WorkingMemoryConfigDesk";

interface WorkingMemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const WorkingMemoryCard = memo(({ layoutMode = "full-max" }: WorkingMemoryCardProps) => {
  const workingMemory = useSettingsStore((s) => s.draftSettings?.working_memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  if (!workingMemory) return null;

  const copy = WORKING_MEMORY_SETTINGS_COPY;

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
          <History className="text-[rgb(var(--accent))]" size={17} />
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
          {/* Incognito Mode */}
          <ToggleTile
            title={copy.privateModeTitle}
            active={workingMemory.private_mode}
            activeLabel={copy.privateModeActive}
            inactiveLabel={copy.privateModeInactive}
            activeSublabel={copy.privateModeActiveSub}
            inactiveSublabel={copy.privateModeInactiveSub}
            icon={ShieldOff}
            onToggle={() =>
              updateDraft("working_memory", "private_mode", !workingMemory.private_mode)
            }
            layoutMode={layoutMode}
          />

          {/* Auto Compaction Mode */}
          <ToggleTile
            title={copy.autoCompactionTitle}
            active={workingMemory.auto_compaction ?? false}
            activeLabel={copy.autoCompactionActive}
            inactiveLabel={copy.autoCompactionInactive}
            activeSublabel={copy.autoCompactionActiveSub}
            inactiveSublabel={copy.autoCompactionInactiveSub}
            icon={FoldVertical}
            onToggle={() =>
              updateDraft("working_memory", "auto_compaction", !workingMemory.auto_compaction)
            }
            layoutMode={layoutMode}
          />
        </div>

        {/* Layer 2: Working Memory Config Desk (Web Search | Content Share) */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between",
            isSmall ? "h-auto" : "h-full"
          )}
        >
          <WorkingMemoryConfigDesk layoutMode={layoutMode} />
        </div>
      </div>
    </Card>
  );
});

WorkingMemoryCard.displayName = "WorkingMemoryCard";
