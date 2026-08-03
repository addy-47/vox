import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Layers, Zap } from "lucide-react";

interface PipelineModeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PipelineModeCard = memo(({ layoutMode }: PipelineModeCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;
  const isModular = draftSettings.interaction.pipeline_mode === "modular";

  const toggle = () => {
    updateDraft("interaction", "pipeline_mode", isModular ? "realtime" : "modular");
  };

  return (
    <div className="group flex items-center w-full h-[85px] relative">
      <div
        onClick={toggle}
        className="flex-1 p-3 rounded-xl group-hover:rounded-r-none border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between h-full cursor-pointer"
      >
        <div className="flex items-center justify-between">
          <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">
            Pipeline
          </span>
          <div className="flex items-center gap-3">
            {isModular ? (
              <Layers size={16} className="text-[rgb(var(--accent))]" />
            ) : (
              <Zap size={16} className="text-[rgb(var(--accent))]" />
            )}
          </div>
        </div>

        <div className="flex items-end justify-between mt-2">
          <div className="flex flex-col">
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
              {isModular ? "Modular" : "Realtime"}
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
              {isModular ? "Hybrid Grid" : "Stream Duplex"}
            </span>
          </div>

          {/* Visualizer widget */}
          <div className="flex items-center">
            {isModular ? (
              <div className="flex flex-col gap-[1.5px] items-center">
                <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                <span className="w-2.5 h-[2px] bg-[rgb(var(--accent))]/60 rounded animate-pulse" />
                <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
              </div>
            ) : (
              <div className="w-7 h-2 bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] rounded-full relative overflow-hidden flex items-center">
                <span className="absolute w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-flow-dot shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
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

PipelineModeCard.displayName = "PipelineModeCard";
