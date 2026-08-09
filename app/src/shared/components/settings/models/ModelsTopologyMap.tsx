import { memo } from "react";
import { Activity, Sparkles, Brain, Volume2, Layers } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export type PipelineTab = "vad" | "asr" | "llm" | "tts" | "auxiliary";

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
    const PIPELINE_NODES = [
      { id: "vad" as PipelineTab, label: "VAD", Icon: Activity, isVerified: isVadVerified },
      { id: "asr" as PipelineTab, label: "STT", Icon: Sparkles, isVerified: isAsrVerified },
      { id: "llm" as PipelineTab, label: "LLM", Icon: Brain, isVerified: isLlmDownloaded },
      { id: "tts" as PipelineTab, label: "TTS", Icon: Volume2, isVerified: isTtsVerified },
      { id: "auxiliary" as PipelineTab, label: "Auxiliary", Icon: Layers, isVerified: isAuxiliaryVerified },
    ];

    return (
      <div
        className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-1 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "grid grid-cols-5"
        )}
      >
        {PIPELINE_NODES.map(({ id, label, Icon, isVerified }) => (
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
            <Icon
              size={18}
              className={cn(
                "transition-colors shrink-0",
                activeTab === id
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
              )}
            />
            <span className="text-[12px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
              {label}
            </span>
            <span
              className={cn(
                "w-1 h-1 rounded-full shrink-0 mt-0.5",
                isVerified
                  ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                  : "bg-[rgb(var(--accent))]/30"
              )}
            />
          </button>
        ))}
      </div>
    );
  }
);

ModelsTopologyMap.displayName = "ModelsTopologyMap";
