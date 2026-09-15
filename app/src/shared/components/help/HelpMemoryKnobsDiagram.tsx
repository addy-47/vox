import { memo, useState } from "react";
import { Database, Filter, GitFork, PieChart, Clock } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export const HelpMemoryKnobsDiagram = memo(() => {
  const [activeKnob, setActiveKnob] = useState<"depth" | "cutoff" | "graph" | "budget" | "window">("depth");

  const KNOBS = [
    { id: "depth" as const, label: "Depth", icon: Database, param: "5 facts", desc: "How many relevant past memories Vox retrieves for each question." },
    { id: "cutoff" as const, label: "Relevance", icon: Filter, param: "40% cutoff", desc: "Filters out past memories that don't closely match your current topic." },
    { id: "graph" as const, label: "Links", icon: GitFork, param: "2 hops", desc: "Allows Vox to connect related concepts across multiple past conversations." },
    { id: "budget" as const, label: "Budget", icon: PieChart, param: "15% max", desc: "Limits memory size so your active chat remains fast and responsive." },
    { id: "window" as const, label: "Window", icon: Clock, param: "12 hours", desc: "Keeps conversation context warm when you return within this timeframe." },
  ];

  const selected = KNOBS.find((k) => k.id === activeKnob)!;

  return (
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Memory Tuning Options
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Click an option to learn more
        </span>
      </div>

      {/* Knobs Pill Selector */}
      <div className="grid grid-cols-5 gap-1 p-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)]">
        {KNOBS.map((k) => {
          const Icon = k.icon;
          const isSel = k.id === activeKnob;
          return (
            <button
              key={k.id}
              onClick={() => setActiveKnob(k.id)}
              className={cn(
                "flex flex-col items-center gap-1 py-1.5 px-1 rounded-lg text-[10.5px] font-medium transition-all cursor-pointer text-center",
                isSel
                  ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.3)]"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <Icon size={13} className={isSel ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]"} />
              <span className="truncate w-full">{k.label}</span>
            </button>
          );
        })}
      </div>

      {/* Knob Explainer Box */}
      <div className="rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] p-3 flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <selected.icon size={15} className="text-[rgb(var(--accent))]" />
            <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">{selected.label} Setting</span>
          </div>
          <span className="px-2 py-0.5 rounded bg-[rgba(var(--accent),0.12)] text-[10px] font-mono font-medium text-[rgb(var(--accent))]">
            {selected.param}
          </span>
        </div>
        <p className="text-[11.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
          {selected.desc}
        </p>
      </div>
    </div>
  );
});

HelpMemoryKnobsDiagram.displayName = "HelpMemoryKnobsDiagram";
