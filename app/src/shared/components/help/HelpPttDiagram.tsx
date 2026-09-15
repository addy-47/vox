import { memo } from "react";
import { Mic, ArrowRight, CheckCircle2, XCircle } from "lucide-react";

export const HelpPttDiagram = memo(() => {
  return (
    <div className="flex flex-col gap-2.5 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Push-To-Talk Workflow
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Hold [Space] to talk
        </span>
      </div>

      {/* SVG Diagram Canvas */}
      <div className="w-full py-1.5 flex items-center justify-between gap-1">
        {/* Step 1: Press & Hold */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="relative flex items-center justify-center w-8 h-8 rounded-full border border-[rgb(var(--accent))]/50 bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] shrink-0">
            <div className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-50" />
            <Mic size={14} />
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">Hold Space</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Mic opens</span>
        </div>

        <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />

        {/* Step 2: Speak */}
        <div className="flex-1 min-w-0 flex flex-col items-center gap-1 text-center">
          <div className="flex items-center justify-center w-8 h-8 rounded-full border border-sky-400/50 bg-sky-400/10 text-sky-400 shrink-0">
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5">
              <path
                d="M 2 12 Q 6 4 10 12 T 18 12 T 22 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                className="animate-pulse"
              />
            </svg>
          </div>
          <span className="text-[10.5px] font-semibold text-[rgb(var(--foreground))] truncate max-w-full">Speak</span>
          <span className="text-[9px] text-[rgb(var(--foreground-muted))] truncate max-w-full">Ask anything</span>
        </div>

        <ArrowRight size={12} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />

        {/* Step 3: Release */}
        <div className="flex-1 min-w-0 flex flex-col gap-1 items-center">
          <div className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-emerald-500/10 border border-emerald-500/25 text-emerald-500 text-[10px] font-medium w-full justify-center">
            <CheckCircle2 size={11} className="shrink-0" />
            <span className="truncate">Release</span>
          </div>
          <div className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-rose-500/10 border border-rose-500/25 text-rose-500 text-[10px] font-medium w-full justify-center">
            <XCircle size={11} className="shrink-0" />
            <span className="truncate">Esc cancels</span>
          </div>
        </div>
      </div>
    </div>
  );
});

HelpPttDiagram.displayName = "HelpPttDiagram";
