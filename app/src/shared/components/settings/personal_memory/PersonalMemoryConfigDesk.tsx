import { useState, memo, useCallback, useMemo, useEffect } from "react";
import {  Clock } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl } from "@/shared/ui";
import { PERSONAL_MEMORY_CONFIG_DESK_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";

export interface PersonalMemoryConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

type PersonalMemorySubTab = "consolidation" | "depth" | "cutoff";

const TABS: Array<{ id: PersonalMemorySubTab; label: string }> = [
  { id: "consolidation", label: PERSONAL_MEMORY_CONFIG_DESK_COPY.tabs.consolidation },
  { id: "depth", label: PERSONAL_MEMORY_CONFIG_DESK_COPY.tabs.depth },
  { id: "cutoff", label: PERSONAL_MEMORY_CONFIG_DESK_COPY.tabs.cutoff },
];

function computeNextRunText(
  cadence: string,
  timeStr: string,
  copy: typeof PERSONAL_MEMORY_CONFIG_DESK_COPY.consolidation
): string | null {
  if (cadence !== "daily") return null;
  const parts = timeStr.split(":");
  if (parts.length !== 2) return null;
  const hour = parseInt(parts[0], 10);
  const minute = parseInt(parts[1], 10);
  if (isNaN(hour) || isNaN(minute)) return null;

  const now = new Date();
  const targetToday = new Date(now.getFullYear(), now.getMonth(), now.getDate(), hour, minute, 0);

  if (targetToday.getTime() > now.getTime()) {
    const diffMs = targetToday.getTime() - now.getTime();
    const totalMinutes = Math.floor(diffMs / 60000);
    const h = Math.floor(totalMinutes / 60);
    const m = totalMinutes % 60;
    const durationStr = h > 0 ? `${h}${copy.hoursShort} ${m}${copy.minutesShort}` : `${m}${copy.minutesShort}`;
    return `${copy.nextRunLabel}: ${copy.todayAt} ${timeStr} (${copy.in} ${durationStr})`;
  } else {
    return `${copy.nextRunLabel}: ${copy.tomorrowAt} ${timeStr}`;
  }
}

export const PersonalMemoryConfigDesk = memo(({ layoutMode }: PersonalMemoryConfigDeskProps) => {
  const [activeSubTab, setActiveSubTab] = useState<PersonalMemorySubTab>("consolidation");
  const personalMemory = useSettingsStore((s) => s.draftSettings?.personal_memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const topKFacts = personalMemory?.top_k_facts ?? 5;
  const semanticSimilarityCutoff = personalMemory?.semantic_similarity_cutoff ?? 0.40;
  const consolidationCadence = personalMemory?.consolidation_cadence ?? "daily";
  const consolidationTime = personalMemory?.consolidation_time ?? "02:00";

  const [timeDraft, setTimeDraft] = useState(consolidationTime);

  useEffect(() => {
    setTimeDraft(consolidationTime);
  }, [consolidationTime]);

  const handleTimeChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setTimeDraft(val);
    if (/^([01]\d|2[0-3]):[0-5]\d$/.test(val)) {
      updateDraft("personal_memory", "consolidation_time", val);
    }
  }, [updateDraft]);

  const handleTimeBlur = useCallback(() => {
    const trimmed = timeDraft.trim();
    if (/^([01]?\d|2[0-3]):([0-5]?\d)$/.test(trimmed)) {
      const [h, m] = trimmed.split(":");
      const clean = `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;
      setTimeDraft(clean);
      updateDraft("personal_memory", "consolidation_time", clean);
    } else if (/^([01]?\d|2[0-3])$/.test(trimmed)) {
      const clean = `${trimmed.padStart(2, "0")}:00`;
      setTimeDraft(clean);
      updateDraft("personal_memory", "consolidation_time", clean);
    } else if (/^([01]\d|2[0-3])([0-5]\d)$/.test(trimmed)) {
      const clean = `${trimmed.slice(0, 2)}:${trimmed.slice(2, 4)}`;
      setTimeDraft(clean);
      updateDraft("personal_memory", "consolidation_time", clean);
    } else {
      setTimeDraft(consolidationTime);
    }
  }, [timeDraft, consolidationTime, updateDraft]);

  const handleCustomDepthChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const clean = e.target.value.replace(/[^0-9]/g, "");
      if (!clean) return;
      const val = parseInt(clean, 10);
      if (!isNaN(val) && val >= 1 && val <= 50) {
        updateDraft("personal_memory", "top_k_facts", val);
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
        updateDraft("personal_memory", "semantic_similarity_cutoff", val / 100);
      }
    },
    [updateDraft]
  );

  const copy = PERSONAL_MEMORY_CONFIG_DESK_COPY;
  const isSmall = layoutMode === "small";

  const isDepthCustom = ![3, 5, 8].includes(topKFacts);
  const cutoffPct = Math.round(semanticSimilarityCutoff * 100);
  const isCutoffCustom = ![25, 40, 70].includes(cutoffPct);

  const nextRunText = computeNextRunText(consolidationCadence, consolidationTime, copy.consolidation);

  const cadenceOptions = useMemo(
    () => [
      { id: "manual", label: copy.consolidation.manualLabel },
      { id: "daily", label: copy.consolidation.dailyLabel },
    ],
    [copy.consolidation.manualLabel, copy.consolidation.dailyLabel]
  );

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* Layer 1: Subtab Navigation */}
      <div className="w-full flex items-center justify-between pt-0.5 pb-1.5 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5 overflow-x-auto no-scrollbar">
        {TABS.map((tab, idx, arr) => {
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
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          isSmall ? "h-auto py-1" : "h-[128px] max-h-[128px]"
        )}
      >
        {/* TAB 1: CONSOLIDATION */}
        {activeSubTab === "consolidation" && (
          <div className="flex flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            {/* Left: Title & Description */}
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                  {copy.consolidation.title} <span className="text-[9px] font-bold px-1.5 text-[rgb(var(--accent))]">
                  {copy.consolidation.sublabel}
                </span>
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.consolidation.description}
              </p>
              {consolidationCadence === "daily" && nextRunText ? (
                <div className="flex items-center gap-1.5 text-[10px] sm:text-[10.5px] font-mono text-[rgb(var(--accent))] font-medium pt-0.5">
                  <Clock size={11} className="shrink-0 opacity-85" />
                  <span>{nextRunText}</span>
                </div>
              ) : (
                <p className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 pt-0.5">
                  {copy.consolidation.noSchedule}
                </p>
              )}
            </div>

            {/* Right: Segmented Control + Time Input */}
            <div className="shrink-0 flex flex-col items-center justify-center gap-2">
              <SegmentedControl
                options={cadenceOptions}
                value={consolidationCadence}
                onChange={(val) =>
                  updateDraft("personal_memory", "consolidation_cadence", val)
                }
                size="sm"
              />

              {consolidationCadence === "daily" ? (
                <div className="flex flex-col items-center text-[rgb(var(--foreground))]">
                  <div className="flex items-center gap-1.5">
                    <Clock
                      size={12}
                      className="text-[rgb(var(--accent))] shrink-0"
                    />

                    <input
                      type="text"
                      inputMode="numeric"
                      value={timeDraft}
                      onChange={handleTimeChange}
                      onBlur={handleTimeBlur}
                      placeholder="14:00"
                      maxLength={5}
                      aria-label={copy.consolidation.timeFormatHint}
                      className="w-12 bg-transparent font-mono text-[12.5px] font-bold outline-none text-[rgb(var(--foreground))] text-center tracking-wider caret-[rgb(var(--accent))] selection:bg-[rgba(var(--accent),0.25)]"
                    />
                  </div>
                </div>
              ) : (
                <div className="text-[9.5px] font-mono font-medium text-[rgb(var(--foreground-muted))]/40 py-0.5 tracking-tight">
                  {copy.consolidation.onDemandStatus}
                </div>
              )}
</div>
          </div>
        )}

        {/* TAB 2: DEPTH (FACT LIMIT) */}
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
                  onClick={() => updateDraft("personal_memory", "top_k_facts", k)}
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
                  placeholder={COMPUTE_PROFILE_COPY.custom}
                  className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
              </div>
            </div>
          </div>
        )}

        {/* TAB 3: CUTOFF (RELEVANCE THRESHOLD) */}
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
                  onClick={() => updateDraft("personal_memory", "semantic_similarity_cutoff", pct / 100)}
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
                  placeholder={COMPUTE_PROFILE_COPY.custom}
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

PersonalMemoryConfigDesk.displayName = "PersonalMemoryConfigDesk";
