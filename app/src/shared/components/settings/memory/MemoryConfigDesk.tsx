import { useState, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { MEMORY_CONFIG_DESK_COPY } from "@/data/settingsCopy";

export interface MemoryConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type MemorySubTab = "depth" | "cutoff" | "graph" | "budget" | "window";

const TABS: Array<{ id: MemorySubTab; label: string }> = [
  { id: "depth", label: MEMORY_CONFIG_DESK_COPY.tabs.depth },
  { id: "cutoff", label: MEMORY_CONFIG_DESK_COPY.tabs.cutoff },
  { id: "graph", label: MEMORY_CONFIG_DESK_COPY.tabs.graph },
  { id: "budget", label: MEMORY_CONFIG_DESK_COPY.tabs.budget },
  { id: "window", label: MEMORY_CONFIG_DESK_COPY.tabs.window },
];

export const MemoryConfigDesk = memo(({ layoutMode }: MemoryConfigDeskProps) => {
  const [activeSubTab, setActiveSubTab] = useState<MemorySubTab>("depth");
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const topKFacts = memory?.top_k_facts ?? 5;
  const semanticSimilarityCutoff = memory?.semantic_similarity_cutoff ?? 0.40;
  const maxHops = memory?.max_hops ?? 2;
  const maxContextShare = memory?.max_context_share ?? 0.15;
  const contextChainingWindowHours = memory?.context_chaining_window_hours ?? 12;

  // Custom input handlers with numeric sanitization to prevent weird browser spinner icons
  const handleCustomDepthChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseInt(clean, 10);
      if (!isNaN(val) && val >= 1 && val <= 50) {
        updateDraft("memory", "top_k_facts", val);
      }
    },
    [updateDraft]
  );

  const handleCustomCutoffChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseFloat(clean);
      if (!isNaN(val) && val >= 1 && val <= 99) {
        updateDraft("memory", "semantic_similarity_cutoff", val / 100);
      }
    },
    [updateDraft]
  );

  const handleCustomBudgetChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseFloat(clean);
      if (!isNaN(val) && val >= 1 && val <= 50) {
        updateDraft("memory", "max_context_share", val / 100);
      }
    },
    [updateDraft]
  );

  const handleCustomWindowChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseInt(clean, 10);
      if (!isNaN(val) && val >= 1 && val <= 168) {
        updateDraft("memory", "context_chaining_window_hours", val);
      }
    },
    [updateDraft]
  );

  const copy = MEMORY_CONFIG_DESK_COPY;
  const isSmall = layoutMode === "small";

  const isDepthCustom = ![3, 5, 8].includes(topKFacts);
  const cutoffPct = Math.round(semanticSimilarityCutoff * 100);
  const isCutoffCustom = ![25, 40, 70].includes(cutoffPct);
  const budgetPct = Math.round(maxContextShare * 100);
  const isBudgetCustom = ![10, 15, 25].includes(budgetPct);
  const isWindowCustom = ![6, 12, 24].includes(contextChainingWindowHours);

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* ─── Layer 1: Subtab Navigation (Full-Width Distributed Tabs) ─── */}
      <div className="w-full flex items-center justify-between pt-0.5 pb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5">
        {TABS.map((tab, idx, arr) => {
          const isActive = activeSubTab === tab.id;
          return (
            <div key={tab.id} className="flex-1 flex items-center justify-center">
              <button
                type="button"
                onClick={() => setActiveSubTab(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer text-center",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span>{tab.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-1 sm:px-2">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* ─── Layer 2: Subtab Workspace (HistoryCard Side-by-Side Ergonomics with 2x2 Grids) ─── */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          isSmall ? "h-auto py-1" : "h-[128px] max-h-[128px]"
        )}
      >
        {/* TAB 1: DEPTH (FACT LIMIT) - 2x2 Grid with Presets + Custom Input */}
        {activeSubTab === "depth" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.depth.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {topKFacts} {copy.depth.unit}
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.depth.description}
              </p>
            </div>

            {/* 2x2 Grid: [3, 5, 8, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[100px] sm:w-[116px]">
              {[3, 5, 8].map((k) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => updateDraft("memory", "top_k_facts", k)}
                  className={cn(
                    "py-1 rounded-lg border text-[11.5px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                    topKFacts === k && !isDepthCustom
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {k}
                </button>
              ))}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  isDepthCustom
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={isDepthCustom ? topKFacts : ""}
                  onChange={handleCustomDepthChange}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}

        {/* TAB 2: CUTOFF (RELEVANCE THRESHOLD) - 2x2 Grid with Presets + Custom Input */}
        {activeSubTab === "cutoff" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.cutoff.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {cutoffPct}%
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.cutoff.description}
              </p>
            </div>

            {/* 2x2 Grid: [25%, 40%, 70%, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[100px] sm:w-[116px]">
              {[25, 40, 70].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => updateDraft("memory", "semantic_similarity_cutoff", pct / 100)}
                  className={cn(
                    "py-1 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
                    cutoffPct === pct && !isCutoffCustom
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {pct}%
                </button>
              ))}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  isCutoffCustom
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={isCutoffCustom ? cutoffPct : ""}
                  onChange={handleCustomCutoffChange}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}

        {/* TAB 3: GRAPH (MAX HOPS) - 2x2 Grid with [1, 2, 3, 4] Hops */}
        {activeSubTab === "graph" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.graph.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {maxHops} {maxHops === 1 ? "Hop" : "Hops"}
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.graph.description}
              </p>
            </div>

            {/* 2x2 Grid: [1, 2, 3, 4] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[130px]">
              {[1, 2, 3, 4].map((h) => (
                <button
                  key={h}
                  type="button"
                  onClick={() => updateDraft("memory", "max_hops", h)}
                  className={cn(
                    "py-1 px-1 rounded-lg border text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-0.5",
                    maxHops === h
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{h}</span>
                  <span className="text-[9px] font-normal uppercase opacity-75">{h === 1 ? "Hop" : "Hops"}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* TAB 4: BUDGET (MAX CONTEXT SHARE) - 2x2 Grid with Presets + Custom Input */}
        {activeSubTab === "budget" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.budget.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {budgetPct}%
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.budget.description}
              </p>
            </div>

            {/* 2x2 Grid: [10%, 15%, 25%, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[100px] sm:w-[116px]">
              {[10, 15, 25].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => updateDraft("memory", "max_context_share", pct / 100)}
                  className={cn(
                    "py-1 rounded-lg text-[11px] font-mono font-bold transition-all cursor-pointer flex items-center justify-center border",
                    budgetPct === pct && !isBudgetCustom
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {pct}%
                </button>
              ))}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  isBudgetCustom
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={isBudgetCustom ? budgetPct : ""}
                  onChange={handleCustomBudgetChange}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}

        {/* TAB 5: WINDOW (CONTEXT CHAINING HOURS) - 2x2 Grid with Presets + Custom Input */}
        {activeSubTab === "window" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.window.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {contextChainingWindowHours}h
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.window.description}
              </p>
            </div>

            {/* 2x2 Grid: [6h, 12h, 24h, Custom] */}
            <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[100px] sm:w-[116px]">
              {[6, 12, 24].map((hours) => (
                <button
                  key={hours}
                  type="button"
                  onClick={() => updateDraft("memory", "context_chaining_window_hours", hours)}
                  className={cn(
                    "py-1 rounded-lg text-[10.5px] font-mono transition-all cursor-pointer text-center border font-bold",
                    contextChainingWindowHours === hours && !isWindowCustom
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.18)] text-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  {hours}h
                </button>
              ))}
              <div
                className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  isWindowCustom
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={isWindowCustom ? contextChainingWindowHours : ""}
                  onChange={handleCustomWindowChange}
                  placeholder="Custom"
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

MemoryConfigDesk.displayName = "MemoryConfigDesk";
