import { memo } from "react";
import { AudioLines, BrainCircuit, ChevronRight, Ear } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui";
import { PIPELINE_PROVIDERS_DESK_COPY } from "@/data/settingsCopy";
import type { ActiveSelection } from "@/store/settingsStore";

export interface PipelineProvidersDeskProps {
  /** Verbatim `settings.<domain>.active`. Every rendered value is read from it. */
  sttActive?: ActiveSelection;
  llmActive?: ActiveSelection;
  ttsActive?: ActiveSelection;
  onSelectCategory: (category: "STT" | "LLM" | "TTS") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PipelineProvidersDesk = memo(({
  sttActive,
  llmActive,
  ttsActive,
  onSelectCategory,
  layoutMode = "full-max",
}: PipelineProvidersDeskProps) => {
  const copy = PIPELINE_PROVIDERS_DESK_COPY;
  const compact = layoutMode === "small";

  const STAGES = [
    { id: "STT" as const, card: copy.cards.stt, Icon: Ear, active: sttActive },
    { id: "LLM" as const, card: copy.cards.llm, Icon: BrainCircuit, active: llmActive },
    { id: "TTS" as const, card: copy.cards.tts, Icon: AudioLines, active: ttsActive },
  ];

  const iconSize = compact ? 16 : 26;
  const chevronSize = compact ? 12 : 14;
  const textSize = compact ? "text-[11px]" : "text-[14px]";

  return (
    <div className="grid grid-cols-3 gap-2 w-full flex-1 min-h-0 select-none">
      {STAGES.map((stage) => {
        const IconComponent = stage.Icon;
        const category = stage.active?.category ?? "";
        const provider = stage.active?.provider ?? "";
        const model = stage.active?.model ?? "";

        return (
          <Tooltip
            key={stage.id}
            label={`${copy.configureAriaPrefix} ${stage.card.title}`}
            side="top"
            asChild
          >
            <button
              type="button"
              onClick={() => onSelectCategory(stage.id)}
              aria-label={`${copy.configureAriaPrefix} ${stage.card.title}`}
              className={cn(
                "group relative h-full w-full min-h-0 flex flex-col items-center justify-center gap-1.5 overflow-hidden text-center cursor-pointer rounded-xl border transition-colors duration-200",
                compact ? "px-4 py-3" : "px-5 py-3.5",
                "border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.015)]",
                "hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.03)]",
                "focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[rgb(var(--accent))]"
              )}
            >
              <ChevronRight
                size={chevronSize}
                strokeWidth={2.2}
                className="absolute top-2 right-2 text-[rgb(var(--foreground-muted))]/35 transition-all duration-200 group-hover:translate-x-0.5 group-hover:text-[rgb(var(--accent))]"
              />

              <IconComponent size={iconSize} strokeWidth={2} className="shrink-0 text-[rgb(var(--accent))]" />

              <span className="flex flex-col items-center gap-0 max-w-full">
                {category && (
                  <span className={cn("max-w-full leading-tight truncate", textSize, "text-[rgb(var(--foreground))]/85")}>
                    {category}
                  </span>
                )}
                {provider && (
                  <span className={cn("max-w-full leading-tight truncate", textSize, "text-[rgb(var(--foreground-muted))]/80")}>
                    {provider}
                  </span>
                )}
                {model && (
                  <span className={cn("max-w-full leading-tight truncate", textSize, "text-[rgb(var(--foreground-muted))]/80")}>
                    {model}
                  </span>
                )}
              </span>
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
});

PipelineProvidersDesk.displayName = "PipelineProvidersDesk"