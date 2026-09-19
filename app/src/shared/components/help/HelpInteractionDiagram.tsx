import React, { memo, useState } from "react";
import { Mic, Radio, Keyboard, VolumeX, Sparkles, ArrowRight, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";

type InteractionTab = "continuous" | "ptt" | "dictation";

interface FlowStep {
  icon: React.ComponentType<{ size?: number; className?: string }>;
  colorClass: string;
  title: string;
  sub: string;
}

interface FlowData {
  steps: FlowStep[];
  desc: string;
}

const FLOWS: Record<InteractionTab, FlowData> = {
  continuous: {
    steps: [
      { icon: Radio, colorClass: "border-sky-400/30 bg-sky-400/10 text-sky-400", title: "Always Ready", sub: "Listens for voice" },
      { icon: Sparkles, colorClass: "border-cyan-400/30 bg-cyan-400/10 text-cyan-400", title: "You Speak", sub: "Auto transcribes" },
      { icon: VolumeX, colorClass: "border-amber-400/30 bg-amber-400/10 text-amber-400", title: "You Pause", sub: "Sends reply" },
    ],
    desc: "Speak freely without holding any buttons. Vox automatically replies when you finish speaking.",
  },
  ptt: {
    steps: [
      { icon: Keyboard, colorClass: "border-amber-400/30 bg-amber-400/10 text-amber-400", title: "Hold Space", sub: "Mic opens" },
      { icon: Mic, colorClass: "border-cyan-400/30 bg-cyan-400/10 text-cyan-400", title: "Speak", sub: "Record voice" },
      { icon: Check, colorClass: "border-emerald-400/30 bg-emerald-400/10 text-emerald-400", title: "Release Key", sub: "Sends reply" },
    ],
    desc: "Complete control. Only records when Space is held, preventing background noise from triggering unwanted answers.",
  },
  dictation: {
    steps: [
      { icon: Keyboard, colorClass: "border-purple-400/30 bg-purple-400/10 text-purple-400", title: "Shortcut", sub: "In any app" },
      { icon: Mic, colorClass: "border-cyan-400/30 bg-cyan-400/10 text-cyan-400", title: "Speak", sub: "Speech to text" },
      { icon: Sparkles, colorClass: "border-emerald-400/30 bg-emerald-400/10 text-emerald-400", title: "Auto Types", sub: "At active cursor" },
    ],
    desc: "Dictate text into your code editor, browser, or documents using Vox's voice recognition.",
  },
};

const TABS: { id: InteractionTab; label: string; icon: React.ComponentType<{ size?: number; className?: string }> }[] = [
  { id: "continuous", label: "Hands-Free", icon: Radio },
  { id: "ptt", label: "Push-to-Talk", icon: Mic },
  { id: "dictation", label: "Dictation", icon: Keyboard },
];

export const HelpInteractionDiagram = memo(() => {
  const [activeTab, setActiveTab] = useState<InteractionTab>("continuous");
  const flow = FLOWS[activeTab];

  return (
    <div className="flex flex-col gap-2 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Speaking Modes Compared
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Preview Flow
        </span>
      </div>

      {/* Mode Selector Tabs */}
      <div className="flex items-center gap-1.5 pt-0.5">
        {TABS.map((tab) => {
          const TabIcon = tab.icon;
          const isSel = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "flex items-center gap-1.5 py-1 px-2.5 rounded-lg text-[11.5px] font-medium transition-colors cursor-pointer border",
                isSel
                  ? "bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.3)] font-semibold shadow-xs"
                  : "border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.25)]"
              )}
            >
              <TabIcon size={13} className={isSel ? "text-[rgb(var(--accent))]" : "opacity-60"} />
              <span>{tab.label}</span>
            </button>
          );
        })}
      </div>

      {/* Interactive Flow Box */}
      <div className="w-full rounded-xl border border-[rgba(var(--border),0.14)] p-3 flex flex-col gap-2.5">
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between gap-1 text-center py-1 px-1">
            {flow.steps.map((step, idx) => {
              const StepIcon = step.icon;
              return (
                <React.Fragment key={step.title}>
                  {idx > 0 && <ArrowRight size={13} className="text-[rgb(var(--foreground-muted))]/40 shrink-0" />}
                  <div className="flex flex-col items-center gap-1 w-20">
                    <div className={cn("w-8 h-8 rounded-full border flex items-center justify-center", step.colorClass)}>
                      <StepIcon size={14} />
                    </div>
                    <span className="text-[11px] font-semibold text-[rgb(var(--foreground))]">{step.title}</span>
                    <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]">{step.sub}</span>
                  </div>
                </React.Fragment>
              );
            })}
          </div>
          <p className="text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))] pt-1 border-t border-[rgba(var(--border),0.06)]">
            {flow.desc}
          </p>
        </div>
      </div>
    </div>
  );
});

HelpInteractionDiagram.displayName = "HelpInteractionDiagram";
