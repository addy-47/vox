import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Activity, Radio } from "lucide-react";
import { ToggleTile } from "@/shared/ui";
import { TRIGGER_MODE_COPY } from "@/data/settingsCopy";

interface TriggerModeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const TriggerModeCard = memo(({ layoutMode }: TriggerModeCardProps) => {
  const mode = useSettingsStore((s) => s.draftSettings?.interaction.mode ?? "Passive");
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isPassive = mode === "Passive";

  return (
    <ToggleTile
      title="Trigger"
      active={isPassive}
      activeLabel={TRIGGER_MODE_COPY.continuousTitle}
      inactiveLabel={TRIGGER_MODE_COPY.pttTitle}
      activeSublabel={TRIGGER_MODE_COPY.continuousSub}
      inactiveSublabel={TRIGGER_MODE_COPY.pttSub}
      icon={isPassive ? Activity : Radio}
      onToggle={() =>
        updateDraft("interaction", "mode", isPassive ? "PTT" : "Passive")
      }
      layoutMode={layoutMode}
      visualizer={
        isPassive ? (
          <div className="flex items-end gap-[1.5px] h-3">
            <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-1" />
            <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-2" />
            <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-3" />
            <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-4" />
          </div>
        ) : (
          <div className="w-3 h-3 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
            <span className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-60" />
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          </div>
        )
      }
    />
  );
});

TriggerModeCard.displayName = "TriggerModeCard";
