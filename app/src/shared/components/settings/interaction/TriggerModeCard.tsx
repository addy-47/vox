import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Activity, Radio } from "lucide-react";

interface TriggerModeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const TriggerModeCard = memo(({ layoutMode }: TriggerModeCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;
  const isPassive = draftSettings.interaction.main_app_mode === "Passive";

  const toggle = () => {
    updateDraft("interaction", "main_app_mode", isPassive ? "PTT" : "Passive");
  };

  return (
    <div className="group flex items-center w-full h-[85px] relative">
      <div
        onClick={toggle}
        className="flex-1 p-3 rounded-xl group-hover:rounded-r-none border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between h-full cursor-pointer"
      >
        <div className="flex items-center justify-between">
          <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">
            Trigger
          </span>
          <div className="flex items-center gap-3">
            {isPassive ? (
              <Activity size={16} className="text-[rgb(var(--accent))]" />
            ) : (
              <Radio size={16} className="text-[rgb(var(--accent))]" />
            )}
          </div>
        </div>

        <div className="flex items-end justify-between mt-2">
          <div className="flex flex-col">
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
              {isPassive ? "Continuous" : "Push-To-Talk"}
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
              {isPassive ? "Passive Sense" : "Manual Trigger"}
            </span>
          </div>

          {/* Visualizer widget */}
          <div className="h-4 flex items-end">
            {isPassive ? (
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
            )}
          </div>
        </div>
      </div>

      {/* Slide-out toggle side panel */}
      <div
        onClick={toggle}
        className="h-full w-0 group-hover:w-[38px] opacity-0 group-hover:opacity-100 flex items-center justify-center bg-[rgba(var(--accent),0.05)] border border-transparent border-l-transparent group-hover:border-[rgba(var(--accent),0.15)] group-hover:border-l-transparent rounded-r-xl transition-all duration-300 overflow-hidden cursor-pointer select-none shrink-0"
      >
        <span className="text-[8px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
          {layoutMode === "small" ? "TAP" : "TOGGLE"}
        </span>
      </div>
    </div>
  );
});

TriggerModeCard.displayName = "TriggerModeCard";
