import { memo } from "react";
import { Mic, BrainCircuit, AudioLines, ChevronRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui";
import { ProviderLogo } from "@/shared/components/settings/models/ProviderLogo";

export interface PipelineProvidersDeskProps {
  sttActiveLabel: string;
  llmActiveLabel: string;
  ttsActiveLabel: string;
  sttProviderId?: string;
  llmProviderId?: string;
  ttsProviderId?: string;
  onSelectCategory: (category: "STT" | "LLM" | "TTS") => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const PipelineProvidersDesk = memo(({
  sttActiveLabel,
  llmActiveLabel,
  ttsActiveLabel,
  sttProviderId,
  llmProviderId,
  ttsProviderId,
  onSelectCategory,
  layoutMode = "full-max",
}: PipelineProvidersDeskProps) => {
  const STAGES = [
    {
      id: "STT" as const,
      title: "Listening",
      role: "Acoustic hearing",
      activeLabel: sttActiveLabel,
      icon: Mic,
      providerId:
        sttProviderId ||
        (sttActiveLabel.toLowerCase().includes("cloud") ? "openai" : "embedded"),
    },
    {
      id: "LLM" as const,
      title: "Reasoning",
      role: "Thinking & logic",
      activeLabel: llmActiveLabel,
      icon: BrainCircuit,
      providerId:
        llmProviderId ||
        (llmActiveLabel.toLowerCase().includes("server")
          ? "server"
          : llmActiveLabel.toLowerCase().includes("device")
          ? "embedded"
          : llmActiveLabel.toLowerCase()),
    },
    {
      id: "TTS" as const,
      title: "Speaking",
      role: "Voice synthesis",
      activeLabel: ttsActiveLabel,
      icon: AudioLines,
      providerId:
        ttsProviderId ||
        (ttsActiveLabel.toLowerCase().includes("remote")
          ? "server"
          : ttsActiveLabel.toLowerCase().includes("edge")
          ? "microsoft"
          : "embedded"),
    },
  ];

  return (
    <div className="grid grid-cols-3 gap-2.5 w-full h-full items-stretch select-none">
      {STAGES.map((stage) => {
        const IconComponent = stage.icon;

        return (
          <Tooltip
            key={stage.id}
            label={`Configure ${stage.title}`}
            side="top"
            wrapperClassName="w-full h-full flex-1"
          >
            <button
              type="button"
              onClick={() => onSelectCategory(stage.id)}
              className={cn(
                "group relative flex flex-col justify-between w-full h-full text-left transition-all duration-200 cursor-pointer overflow-hidden rounded-xl border",
                "border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.015)]",
                "hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.03)] hover:shadow-xs",
                "focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
                layoutMode === "small" ? "p-2.5" : "p-3 sm:p-3.5"
              )}
            >
              {/* Row 1: Icon on left, Title immediately to right of icon on same row, Chevron on top right */}
              <div className="w-full flex items-center justify-between gap-1.5">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <div className="w-6 h-6 rounded-md flex items-center justify-center shrink-0 bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] group-hover:bg-[rgba(var(--accent),0.16)] group-hover:scale-105 transition-all">
                    <IconComponent size={13} strokeWidth={2.2} />
                  </div>
                  <span className="font-display font-bold text-[13px] tracking-tight text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors truncate">
                    {stage.title}
                  </span>
                </div>
                <ChevronRight
                  size={13}
                  className="text-[rgb(var(--foreground-muted))]/40 group-hover:text-[rgb(var(--accent))] group-hover:translate-x-0.5 transition-all shrink-0"
                />
              </div>

              {/* Row 2: Subtext directly below title */}
              <div className="w-full mt-1 min-w-0">
                <p className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 truncate leading-tight">
                  {stage.role}
                </p>
              </div>

              {/* Row 3: Bottom line with active provider and status indicator (zero nested pill/badge) */}
              <div className="mt-2.5 pt-2 border-t border-[rgba(var(--foreground),0.06)] flex items-center justify-between gap-1.5 w-full min-w-0">
                <div className="flex items-center gap-1.5 min-w-0 flex-1">
                  <ProviderLogo providerId={stage.providerId} size={13} className="shrink-0" />
                  <span className="text-[11px] font-medium text-[rgb(var(--foreground))]/90 truncate">
                    {stage.activeLabel}
                  </span>
                </div>
                <span
                  className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)] shrink-0"
                  aria-hidden="true"
                />
              </div>
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
});

PipelineProvidersDesk.displayName = "PipelineProvidersDesk";
