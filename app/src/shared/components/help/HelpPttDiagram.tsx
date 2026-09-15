import { memo } from "react";
import { motion } from "framer-motion";
import { Mic, ArrowRight, CheckCircle2, XCircle } from "lucide-react";

export const HelpPttDiagram = memo(() => {
  return (
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Push-To-Talk Workflow
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Hold [Space] to talk
        </span>
      </div>

      {/* SVG Diagram Canvas */}
      <div className="relative w-full h-32 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] p-3 flex items-center justify-around">
        {/* Step 1: Press & Hold */}
        <div className="flex flex-col items-center gap-1 text-center">
          <div className="relative flex items-center justify-center w-9 h-9 rounded-full border border-[rgb(var(--accent))]/50 bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))]">
            <motion.div
              animate={{ scale: [1, 1.25, 1], opacity: [0.6, 0, 0.6] }}
              transition={{ duration: 1.8, repeat: Infinity, ease: "easeOut" }}
              className="absolute inset-0 rounded-full border border-[rgb(var(--accent))]"
            />
            <Mic size={16} />
          </div>
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))]">1. Hold Space</span>
          <span className="text-[10px] text-[rgb(var(--foreground-muted))]">Mic is open</span>
        </div>

        <ArrowRight size={14} className="text-[rgb(var(--foreground-muted))]/40" />

        {/* Step 2: Speak */}
        <div className="flex flex-col items-center gap-1 text-center">
          <div className="flex items-center justify-center w-9 h-9 rounded-full border border-sky-400/50 bg-sky-400/10 text-sky-400">
            <svg viewBox="0 0 24 24" className="w-4 h-4">
              <motion.path
                d="M 2 12 Q 6 4 10 12 T 18 12 T 22 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                animate={{ d: [
                  "M 2 12 Q 6 4 10 12 T 18 12 T 22 12",
                  "M 2 12 Q 6 18 10 12 T 18 6 T 22 12",
                  "M 2 12 Q 6 4 10 12 T 18 12 T 22 12",
                ] }}
                transition={{ duration: 1.4, repeat: Infinity, ease: "easeInOut" }}
              />
            </svg>
          </div>
          <span className="text-[11px] font-bold text-[rgb(var(--foreground))]">2. Speak</span>
          <span className="text-[10px] text-[rgb(var(--foreground-muted))]">Ask anything</span>
        </div>

        <ArrowRight size={14} className="text-[rgb(var(--foreground-muted))]/40" />

        {/* Step 3: Release */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-emerald-500/10 border border-emerald-500/25 text-emerald-500 text-[11px] font-medium">
            <CheckCircle2 size={12} className="shrink-0" />
            <span><strong>Release</strong> to send</span>
          </div>
          <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-rose-500/10 border border-rose-500/25 text-rose-500 text-[11px] font-medium">
            <XCircle size={12} className="shrink-0" />
            <span><strong>Press Esc</strong> to cancel</span>
          </div>
        </div>
      </div>
    </div>
  );
});

HelpPttDiagram.displayName = "HelpPttDiagram";
