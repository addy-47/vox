import { useState, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { WORKING_MEMORY_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import { WebSearchGlobe } from "./WebSearchGlobe";

export interface WorkingMemoryConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type WorkingMemorySubTab = "web_search" | "content_share";

export const WorkingMemoryConfigDesk = memo(({ layoutMode }: WorkingMemoryConfigDeskProps) => {
  const [activeSubTab, setActiveSubTab] = useState<WorkingMemorySubTab>("web_search");
  const workingMemory = useSettingsStore((s) => s.draftSettings?.working_memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const copy = WORKING_MEMORY_SETTINGS_COPY;
  const isSmall = layoutMode === "small";

  const tabs: Array<{ id: WorkingMemorySubTab; label: string }> = [
    { id: "web_search", label: copy.tabs.webSearch },
    { id: "content_share", label: copy.tabs.contentShare },
  ];

  const maxContextShare = workingMemory?.max_context_share ?? 0.15;
  const budgetPct = Math.round(maxContextShare * 100);
  const isBudgetCustom = ![10, 15, 25].includes(budgetPct);
  const webSearchEnabled = workingMemory?.web_search_enabled ?? true;

  const handleToggleWebSearch = useCallback(() => {
    updateDraft("working_memory", "web_search_enabled", !webSearchEnabled);
  }, [webSearchEnabled, updateDraft]);

  const handleCustomBudgetChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseFloat(clean);
      if (!isNaN(val) && val >= 1 && val <= 50) {
        updateDraft("working_memory", "max_context_share", val / 100);
      }
    },
    [updateDraft]
  );

  if (!workingMemory) return null;

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* Layer 1: Subtab Navigation */}
      <div className="w-full flex items-center justify-between pt-0.5 pb-1.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5 overflow-x-auto no-scrollbar">
        {tabs.map((tab, idx, arr) => {
          const isActive = activeSubTab === tab.id;
          return (
            <div key={tab.id} className="flex-1 min-w-0 flex items-center justify-center">
              <button
                type="button"
                onClick={() => setActiveSubTab(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[9.5px] sm:text-[10.5px] xl:text-[11px] font-black uppercase tracking-[0.04em] sm:tracking-[0.08em] outline-none cursor-pointer text-center truncate px-0.5",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span className="truncate">{tab.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[10px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-0.5 sm:px-1">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* Layer 2: Subtab Workspace */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5",
          isSmall ? "h-auto" : "h-full"
        )}
      >
        {/* TAB 1: WEB SEARCH — title/description left, floating globe toggle right */}
        {activeSubTab === "web_search" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.webSearch.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.webSearch.description}
              </p>
            </div>
            <WebSearchGlobe
              enabled={webSearchEnabled}
              onToggle={handleToggleWebSearch}
              label={webSearchEnabled ? copy.webSearch.enabled : copy.webSearch.disabled}
              ariaLabel={copy.webSearch.toggleAria}
            />
          </div>
        )}

        {/* TAB 2: CONTENT SHARE — title/description left, preset grid right (no fill bar) */}
        {activeSubTab === "content_share" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.contentShare.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.contentShare.description}
              </p>
            </div>

            <div className="shrink-0 grid grid-cols-4 gap-1.5 w-[164px]">
              {[10, 15, 25].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => updateDraft("working_memory", "max_context_share", pct / 100)}
                  className={cn(
                    "min-h-[32px] py-1.5 rounded-lg text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center border",
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
                  "min-h-[32px] rounded-lg border flex items-center justify-center transition-all duration-200 overflow-hidden",
                  isBudgetCustom
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}
              >
                <input
                  type="text"
                  inputMode="numeric"
                  value={isBudgetCustom ? budgetPct : ""}
                  onChange={handleCustomBudgetChange}
                  placeholder={COMPUTE_PROFILE_COPY.custom}
                  className={cn(
                    "w-full text-center text-[11px] font-mono font-bold bg-transparent outline-none py-1.5 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal",
                    isBudgetCustom ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                  )}
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

WorkingMemoryConfigDesk.displayName = "WorkingMemoryConfigDesk";
