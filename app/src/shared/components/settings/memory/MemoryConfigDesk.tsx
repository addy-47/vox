import { useState, memo, useCallback, useEffect, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useSessionStore } from "@/store/sessionStore";
import { cn } from "@/shared/lib/utils";
import { MEMORY_CARD_COPY, CUSTOM_VALUE_COPY } from "@/data/settingsCopy";
import { PrivateModeGlyph } from "./PrivateModeGlyph";

export interface MemoryConfigDeskProps {
  view: "basic" | "advanced";
  layoutMode?: "full-max" | "full-min" | "small";
}

type MemorySubTab =
  | "session_storage"
  | "schedule"
  | "suggestions"
  | "context"
  | "limit"
  | "relevance";

const BASIC_TABS: MemorySubTab[] = ["session_storage", "schedule", "suggestions"];
const ADVANCED_TABS: MemorySubTab[] = ["context", "limit", "relevance"];

const TAB_LABELS: Record<MemorySubTab, string> = {
  session_storage: MEMORY_CARD_COPY.tabs.sessionStorage,
  schedule: MEMORY_CARD_COPY.tabs.schedule,
  suggestions: MEMORY_CARD_COPY.tabs.suggestions,
  context: MEMORY_CARD_COPY.tabs.context,
  limit: MEMORY_CARD_COPY.tabs.limit,
  relevance: MEMORY_CARD_COPY.tabs.relevance,
};

/**
 * Desk for the merged Memory card. The `view` prop selects the tab group:
 * Basic holds the everyday switches and schedule, Advanced holds the dials.
 */
export const MemoryConfigDesk = memo(({ view, layoutMode }: MemoryConfigDeskProps) => {
  const tabs = view === "basic" ? BASIC_TABS : ADVANCED_TABS;
  const [activeSubTab, setActiveSubTab] = useState<MemorySubTab>(tabs[0]);
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const copy = MEMORY_CARD_COPY;
  const isSmall = layoutMode === "small";

  const topKFacts = memory?.top_k_facts ?? 5;
  const semanticSimilarityCutoff = memory?.semantic_similarity_cutoff ?? 0.4;
  const consolidationCadence = memory?.consolidation_cadence ?? "manual";
  const consolidationTime = memory?.consolidation_time ?? "02:00";
  const suggestionPolicy = memory?.suggestion_policy ?? "manual_review";
  const privateMode = memory?.private_mode ?? false;
  const maxContextShare = memory?.max_context_share ?? 0.15;

  const [timeDraft, setTimeDraft] = useState(consolidationTime);

  useEffect(() => {
    setTimeDraft(consolidationTime);
  }, [consolidationTime]);

  // Keep the selection inside the active group when the card toggles views.
  useEffect(() => {
    setActiveSubTab((prev) => (tabs.includes(prev) ? prev : tabs[0]));
  }, [tabs]);

  const handleTimeChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
      setTimeDraft(val);
      if (/^([01]\d|2[0-3]):[0-5]\d$/.test(val)) {
        updateDraft("memory", "consolidation_time", val);
      }
    },
    [updateDraft]
  );

  const handleTimeBlur = useCallback(() => {
    const trimmed = timeDraft.trim();
    if (/^([01]?\d|2[0-3]):([0-5]?\d)$/.test(trimmed)) {
      const [h, m] = trimmed.split(":");
      const clean = `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;
      setTimeDraft(clean);
      updateDraft("memory", "consolidation_time", clean);
    } else if (/^([01]?\d|2[0-3])$/.test(trimmed)) {
      const clean = `${trimmed.padStart(2, "0")}:00`;
      setTimeDraft(clean);
      updateDraft("memory", "consolidation_time", clean);
    } else if (/^([01]\d|2[0-3])([0-5]\d)$/.test(trimmed)) {
      const clean = `${trimmed.slice(0, 2)}:${trimmed.slice(2, 4)}`;
      setTimeDraft(clean);
      updateDraft("memory", "consolidation_time", clean);
    } else {
      setTimeDraft(consolidationTime);
    }
  }, [timeDraft, consolidationTime, updateDraft]);

  const handleTogglePrivateMode = useCallback(() => {
    const next = !privateMode;
    updateDraft("memory", "private_mode", next);
    useSessionStore.getState().setIsTemporarySession(next);
  }, [privateMode, updateDraft]);

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

  const isDepthCustom = ![3, 5, 8].includes(topKFacts);
  const cutoffPct = Math.round(semanticSimilarityCutoff * 100);
  const isCutoffCustom = ![25, 40, 70].includes(cutoffPct);
  const budgetPct = Math.round(maxContextShare * 100);
  const isBudgetCustom = ![10, 15, 25].includes(budgetPct);

  const formatTimeDisplay = useCallback((t: string) => {
    const [hStr = "", mStr = ""] = t.split(":");
    const h = parseInt(hStr, 10);
    const m = mStr || "00";
    if (isNaN(h)) return t;
    const ampm = h >= 12 ? "PM" : "AM";
    const h12 = h % 12 === 0 ? 12 : h % 12;
    return `${h12}:${m} ${ampm}`;
  }, []);

  const timeInputRef = useRef<HTMLInputElement>(null);

  if (!memory) return null;

  const panelShell = "flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in";
  const panelHeading = "flex flex-col gap-1 min-w-0 flex-1";
  // Fixed-width right column: every control lands at the same offset, so
  // switching tabs never shifts the glyph or its right padding.
  const controlColumn = "shrink-0 w-[116px] flex flex-col items-center justify-center";
  const controlGrid = "shrink-0 w-[116px] grid grid-cols-2 gap-1.5";

  const customInput = (
    isCustom: boolean,
    customValue: number | "",
    onCustomChange: (e: React.ChangeEvent<HTMLInputElement>) => void
  ) => (
    <div
      className={cn(
        "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
        isCustom
          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
      )}
    >
      <input
        type="text"
        inputMode="numeric"
        value={customValue}
        onChange={onCustomChange}
        placeholder={CUSTOM_VALUE_COPY.placeholder}
        className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
      />
    </div>
  );

  const presetButtonClass = (isSelected: boolean) =>
    cn(
      "py-1 rounded-lg border text-[11.5px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center",
      isSelected
        ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
        : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
    );

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* Layer 1: Subtab Navigation — equal-width grid so switching never shifts the row */}
      <div
        className={cn(
          "w-full grid gap-1 pt-0.5 pb-1.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5",
          tabs.length === 3 ? "grid-cols-3" : "grid-cols-3"
        )}
      >
        {tabs.map((tab) => {
          const isActive = activeSubTab === tab;
          return (
            <button
              key={tab}
              type="button"
              onClick={() => setActiveSubTab(tab)}
              className={cn(
                "w-full min-w-0 flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[9.5px] sm:text-[10.5px] xl:text-[11px] font-black uppercase tracking-[0.04em] sm:tracking-[0.08em] outline-none cursor-pointer text-center truncate px-0.5",
                isActive
                  ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
              )}
            >
              <span className="truncate">{TAB_LABELS[tab]}</span>
            </button>
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
        {/* TAB: SESSION STORAGE (private mode) */}
        {activeSubTab === "session_storage" && (
          <div className={cn(panelShell, "flex-row items-center justify-between")}>
            <div className={panelHeading}>
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.sessionStorage.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.sessionStorage.description}
              </p>
            </div>
            <div className={controlColumn}>
              <PrivateModeGlyph
                enabled={privateMode}
                onToggle={handleTogglePrivateMode}
                label={privateMode
                  ? copy.sessionStorage.enabledLabel
                  : copy.sessionStorage.disabledLabel}
                ariaLabel={copy.sessionStorage.title}
              />
            </div>
          </div>
        )}

        {/* TAB: SCHEDULE (consolidation cadence + time) */}
        {activeSubTab === "schedule" && (
          <div className={panelShell}>
            <div className={panelHeading}>
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.schedule.title}
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.schedule.description}
              </p>
            </div>

            <div className={controlColumn}>
              <button
                type="button"
                role="switch"
                aria-checked={consolidationCadence === "daily"}
                aria-label={
                  consolidationCadence === "daily"
                    ? copy.schedule.dailyLabel
                    : copy.schedule.manualLabel
                }
                onClick={() =>
                  updateDraft(
                    "memory",
                    "consolidation_cadence",
                    consolidationCadence === "daily" ? "manual" : "daily"
                  )
                }
                className={cn(
                  "group flex flex-col items-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
                  consolidationCadence === "daily"
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
                )}
              >
                <div
                  className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105"
                  style={{
                    animation:
                      consolidationCadence === "daily"
                        ? "wm-globe-float 3.6s ease-in-out infinite"
                        : "none",
                  }}
                >
                  <svg
                    viewBox="0 0 52 52"
                    className="w-full h-full overflow-visible select-none"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <circle
                      cx="26"
                      cy="26"
                      r="20"
                      stroke="currentColor"
                      strokeWidth="1.25"
                      className={cn(
                        "transition-opacity duration-300",
                        consolidationCadence === "daily" ? "opacity-90" : "opacity-35"
                      )}
                    />
                    {Array.from({ length: 12 }, (_, i) => {
                      const angle = (i * 30 * Math.PI) / 180;
                      const inner = i % 3 === 0 ? 15.5 : 17;
                      const outer = 19;
                      return (
                        <line
                          key={i}
                          x1={26 + inner * Math.sin(angle)}
                          y1={26 - inner * Math.cos(angle)}
                          x2={26 + outer * Math.sin(angle)}
                          y2={26 - outer * Math.cos(angle)}
                          stroke="currentColor"
                          strokeWidth={i % 3 === 0 ? "1.3" : "0.8"}
                          strokeLinecap="round"
                          className={cn(
                            "transition-opacity duration-300",
                            consolidationCadence === "daily" ? "opacity-70" : "opacity-25"
                          )}
                        />
                      );
                    })}
                    <line
                      x1="26"
                      y1="26"
                      x2="18.5"
                      y2="22"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                      className={cn(
                        "transition-opacity duration-300",
                        consolidationCadence === "daily" ? "opacity-90" : "opacity-35"
                      )}
                    />
                    <line
                      x1="26"
                      y1="26"
                      x2="26"
                      y2="10"
                      stroke="currentColor"
                      strokeWidth="1.1"
                      strokeLinecap="round"
                      className={cn(
                        "transition-opacity duration-300",
                        consolidationCadence === "daily" ? "opacity-90" : "opacity-35"
                      )}
                    />
                    <circle
                      cx="26"
                      cy="26"
                      r="1.5"
                      fill="currentColor"
                      className={cn(
                        "transition-opacity duration-300",
                        consolidationCadence === "daily" ? "opacity-90" : "opacity-40"
                      )}
                    />
                  </svg>
                </div>

                <span
                  className={cn(
                    "mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
                    consolidationCadence === "daily"
                      ? "text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/50"
                  )}
                >
                  {consolidationCadence === "daily" ? (
                    <span
                      onClick={(e) => {
                        e.stopPropagation();
                        timeInputRef.current?.focus();
                      }}
                      className="inline-flex items-center gap-1 cursor-text"
                    >
                      <input
                        ref={timeInputRef}
                        type="text"
                        inputMode="numeric"
                        value={timeDraft}
                        onChange={handleTimeChange}
                        onBlur={handleTimeBlur}
                        placeholder="02:00"
                        maxLength={5}
                        aria-label={copy.schedule.timeFormatHint}
                        className="w-[2.8rem] bg-transparent font-mono text-[9px] font-bold outline-none text-[rgb(var(--accent))] text-center tracking-wider caret-[rgb(var(--accent))] cursor-text"
                      />
                      <span className="text-[8px] opacity-75">
                        {formatTimeDisplay(consolidationTime).split(" ")[1]}
                      </span>
                    </span>
                  ) : (
                    copy.schedule.manualLabel
                  )}
                </span>
              </button>
            </div>
          </div>
        )}

        {/* TAB: SUGGESTIONS (suggestion policy) */}
        {activeSubTab === "suggestions" && (
          <div className={panelShell}>
            <div className={panelHeading}>
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.suggestions.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.suggestions.description}
              </p>
            </div>

            <div className={controlColumn}>
              <button
                type="button"
                onClick={() =>
                  updateDraft(
                    "memory",
                    "suggestion_policy",
                    suggestionPolicy === "auto_apply" ? "manual_review" : "auto_apply"
                  )
                }
                className={cn(
                  "group flex flex-col items-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
                  suggestionPolicy === "auto_apply"
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
                )}
                title={
                  suggestionPolicy === "auto_apply"
                    ? "Click to switch to Manual Review"
                    : "Click to switch to Auto-Apply"
                }
              >
                <div className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105">
                  <svg
                    viewBox="0 0 48 48"
                    className="w-full h-full overflow-visible select-none"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <circle
                      cx="24"
                      cy="24"
                      r="20"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeDasharray={suggestionPolicy === "auto_apply" ? "none" : "3 3"}
                      className={cn(
                        "transition-opacity duration-300",
                        suggestionPolicy === "auto_apply" ? "opacity-90" : "opacity-40"
                      )}
                    />
                    <path
                      d={
                        suggestionPolicy === "auto_apply"
                          ? "M 10 24 L 38 24"
                          : "M 10 24 L 20 24 M 28 24 L 38 24"
                      }
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.8"
                      strokeLinecap="round"
                      className="transition-all duration-300"
                    />
                    {suggestionPolicy === "auto_apply" ? (
                      <>
                        <path
                          d="M 32 19 L 37 24 L 32 29"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="1.8"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                        <circle
                          cx="24"
                          cy="24"
                          r="3.5"
                          fill="currentColor"
                          className="animate-pulse"
                        />
                      </>
                    ) : (
                      <>
                        <rect
                          x="20.5"
                          y="18.5"
                          width="7"
                          height="11"
                          rx="2"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="1.5"
                        />
                        <line
                          x1="24"
                          y1="22"
                          x2="24"
                          y2="26"
                          stroke="currentColor"
                          strokeWidth="1.5"
                          strokeLinecap="round"
                        />
                      </>
                    )}
                  </svg>
                </div>

                <span
                  className={cn(
                    "mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.1em] leading-none transition-all duration-300",
                    suggestionPolicy === "auto_apply"
                      ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.1)]"
                      : "text-[rgb(var(--foreground-muted))]/60 bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  {suggestionPolicy === "auto_apply"
                    ? copy.suggestions.autoLabel
                    : copy.suggestions.manualLabel}
                </span>
              </button>
            </div>
          </div>
        )}

        {/* TAB: CONTEXT (max context share budget) */}
        {activeSubTab === "context" && (
          <div className={panelShell}>
            <div className={panelHeading}>
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.context.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.context.description}
              </p>
            </div>

            <div className={controlGrid}>
              {[10, 15, 25].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => updateDraft("memory", "max_context_share", pct / 100)}
                  className={presetButtonClass(budgetPct === pct && !isBudgetCustom)}
                >
                  {pct}%
                </button>
              ))}
              {customInput(isBudgetCustom, isBudgetCustom ? budgetPct : "", handleCustomBudgetChange)}
            </div>
          </div>
        )}

        {/* TAB: LIMIT (fact budget) */}
        {activeSubTab === "limit" && (
          <div className={panelShell}>
            <div className={panelHeading}>
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.limit.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.limit.description}
              </p>
            </div>

            <div className={controlGrid}>
              {[3, 5, 8].map((k) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => updateDraft("memory", "top_k_facts", k)}
                  className={presetButtonClass(topKFacts === k && !isDepthCustom)}
                >
                  {k}
                </button>
              ))}
              {customInput(isDepthCustom, isDepthCustom ? topKFacts : "", handleCustomDepthChange)}
            </div>
          </div>
        )}

        {/* TAB: RELEVANCE (semantic similarity cutoff) */}
        {activeSubTab === "relevance" && (
          <div className={panelShell}>
            <div className={panelHeading}>
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.relevance.title}
              </span>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.relevance.description}
              </p>
            </div>

            <div className={controlGrid}>
              {[25, 40, 70].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => updateDraft("memory", "semantic_similarity_cutoff", pct / 100)}
                  className={presetButtonClass(cutoffPct === pct && !isCutoffCustom)}
                >
                  {pct}%
                </button>
              ))}
              {customInput(isCutoffCustom, isCutoffCustom ? cutoffPct : "", handleCustomCutoffChange)}
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

MemoryConfigDesk.displayName = "MemoryConfigDesk";
