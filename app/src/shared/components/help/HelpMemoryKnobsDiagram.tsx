import { memo, useState } from "react";
import { Database, Filter, GitFork, PieChart, Clock } from "lucide-react";
import { cn } from "@/shared/lib/utils";

type KnobId = "depth" | "cutoff" | "graph" | "budget" | "window";

interface KnobItem {
  id: KnobId;
  label: string;
  icon: typeof Database;
  param: string;
  desc: string;
}

const KNOBS: KnobItem[] = [
  { id: "depth", label: "Depth", icon: Database, param: "5 facts", desc: "How many relevant past memories Vox retrieves for each question." },
  { id: "cutoff", label: "Relevance", icon: Filter, param: "40% cutoff", desc: "Filters out past memories that don't closely match your current topic." },
  { id: "graph", label: "Links", icon: GitFork, param: "2 hops", desc: "Allows Vox to connect related concepts across multiple past conversations." },
  { id: "budget", label: "Budget", icon: PieChart, param: "15% max", desc: "Limits memory size so your active chat remains fast and responsive." },
  { id: "window", label: "Window", icon: Clock, param: "12 hours", desc: "Keeps conversation context warm when you return within this timeframe." },
];

const KNOB_MAP: Record<KnobId, KnobItem> = {
  depth: KNOBS[0],
  cutoff: KNOBS[1],
  graph: KNOBS[2],
  budget: KNOBS[3],
  window: KNOBS[4],
};

export const HelpMemoryKnobsDiagram = memo(() => {
  const [activeKnob, setActiveKnob] = useState<KnobId>("depth");
  const selected = KNOB_MAP[activeKnob];
  const SelectedIcon = selected.icon;

  return (
    <div className="flex flex-col gap-2 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Memory Tuning Options
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Parameter Guide
        </span>
      </div>

      {/* Knobs Selector */}
      <div className="flex flex-wrap gap-1 pt-0.5">
        {KNOBS.map((k) => {
          const Icon = k.icon;
          const isSel = k.id === activeKnob;
          return (
            <button
              key={k.id}
              type="button"
              onClick={() => setActiveKnob(k.id)}
              className={cn(
                "flex items-center gap-1.5 py-1 px-2.5 rounded-lg text-[11px] font-medium transition-colors cursor-pointer border",
                isSel
                  ? "bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.3)] font-semibold shadow-xs"
                  : "border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.25)]"
              )}
            >
              <Icon size={12} className={isSel ? "text-[rgb(var(--accent))]" : "opacity-60"} />
              <span>{k.label}</span>
            </button>
          );
        })}
      </div>

      {/* Knob Explainer Box */}
      <div className="w-full rounded-xl border border-[rgba(var(--border),0.14)] p-3 flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <SelectedIcon size={14} className="text-[rgb(var(--accent))]" />
            <span className="text-[12.5px] font-semibold text-[rgb(var(--foreground))]">{selected.label} Setting</span>
          </div>
          <span className="text-[10.5px] font-mono text-[rgb(var(--accent))] font-semibold">
            {selected.param}
          </span>
        </div>
        <p className="text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]">
          {selected.desc}
        </p>
      </div>
    </div>
  );
});

HelpMemoryKnobsDiagram.displayName = "HelpMemoryKnobsDiagram";
