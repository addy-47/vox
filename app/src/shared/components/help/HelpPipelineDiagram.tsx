import { memo } from "react";
import { ArrowRight, Mic, Cpu, MessageSquare, Volume2 } from "lucide-react";

export const HelpPipelineDiagram = memo(() => {
  return (
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Voice Pipeline Flow
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Instant Voice Response
        </span>
      </div>

      {/* Flow Container */}
      <div className="relative w-full rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] p-2.5 flex items-center justify-between gap-1 overflow-x-auto custom-scrollbar">
        {/* Stage 1: Mic */}
        <div className="flex flex-col items-center gap-1 shrink-0 w-16 text-center">
          <div className="w-8 h-8 rounded-lg border border-sky-400/40 bg-sky-400/10 text-sky-400 flex items-center justify-center">
            <Mic size={15} />
          </div>
          <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">You Speak</span>
          <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Mic capture</span>
        </div>

        <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />

        {/* Stage 2: STT */}
        <div className="flex flex-col items-center gap-1 shrink-0 w-16 text-center">
          <div className="w-8 h-8 rounded-lg border border-cyan-400/40 bg-cyan-400/10 text-cyan-400 flex items-center justify-center">
            <Cpu size={15} />
          </div>
          <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Transcribe</span>
          <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Speech to text</span>
        </div>

        <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />

        {/* Stage 3: LLM */}
        <div className="flex flex-col items-center gap-1 shrink-0 w-16 text-center">
          <div className="w-8 h-8 rounded-lg border border-violet-400/40 bg-violet-400/10 text-violet-400 flex items-center justify-center">
            <MessageSquare size={15} />
          </div>
          <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">AI Think</span>
          <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Creates reply</span>
        </div>

        <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />

        {/* Stage 4: Voice */}
        <div className="flex flex-col items-center gap-1 shrink-0 w-16 text-center">
          <div className="w-8 h-8 rounded-lg border border-emerald-400/40 bg-emerald-400/10 text-emerald-400 flex items-center justify-center">
            <Volume2 size={15} />
          </div>
          <span className="text-[10.5px] font-bold text-[rgb(var(--foreground))]">Voice Out</span>
          <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">Speaks answer</span>
        </div>
      </div>
    </div>
  );
});

HelpPipelineDiagram.displayName = "HelpPipelineDiagram";
