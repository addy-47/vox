import { memo } from "react";
import { AudioWaveform, Ear, BrainCircuit, AudioLines, LifeBuoy } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { DIRTY_STATE_COPY } from "@/data/settingsCopy";

export type PipelineTab = "vad" | "stt" | "llm" | "tts" | "auxiliary";

interface ModelsTopologyMapProps {
  activeTab: PipelineTab;
  onChangeTab: (tab: PipelineTab) => void;
  layoutMode?: "full-max" | "full-min" | "small";
  isVadVerified?: boolean;
  isAsrVerified?: boolean;
  isLlmDownloaded?: boolean;
  isTtsVerified?: boolean;
  isAuxiliaryVerified?: boolean;
}

export const ModelsTopologyMap = memo(
  ({
    activeTab,
    onChangeTab,
    layoutMode,
    isVadVerified = true,
    isAsrVerified = true,
    isLlmDownloaded = true,
    isTtsVerified = true,
    isAuxiliaryVerified = true,
  }: ModelsTopologyMapProps) => {
    const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);

    const PIPELINE_NODES = [
      { id: "vad" as PipelineTab, label: "Voice Detection", Icon: AudioWaveform, isVerified: isVadVerified },
      { id: "stt" as PipelineTab, label: "Listening", Icon: Ear, isVerified: isAsrVerified },
      { id: "llm" as PipelineTab, label: "Reasoning", Icon: BrainCircuit, isVerified: isLlmDownloaded },
      { id: "tts" as PipelineTab, label: "Speaking", Icon: AudioLines, isVerified: isTtsVerified },
      { id: "auxiliary" as PipelineTab, label: "Support", Icon: LifeBuoy, isVerified: isAuxiliaryVerified },
    ];

    return (
      <div
        className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-2.5 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "grid grid-cols-5"
        )}
      >
        {PIPELINE_NODES.map(({ id, label, Icon }) => {
          const isDirty = isCategoryDirty(id);

          return (
            <button
              key={id}
              type="button"
              onClick={() => onChangeTab(id)}
              className={cn(
                "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
                activeTab === id
                  ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                  : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
              )}
            >
              {isDirty && (
                <span
                  title={DIRTY_STATE_COPY.category}
                  className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-amber-400 shadow-[0_0_6px_rgba(251,191,36,0.8)] shrink-0"
                />
              )}
              <Icon
                size={18}
                className={cn(
                  "transition-colors shrink-0",
                  activeTab === id
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
                )}
              />
              <span className="text-[11px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate max-w-full leading-tight">
                {label}
              </span>
            </button>
          );
        })}
      </div>
    );
  }
);

ModelsTopologyMap.displayName = "ModelsTopologyMap";
