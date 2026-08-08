import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Layers, Zap } from "lucide-react";
import { ToggleTile } from "@/shared/ui";

interface PipelineModeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PipelineModeCard = memo(({ layoutMode }: PipelineModeCardProps) => {
  const pipelineMode = useSettingsStore((s) => s.draftSettings?.interaction.pipeline_mode ?? "modular");
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isModular = pipelineMode === "modular";

  return (
    <ToggleTile
      title="Pipeline"
      active={isModular}
      activeLabel="Modular"
      inactiveLabel="Realtime"
      activeSublabel="Hybrid Grid"
      inactiveSublabel="Stream Duplex"
      icon={isModular ? Layers : Zap}
      onToggle={() =>
        updateDraft("interaction", "pipeline_mode", isModular ? "realtime" : "modular")
      }
      layoutMode={layoutMode}
      visualizer={
        isModular ? (
          <div className="flex flex-col gap-[1.5px] items-center">
            <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
            <span className="w-2.5 h-[2px] bg-[rgb(var(--accent))]/60 rounded animate-pulse" />
            <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
          </div>
        ) : (
          <div className="w-7 h-2 bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] rounded-full relative overflow-hidden flex items-center">
            <span className="absolute w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-flow-dot shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
          </div>
        )
      }
    />
  );
});

PipelineModeCard.displayName = "PipelineModeCard";
