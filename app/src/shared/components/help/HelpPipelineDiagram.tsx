import { memo } from "react";
import { ArrowRight, Mic, Cpu, MessageSquare, Volume2 } from "lucide-react";

export const HelpPipelineDiagram = memo(() => {
  return (
    <div className="flex flex-col gap-2 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Voice Pipeline Flow
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Instant Voice Response
        </span>
      </div>

      {/* Flow Container */}
      <div className="w-full py-1.5 flex items-center justify-between gap-0.5">
        {/* Stage 1: Mic */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="w-7 h-7 rounded-lg border border-sky-400/30 bg-sky-400/10 text-sky-400 flex items-center justify-center shrink-0">
            <Mic size={14} />
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">You Speak</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Mic capture</span>
        </div>

        <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0 mx-0.5" />

        {/* Stage 2: STT */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="w-7 h-7 rounded-lg border border-cyan-400/30 bg-cyan-400/10 text-cyan-400 flex items-center justify-center shrink-0">
            <Cpu size={14} />
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">Transcribe</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Speech to text</span>
        </div>

        <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0 mx-0.5" />

        {/* Stage 3: LLM */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="w-7 h-7 rounded-lg border border-violet-400/30 bg-violet-400/10 text-violet-400 flex items-center justify-center shrink-0">
            <MessageSquare size={14} />
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">AI Think</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Creates reply</span>
        </div>

        <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0 mx-0.5" />

        {/* Stage 4: Voice */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="w-7 h-7 rounded-lg border border-emerald-400/30 bg-emerald-400/10 text-emerald-400 flex items-center justify-center shrink-0">
            <Volume2 size={14} />
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">Voice Out</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Speaks answer</span>
        </div>
      </div>
    </div>
  );
});

HelpPipelineDiagram.displayName = "HelpPipelineDiagram";
