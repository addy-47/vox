import { useState, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Database, ShieldAlert, Brain, Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, SegmentedControl, ToggleTile, SliderField } from "@/shared/ui";

interface MemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const MODE_OPTIONS = [
  { id: "history" as const, label: "History" },
  { id: "memory" as const, label: "Memory" },
];

export const MemoryCard = memo(({ layoutMode = "full-max" }: MemoryCardProps) => {
  const { draftSettings, updateDraft } = useSettings();
  const [activeMode, setActiveMode] = useState<"history" | "memory">("history");

  if (!draftSettings) return null;
  const { persistence, memory } = draftSettings;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  return (
    <Card 
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none",
        !isSmall && cn(
          "p-5 min-h-[310px] h-full justify-between transition-all duration-300",
          isMin ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
        )
      )}
    >
      {/* Consolidated Header (Hidden on Mobile) */}
      {!isSmall ? (
        <div className="flex items-center justify-between mb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-1.5 w-full">
          <div className="flex items-center gap-2">
            <Database className="text-[rgb(var(--accent))]" size={15} />
            <span className="text-[11px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Memory & Privacy
            </span>
          </div>

          {/* Mode Switcher in Header */}
          <SegmentedControl options={MODE_OPTIONS} value={activeMode} onChange={setActiveMode} size="sm" />
        </div>
      ) : (
        /* Mobile Layout Header */
        <div className="flex items-center justify-between mb-4 w-full shrink-0">
          <span className="text-[12px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/80">
            {activeMode === "history" ? "History Settings" : "Memory settings"}
          </span>
          <SegmentedControl options={MODE_OPTIONS} value={activeMode} onChange={setActiveMode} size="sm" />
        </div>
      )}

      {/* Decoupled Panel Contents */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-1">
        {activeMode === "history" ? (
          /* HISTORY & STORAGE MODE */
          <div key="history-panel" className="flex-1 flex flex-col justify-between min-h-0 py-0.5 gap-2.5">
            {/* Hover slide-out button for Private mode */}
            <div key="private-wrapper-box" className="group flex items-center w-full h-[58px] relative shrink-0">
              <div 
                onClick={() => updateDraft("persistence", "private_mode", !persistence.private_mode)}
                className={cn(
                  "flex-1 p-2.5 rounded-xl group-hover:rounded-r-none border transition-all duration-300 flex flex-col justify-between h-full cursor-pointer min-w-0",
                  persistence.private_mode 
                    ? "border-rose-500/25 bg-rose-500/5 hover:border-rose-500/35 hover:bg-rose-500/10"
                    : "border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)]"
                )}
              >
                <div className="flex items-center justify-between gap-1.5 leading-none">
                  <span className="text-[10px] font-black tracking-widest text-[rgb(var(--foreground-muted))]/60 whitespace-nowrap uppercase">
                    Incognito Mode
                  </span>
                  <ShieldAlert size={13} className={persistence.private_mode ? "text-rose-400 animate-pulse" : "text-[rgb(var(--foreground-muted))]/40"} />
                </div>
                
                <div className="flex items-end justify-between leading-none mt-1">
                  <span className={cn("text-[12px] font-black transition-colors truncate capitalize", persistence.private_mode ? "text-rose-400" : "text-[rgb(var(--foreground))]/90 group-hover:text-[rgb(var(--accent))]" )}>
                    {persistence.private_mode ? "Incognito Active" : "Logging Active"}
                  </span>
                  
                  <div className="w-2.5 h-2.5 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative shrink-0">
                    {persistence.private_mode && (
                      <span className="absolute inset-0 rounded-full border border-rose-500 animate-ping opacity-60" />
                    )}
                    <span className={cn("w-1 h-1 rounded-full", persistence.private_mode ? "bg-rose-400" : "bg-[rgb(var(--foreground-muted))]/40")} />
                  </div>
                </div>
              </div>

              <div 
                onClick={() => updateDraft("persistence", "private_mode", !persistence.private_mode)}
                className="h-full w-0 group-hover:w-[32px] opacity-0 group-hover:opacity-100 flex items-center justify-center bg-[rgba(var(--accent),0.05)] border border-transparent border-l-transparent group-hover:border-[rgba(var(--accent),0.15)] group-hover:border-l-transparent rounded-r-xl transition-all duration-300 overflow-hidden cursor-pointer select-none shrink-0"
              >
                <span className="text-[7.5px] font-black uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
                  TOGGLE
                </span>
              </div>
            </div>

            {/* Sliders */}
            <div className="flex-1 flex flex-col justify-end gap-2.5 pb-0.5">
              <SliderField
                label="Capacity limit"
                value={persistence.max_sessions}
                min={5}
                max={500}
                step={5}
                onChange={(v) => updateDraft("persistence", "max_sessions", v)}
              />
              <SliderField
                label="Retention days"
                value={persistence.retention_days}
                min={1}
                max={365}
                step={1}
                formatValue={(v) => `${v}d`}
                onChange={(v) => updateDraft("persistence", "retention_days", v)}
              />
            </div>
          </div>
        ) : (
          /* COGNITIVE RAG & MEMORY MODE */
          <div key="memory-panel" className="flex-1 flex flex-col justify-between min-h-0 py-0.5 gap-2.5">
            {/* Toggles Side-by-Side (Episodic Recall & Auto Sweeper) */}
            <div key="cognitive-toggles-row" className="flex gap-2.5 w-full shrink-0">
              <ToggleTile
                title="Episodic RAG"
                active={memory.episodic_enabled}
                activeLabel="Recall"
                inactiveLabel="Disabled"
                icon={Brain}
                onToggle={() => updateDraft("memory", "episodic_enabled", !memory.episodic_enabled)}
                layoutMode={layoutMode}
                className="h-[56px] flex-1"
              />

              <ToggleTile
                title="Auto Sweep"
                active={memory.bg_worker_enabled}
                activeLabel="Sweeping"
                inactiveLabel="Stopped"
                icon={Cpu}
                onToggle={() => updateDraft("memory", "bg_worker_enabled", !memory.bg_worker_enabled)}
                layoutMode={layoutMode}
                className="h-[56px] flex-1"
              />
            </div>

            {/* Sliders Column */}
            <div className="flex-1 flex flex-col justify-end gap-2.5 pb-0.5">
              <SliderField
                label="Recall depth"
                value={memory.top_k}
                min={1}
                max={10}
                step={1}
                onChange={(v) => updateDraft("memory", "top_k", v)}
              />
              <div className="flex gap-4 min-w-0">
                <SliderField
                  label="Min Sim"
                  value={memory.similarity_threshold}
                  min={0.10}
                  max={0.95}
                  step={0.05}
                  formatValue={(v) => `${Math.round(v * 100)}%`}
                  onChange={(v) => updateDraft("memory", "similarity_threshold", v)}
                  className="flex-1 min-w-0"
                />
                <SliderField
                  label="Budget"
                  value={memory.max_context_share}
                  min={0.05}
                  max={0.80}
                  step={0.05}
                  formatValue={(v) => `${Math.round(v * 100)}%`}
                  onChange={(v) => updateDraft("memory", "max_context_share", v)}
                  className="flex-1 min-w-0"
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
});

MemoryCard.displayName = "MemoryCard";

