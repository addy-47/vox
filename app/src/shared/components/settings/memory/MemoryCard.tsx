import { useState, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Database, Brain, Cpu, Plus, Minus, Clock, PieChart, GitFork } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile, RotaryKnob, SegmentedControl } from "@/shared/ui";

interface MemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const VIEW_OPTIONS = [
  { id: "retrieval" as const, label: "Retrieval" },
  { id: "pipeline" as const, label: "Pipeline" },
];

const SimilarityCutoffKnobSection = memo(() => {
  const semanticSimilarityCutoff = useSettingsStore(
    (s) => s.draftSettings?.memory?.semantic_similarity_cutoff ?? 0.40
  );
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const handleChange = useCallback(
    (v: number) => {
      updateDraft("memory", "semantic_similarity_cutoff", v);
    },
    [updateDraft]
  );

  return (
    <RotaryKnob
      label="Similarity Floor"
      value={semanticSimilarityCutoff}
      min={0.10}
      max={0.90}
      step={0.05}
      defaultValue={0.40}
      formatValue={(v) => `${Math.round(v * 100)}%`}
      formatPreset={(v) => `${Math.round(v * 100)}%`}
      presetSteps={[0.25, 0.40, 0.60, 0.75]}
      onChange={handleChange}
    />
  );
});

SimilarityCutoffKnobSection.displayName = "SimilarityCutoffKnobSection";

export const MemoryCard = memo(({ layoutMode = "full-max" }: MemoryCardProps) => {
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [activeTab, setActiveTab] = useState<"retrieval" | "pipeline">("retrieval");

  if (!memory) return null;

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  // All 7 backend memory settings fields
  const contextRetrievalEnabled = memory.context_retrieval_enabled ?? true;
  const pipelineProcessingEnabled = memory.pipeline_processing_enabled ?? true;
  const maxPersonalMemoryShare = memory.max_personal_memory_share ?? 0.15;
  const contextChainingWindowHours = memory.context_chaining_window_hours ?? 12;
  const topKFacts = memory.top_k_facts ?? 5;
  const maxHops = memory.max_hops ?? 2;

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none transform-gpu",
        !isSmall && cn(
          "p-5 lg:h-[340px] justify-between transition-all duration-300",
          isMin ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
        )
      )}
    >
      {/* Header with Top-Right Sub-Desk Switcher */}
      {!isSmall ? (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Database className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[13px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Memory Stack
            </span>
          </div>

          <SegmentedControl
            options={VIEW_OPTIONS}
            value={activeTab}
            onChange={setActiveTab}
            size="sm"
          />
        </div>
      ) : (
        <div className="flex items-center justify-between mb-4 w-full shrink-0">
          <span className="text-[13px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/80">
            Memory Settings
          </span>
          <SegmentedControl
            options={VIEW_OPTIONS}
            value={activeTab}
            onChange={setActiveTab}
            size="sm"
          />
        </div>
      )}

      {/* Main Body */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-1 gap-3 overflow-y-auto custom-scrollbar">
        {activeTab === "retrieval" ? (
          /* TAB 1: RETRIEVAL (Readable Glass Chips on Left | Radial Knob on Right) */
          <div key="retrieval-tab" className="flex-1 flex flex-col justify-between min-h-0 gap-3">
            {/* Toggle 1: Context Retrieval */}
            <ToggleTile
              title="Episodic RAG Recall"
              active={contextRetrievalEnabled}
              activeLabel="Recall Active"
              inactiveLabel="Recall Disabled"
              activeSublabel="Context Injected"
              inactiveSublabel="Turn Bypassed"
              icon={Brain}
              onToggle={() =>
                updateDraft("memory", "context_retrieval_enabled", !contextRetrievalEnabled)
              }
              layoutMode={layoutMode}
            />

            {/* Lower Section: Stacked Tiles on Left, Radial Knob on Right (Stacked in small screens) */}
            <div className={cn(
              "flex-1 flex min-w-0 pt-1 gap-4",
              isSmall ? "flex-col items-stretch" : "flex-row items-center justify-between gap-5"
            )}>
              {/* LEFT SIDE: Stacked Sections for Recall Depth & Max Hops */}
              <div className={cn(
                "flex-1 flex flex-col justify-center gap-3.5 min-w-0",
                !isSmall && "pr-2"
              )}>
                {/* 1. Recall Depth (Top-K) Tile */}
                <div className="p-3 rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.01)] flex flex-col gap-2">
                  <div className="flex items-center justify-between flex-wrap gap-1">
                    <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] whitespace-nowrap">
                      Recall Depth
                    </span>
                    <span className="text-[12px] font-mono font-black text-[rgb(var(--accent))] whitespace-nowrap">
                      {topKFacts} facts
                    </span>
                  </div>
                  <div className="flex gap-2 justify-between">
                    {[3, 5, 8, 12].map((k) => (
                      <button
                        key={k}
                        onClick={() => updateDraft("memory", "top_k_facts", k)}
                        className={cn(
                          "flex-1 py-1.5 rounded-lg border text-[13px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                          topKFacts === k
                            ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                            : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                        )}
                      >
                        {k}
                      </button>
                    ))}
                  </div>
                </div>

                {/* 2. Max Hops Expansion Tile */}
                <div className="p-3 rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.01)] flex flex-col gap-2">
                  <div className="flex items-center justify-between flex-wrap gap-1">
                    <div className="flex items-center gap-1.5 shrink-0">
                      <GitFork size={13} className="text-[rgb(var(--accent))]" />
                      <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] whitespace-nowrap">
                        Max Graph Hops
                      </span>
                    </div>
                    <span className="text-[12px] font-mono font-black text-[rgb(var(--accent))] whitespace-nowrap">
                      {maxHops} {maxHops === 1 ? "Hop" : "Hops"}
                    </span>
                  </div>
                  <div className="flex gap-2 justify-between">
                    {[1, 2, 3, 4].map((h) => (
                      <button
                        key={h}
                        onClick={() => updateDraft("memory", "max_hops", h)}
                        className={cn(
                          "flex-1 py-1.5 rounded-lg border text-[12px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                          maxHops === h
                            ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                            : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                        )}
                      >
                        <span>{h}</span>
                        <span className="text-[11px] font-normal uppercase opacity-75">{h === 1 ? "Hop" : "Hops"}</span>
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* RIGHT SIDE: Dedicated Radial Knob for Similarity Cutoff */}
              <div className={cn(
                "shrink-0 flex items-center justify-center",
                isSmall
                  ? "pt-3 border-t border-[rgba(var(--accent),0.08)] w-full"
                  : "pl-3 border-l border-[rgba(var(--accent),0.08)]"
              )}>
                <SimilarityCutoffKnobSection />
              </div>
            </div>
          </div>
        ) : (
          /* TAB 2: PIPELINE (Responsive Side-by-Side or Stacked Cards) */
          <div key="pipeline-tab" className="flex-1 flex flex-col justify-between min-h-0 gap-3">
            {/* Toggle 2: Background Worker */}
            <ToggleTile
              title="Auto Sweep Pipeline"
              active={pipelineProcessingEnabled}
              activeLabel="Sweeper Running"
              inactiveLabel="Sweeper Stopped"
              activeSublabel="4-Stage Deduplication & NLI"
              inactiveSublabel="Queue Staged Only"
              icon={Cpu}
              onToggle={() =>
                updateDraft("memory", "pipeline_processing_enabled", !pipelineProcessingEnabled)
              }
              layoutMode={layoutMode}
            />

            {/* Lower Section: Side-by-Side in Desktop, Stacked in Small Screen */}
            <div className={cn(
              "flex-1 flex min-w-0 pt-1 gap-4",
              isSmall ? "flex-col items-stretch" : "flex-row items-stretch"
            )}>
              {/* Left Card: Context Budget Glass Pills */}
              <div className="flex-1 p-3.5 rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.01)] flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between pb-1 border-b border-[rgba(var(--accent),0.05)]">
                  <div className="flex items-center gap-1.5">
                    <PieChart size={14} className="text-[rgb(var(--accent))]" />
                    <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] whitespace-nowrap">
                      Context Budget
                    </span>
                  </div>
                  <span className="text-[13px] font-mono font-black text-[rgb(var(--accent))]">
                    {Math.round(maxPersonalMemoryShare * 100)}%
                  </span>
                </div>

                <div className="grid grid-cols-2 gap-2 my-2">
                  {[0.10, 0.15, 0.25, 0.35].map((share) => (
                    <button
                      key={share}
                      onClick={() => updateDraft("memory", "max_personal_memory_share", share)}
                      className={cn(
                        "py-1.5 rounded-lg text-[12px] font-mono font-bold transition-all cursor-pointer flex items-center justify-center",
                        Math.abs(maxPersonalMemoryShare - share) < 0.01
                          ? "border border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                          : "border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      {Math.round(share * 100)}%
                    </button>
                  ))}
                </div>
              </div>

              {/* Right Card: Stepper Ticker Card for Context Chaining Window */}
              <div className="flex-1 p-3.5 rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.01)] flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between pb-1 border-b border-[rgba(var(--accent),0.05)]">
                  <div className="flex items-center gap-1.5">
                    <Clock size={14} className="text-[rgb(var(--accent))]" />
                    <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] whitespace-nowrap">
                      Chaining Window
                    </span>
                  </div>
                  <span className="text-[13px] font-mono font-black text-[rgb(var(--accent))]">
                    {contextChainingWindowHours}h
                  </span>
                </div>

                {/* Stepper Buttons Row */}
                <div className="flex items-center justify-between gap-2 my-2">
                  <button
                    onClick={() => updateDraft("memory", "context_chaining_window_hours", Math.max(1, contextChainingWindowHours - 1))}
                    className="flex-1 py-1.5 rounded-lg border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--foreground),0.03)] hover:bg-[rgba(var(--accent),0.15)] hover:border-[rgb(var(--accent))] hover:text-[rgb(var(--accent))] transition-all flex items-center justify-center cursor-pointer text-[12px] font-bold"
                  >
                    <Minus size={14} />
                  </button>
                  <button
                    onClick={() => updateDraft("memory", "context_chaining_window_hours", Math.min(72, contextChainingWindowHours + 1))}
                    className="flex-1 py-1.5 rounded-lg border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--foreground),0.03)] hover:bg-[rgba(var(--accent),0.15)] hover:border-[rgb(var(--accent))] hover:text-[rgb(var(--accent))] transition-all flex items-center justify-center cursor-pointer text-[12px] font-bold"
                  >
                    <Plus size={14} />
                  </button>
                </div>

                {/* Quick Presets */}
                <div className="flex items-center justify-between gap-1 pt-0.5">
                  {[6, 12, 24, 48].map((hours) => (
                    <button
                      key={hours}
                      onClick={() => updateDraft("memory", "context_chaining_window_hours", hours)}
                      className={cn(
                        "flex-1 py-1 rounded-md text-[11px] font-mono transition-all cursor-pointer text-center",
                        contextChainingWindowHours === hours
                          ? "border border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] font-bold shadow-[0_0_10px_rgba(var(--accent),0.25)]"
                          : "border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--accent),0.05)] text-[rgb(var(--foreground-muted))]/80 hover:bg-[rgba(var(--accent),0.12)] hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      {hours}h
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
});

MemoryCard.displayName = "MemoryCard";
