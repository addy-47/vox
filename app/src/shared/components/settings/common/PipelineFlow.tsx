import React, { memo } from "react";
import { Mic, Zap, Cpu, Volume2, ArrowRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface PipelineStep {
  label: string;
  subLabel?: string;
  active?: boolean;
  icon?: React.ReactNode;
}

interface PipelineFlowProps {
  steps?: PipelineStep[];
  mode?: "realtime" | "cascade";
  className?: string;
}

export const PipelineFlow: React.FC<PipelineFlowProps> = memo(({ steps, mode = "cascade", className }) => {
  const defaultSteps: PipelineStep[] =
    mode === "realtime"
      ? [
          { label: "VAD", subLabel: "TEN VAD", active: true, icon: <Mic size={14} /> },
          { label: "S2S Engine", subLabel: "Gemini / MiniOmni", active: true, icon: <Zap size={14} /> },
          { label: "Audio Out", subLabel: "Sub-200ms", active: true, icon: <Volume2 size={14} /> },
        ]
      : [
          { label: "VAD", subLabel: "Silence Det", active: true, icon: <Mic size={14} /> },
          { label: "STT", subLabel: "Qwen3 / Nemotron", active: true, icon: <Cpu size={14} /> },
          { label: "LLM", subLabel: "Gemma4 / Llama3.2", active: true, icon: <Zap size={14} /> },
          { label: "TTS", subLabel: "Supertonic / Edge", active: true, icon: <Volume2 size={14} /> },
        ];

  const renderSteps = steps || defaultSteps;

  return (
    <div className={cn("flex items-center justify-between gap-1 p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)]", className)}>
      {renderSteps.map((step, idx) => (
        <React.Fragment key={idx}>
          <div className="flex items-center gap-2 px-2 py-1 rounded-lg">
            <div className={cn("w-6 h-6 rounded-md flex items-center justify-center text-xs border", step.active ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))]" : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]")}>
              {step.icon}
            </div>
            <div>
              <div className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">{step.label}</div>
              {step.subLabel && <div className="text-[9px] text-[rgb(var(--foreground-muted))] font-mono">{step.subLabel}</div>}
            </div>
          </div>
          {idx < renderSteps.length - 1 && <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />}
        </React.Fragment>
      ))}
    </div>
  );
});

PipelineFlow.displayName = "PipelineFlow";
