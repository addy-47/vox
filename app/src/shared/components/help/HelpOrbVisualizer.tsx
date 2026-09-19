import { useState, memo } from "react";
import { Sparkles, Mic, Brain, Volume2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";

type Mood = "Idle" | "Listening" | "Thinking" | "Speaking";

const MOOD_DATA: Record<Mood, { label: string; icon: typeof Sparkles; color: string; desc: string }> = {
  Idle: {
    label: "Idle",
    icon: Sparkles,
    color: "rgb(var(--accent))",
    desc: "Ready and waiting. Hold Spacebar or start speaking to begin.",
  },
  Listening: {
    label: "Listening",
    icon: Mic,
    color: "#38bdf8",
    desc: "Vox is actively listening to your voice.",
  },
  Thinking: {
    label: "Thinking",
    icon: Brain,
    color: "#a78bfa",
    desc: "Vox is thinking and preparing your answer.",
  },
  Speaking: {
    label: "Speaking",
    icon: Volume2,
    color: "#34d399",
    desc: "Vox is speaking. You can interrupt anytime by talking.",
  },
};

export const HelpOrbVisualizer = memo(() => {
  const [activeMood, setActiveMood] = useState<Mood>("Listening");
  const data = MOOD_DATA[activeMood];

  return (
    <div className="flex flex-col gap-2.5 py-1">
      {/* Header */}
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Visual Orb Moods
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Click state to preview
        </span>
      </div>

      {/* Canvas Area - Light/Dark Theme Respecting */}
      <div className="relative w-full h-36 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] overflow-hidden flex items-center justify-center">
        {/* Ambient background glow */}
        <div
          className="absolute w-24 h-24 rounded-full blur-2xl pointer-events-none transition-colors duration-500 opacity-25 animate-pulse"
          style={{ backgroundColor: data.color }}
        />

        {/* Animated Vector Shapes */}
        <svg viewBox="0 0 160 160" className="w-32 h-32 relative z-10">
          <defs>
            <radialGradient id="orbCoreGlow" cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor={data.color} stopOpacity="0.9" />
              <stop offset="60%" stopColor={data.color} stopOpacity="0.35" />
              <stop offset="100%" stopColor={data.color} stopOpacity="0" />
            </radialGradient>
          </defs>

          {/* Dynamic Rings depending on mood */}
          {activeMood === "Idle" && (
            <g>
              <circle cx="80" cy="80" r="28" fill="url(#orbCoreGlow)" />
              <circle
                cx="80"
                cy="80"
                r="34"
                stroke={data.color}
                strokeWidth="1.2"
                strokeOpacity="0.5"
                fill="none"
                className="animate-pulse"
                style={{ transformOrigin: "80px 80px" }}
              />
            </g>
          )}

          {activeMood === "Listening" && (
            <g>
              <circle
                cx="80"
                cy="80"
                r="26"
                stroke={data.color}
                strokeWidth="1.5"
                fill="none"
                className="animate-ping opacity-60"
                style={{ transformOrigin: "80px 80px" }}
              />
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}

          {activeMood === "Thinking" && (
            <g>
              <g
                className="animate-spin origin-center"
                style={{ transformOrigin: "80px 80px", animationDuration: "5s" }}
              >
                <circle cx="80" cy="42" r="3" fill={data.color} />
                <circle cx="118" cy="80" r="2.5" fill={data.color} opacity="0.7" />
                <circle cx="80" cy="118" r="3" fill={data.color} opacity="0.8" />
                <circle cx="42" cy="80" r="2" fill={data.color} opacity="0.6" />
                <circle cx="80" cy="80" r="38" stroke={data.color} strokeWidth="1" strokeDasharray="4 6" fill="none" opacity="0.4" />
              </g>
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}

          {activeMood === "Speaking" && (
            <g>
              <circle
                cx="80"
                cy="80"
                r="32"
                stroke={data.color}
                strokeWidth="1.8"
                strokeDasharray="14 7"
                fill="none"
                className="animate-spin origin-center"
                style={{ transformOrigin: "80px 80px", animationDuration: "6s", animationDirection: "reverse" }}
              />
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}
        </svg>

        {/* Floating status pill */}
        <div className="absolute bottom-2 px-3 py-0.5 rounded-full bg-[rgba(var(--card),0.95)] border border-[rgba(var(--border),0.15)] shadow-sm flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full animate-pulse" style={{ backgroundColor: data.color }} />
          <span className="text-[10.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
            {activeMood}
          </span>
        </div>
      </div>

      {/* State Switcher Tabs */}
      <div className="flex flex-wrap gap-1.5 pt-0.5">
        {(["Idle", "Listening", "Thinking", "Speaking"] as Mood[]).map((m) => {
          const item = MOOD_DATA[m];
          const Icon = item.icon;
          const isSelected = activeMood === m;

          return (
            <button
              key={m}
              type="button"
              onClick={() => setActiveMood(m)}
              className={cn(
                "flex items-center gap-1.5 py-1 px-2.5 rounded-lg text-[11.5px] font-medium transition-colors cursor-pointer border",
                isSelected
                  ? "bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.3)] font-semibold shadow-xs"
                  : "border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.25)]"
              )}
            >
              <Icon size={12} style={{ color: isSelected ? item.color : undefined }} className={isSelected ? "" : "opacity-60"} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>

      {/* Explanatory description */}
      <p className="text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]">
        {data.desc}
      </p>
    </div>
  );
});

HelpOrbVisualizer.displayName = "HelpOrbVisualizer";
