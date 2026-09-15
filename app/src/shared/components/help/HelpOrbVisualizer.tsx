import { useState, memo } from "react";
import { motion } from "framer-motion";
import { Sparkles, Mic, Brain, Volume2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";

type Mood = "Idle" | "Listening" | "Thinking" | "Speaking";

const MOOD_DATA: Record<Mood, { label: string; icon: any; color: string; desc: string }> = {
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
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      {/* Header */}
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Visual Orb Moods
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Click a state to preview
        </span>
      </div>

      {/* Canvas Area - Light/Dark Theme Respecting */}
      <div className="relative w-full h-36 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] overflow-hidden flex items-center justify-center">
        {/* Ambient background glow */}
        <motion.div
          animate={{
            backgroundColor: data.color,
            opacity: [0.15, 0.28, 0.15],
            scale: [0.95, 1.05, 0.95],
          }}
          transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
          className="absolute w-24 h-24 rounded-full blur-2xl pointer-events-none"
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
              <motion.circle
                cx="80"
                cy="80"
                r="34"
                stroke={data.color}
                strokeWidth="1.2"
                strokeOpacity="0.5"
                fill="none"
                animate={{ scale: [1, 1.08, 1], opacity: [0.3, 0.7, 0.3] }}
                transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
                style={{ transformOrigin: "80px 80px" }}
              />
            </g>
          )}

          {activeMood === "Listening" && (
            <g>
              <motion.circle
                cx="80"
                cy="80"
                r="22"
                stroke={data.color}
                strokeWidth="1.5"
                fill="none"
                animate={{ r: [22, 54], opacity: [0.85, 0] }}
                transition={{ duration: 1.8, repeat: Infinity, ease: "easeOut" }}
              />
              <motion.circle
                cx="80"
                cy="80"
                r="22"
                stroke={data.color}
                strokeWidth="1.5"
                fill="none"
                animate={{ r: [22, 54], opacity: [0.85, 0] }}
                transition={{ duration: 1.8, delay: 0.6, repeat: Infinity, ease: "easeOut" }}
              />
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}

          {activeMood === "Thinking" && (
            <g>
              <motion.g
                animate={{ rotate: 360 }}
                transition={{ duration: 4, repeat: Infinity, ease: "linear" }}
                style={{ transformOrigin: "80px 80px" }}
              >
                <circle cx="80" cy="42" r="3" fill={data.color} />
                <circle cx="118" cy="80" r="2.5" fill={data.color} opacity="0.7" />
                <circle cx="80" cy="118" r="3" fill={data.color} opacity="0.8" />
                <circle cx="42" cy="80" r="2" fill={data.color} opacity="0.6" />
                <circle cx="80" cy="80" r="38" stroke={data.color} strokeWidth="1" strokeDasharray="4 6" fill="none" opacity="0.4" />
              </motion.g>
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}

          {activeMood === "Speaking" && (
            <g>
              <motion.circle
                cx="80"
                cy="80"
                r="32"
                stroke={data.color}
                strokeWidth="1.8"
                strokeDasharray="14 7"
                fill="none"
                animate={{ rotate: -360, scale: [0.95, 1.06, 0.95] }}
                transition={{ rotate: { duration: 6, repeat: Infinity, ease: "linear" }, scale: { duration: 1.2, repeat: Infinity, ease: "easeInOut" } }}
                style={{ transformOrigin: "80px 80px" }}
              />
              <circle cx="80" cy="80" r="24" fill="url(#orbCoreGlow)" />
            </g>
          )}
        </svg>

        {/* Floating status pill */}
        <div className="absolute bottom-2 px-3 py-0.5 rounded-full bg-[rgba(var(--card),0.9)] border border-[rgba(var(--border),0.15)] shadow-sm backdrop-blur-md flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full animate-pulse" style={{ backgroundColor: data.color }} />
          <span className="text-[10.5px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
            {activeMood}
          </span>
        </div>
      </div>

      {/* State Switcher Tabs */}
      <div className="grid grid-cols-4 gap-1 p-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)]">
        {(["Idle", "Listening", "Thinking", "Speaking"] as Mood[]).map((m) => {
          const item = MOOD_DATA[m];
          const Icon = item.icon;
          const isSelected = activeMood === m;

          return (
            <button
              key={m}
              onClick={() => setActiveMood(m)}
              className={cn(
                "flex flex-col items-center justify-center gap-1 py-1.5 px-1 rounded-lg text-[10.5px] font-medium transition-all cursor-pointer",
                isSelected
                  ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground))] shadow-xs border border-[rgba(var(--accent),0.3)]"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
              )}
            >
              <Icon size={13} style={{ color: isSelected ? item.color : undefined }} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>

      {/* Explanatory description */}
      <p className="text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))] text-center">
        {data.desc}
      </p>
    </div>
  );
});

HelpOrbVisualizer.displayName = "HelpOrbVisualizer";
