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
      { id: "vad" as PipelineTab, label: "Voice Detection", Icon: Activity, isVerified: isVadVerified },
      { id: "asr" as PipelineTab, label: "Speech to Text", Icon: Sparkles, isVerified: isAsrVerified },
      { id: "llm" as PipelineTab, label: "Reasoning", Icon: Brain, isVerified: isLlmDownloaded },
      { id: "tts" as PipelineTab, label: "Speech", Icon: Volume2, isVerified: isTtsVerified },
      { id: "auxiliary" as PipelineTab, label: "Support", Icon: Layers, isVerified: isAuxiliaryVerified },
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
        {PIPELINE_NODES.map(({ id, label, Icon }) => (
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
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate max-w-full leading-tight">
              {label}
            </span>
          </button>
        ))}
      </div>
    );
  }
);

ModelsTopologyMap.displayName = "ModelsTopologyMap";
