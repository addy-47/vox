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
    return (
      <div
        className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-1 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "grid grid-cols-5"
        )}
      >
        {/* NODE 1: VAD */}
        <button
          type="button"
          onClick={() => onChangeTab("vad")}
          className={cn(
            "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
            activeTab === "vad"
              ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
              : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
            layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
          )}
        >
          <Activity
            size={18}
            className={cn(
              "transition-colors shrink-0",
              activeTab === "vad"
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
            )}
          />
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
            VAD
          </span>
          <span
            className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isVadVerified
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )}
          />
        </button>

        {/* NODE 2: STT */}
        <button
          type="button"
          onClick={() => onChangeTab("asr")}
          className={cn(
            "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
            activeTab === "asr"
              ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
              : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
            layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
          )}
        >
          <Sparkles
            size={18}
            className={cn(
              "transition-colors shrink-0",
              activeTab === "asr"
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
            )}
          />
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
            STT
          </span>
          <span
            className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isAsrVerified
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )}
          />
        </button>

        {/* NODE 3: LLM */}
        <button
          type="button"
          onClick={() => onChangeTab("llm")}
          className={cn(
            "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
            activeTab === "llm"
              ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
              : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
            layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
          )}
        >
          <Brain
            size={18}
            className={cn(
              "transition-colors shrink-0",
              activeTab === "llm"
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
            )}
          />
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
            LLM
          </span>
          <span
            className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isLlmDownloaded
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )}
          />
        </button>

        {/* NODE 4: TTS */}
        <button
          type="button"
          onClick={() => onChangeTab("tts")}
          className={cn(
            "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
            activeTab === "tts"
              ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
              : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
            layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
          )}
        >
          <Volume2
            size={18}
            className={cn(
              "transition-colors shrink-0",
              activeTab === "tts"
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
            )}
          />
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
            TTS
          </span>
          <span
            className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isTtsVerified
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )}
          />
        </button>

        {/* NODE 5: AUXILIARY */}
        <button
          type="button"
          onClick={() => onChangeTab("auxiliary")}
          className={cn(
            "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden cursor-pointer",
            activeTab === "auxiliary"
              ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
              : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
            layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
          )}
        >
          <Layers
            size={18}
            className={cn(
              "transition-colors shrink-0",
              activeTab === "auxiliary"
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]"
            )}
          />
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">
            Auxiliary
          </span>
          <span
            className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isAuxiliaryVerified
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )}
          />
        </button>
      </div>
    );
  }
);

ModelsTopologyMap.displayName = "ModelsTopologyMap";
