import { useState, memo, useCallback, useEffect, useRef } from "react";
import { Clock } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
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

  // Format HH:MM to H:MM AM/PM for display
  const formatTimeDisplay = useCallback((t: string) => {
    const [hStr, mStr] = t.split(":");
    const h = parseInt(hStr, 10);
    const m = mStr ?? "00";
    if (isNaN(h)) return t;
    const ampm = h >= 12 ? "PM" : "AM";
    const h12 = h % 12 === 0 ? 12 : h % 12;
    return `${h12}:${m} ${ampm}`;
  }, []);

  const timeInputRef = useRef<HTMLInputElement>(null);


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

            {/* Right: Clock SVG schedule control */}
            <div className="shrink-0 flex flex-col items-center justify-center">
              <button
                type="button"
                role="switch"
                aria-checked={consolidationCadence === "daily"}
                aria-label={consolidationCadence === "daily" ? copy.consolidation.dailyLabel : copy.consolidation.manualLabel}
                onClick={() =>
                  updateDraft(
                    "personal_memory",
                    "consolidation_cadence",
                    consolidationCadence === "daily" ? "manual" : "daily"
                  )
                }
                className={cn(
                  "group flex flex-col items-center select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
                  consolidationCadence === "daily"
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
                )}
              >
                {/* Line-style clock SVG */}
                <div
                  className="relative w-[48px] h-[48px] transition-transform duration-300 group-hover:scale-105"
                  style={{
                    animation: consolidationCadence === "daily" ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
                  }}
                >
                  <svg
                    viewBox="0 0 52 52"
                    className="w-full h-full overflow-visible select-none"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    {/* Outer ring */}
                    <circle
                      cx="26" cy="26" r="20"
                      stroke="currentColor"
                      strokeWidth="1.25"
                      className={cn("transition-opacity duration-300", consolidationCadence === "daily" ? "opacity-90" : "opacity-35")}
                    />
                    {/* 12 tick marks */}
                    {Array.from({ length: 12 }).map((_, i) => {
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
                          className={cn("transition-opacity duration-300", consolidationCadence === "daily" ? "opacity-70" : "opacity-25")}
                        />
                      );
                    })}
                    {/* Hour hand — pointing to ~9 o'clock position */}
                    <line
                      x1="26" y1="26"
                      x2="18.5" y2="22"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                      className={cn("transition-opacity duration-300", consolidationCadence === "daily" ? "opacity-90" : "opacity-35")}
                    />
                    {/* Minute hand — pointing to ~12 o'clock position */}
                    <line
                      x1="26" y1="26"
                      x2="26" y2="10"
                      stroke="currentColor"
                      strokeWidth="1.1"
                      strokeLinecap="round"
                      className={cn("transition-opacity duration-300", consolidationCadence === "daily" ? "opacity-90" : "opacity-35")}
                    />
                    {/* Center dot */}
                    <circle
                      cx="26" cy="26" r="1.5"
                      fill="currentColor"
                      className={cn("transition-opacity duration-300", consolidationCadence === "daily" ? "opacity-90" : "opacity-40")}
                    />
                  </svg>
                </div>

                {/* Badge: MANUAL label or nothing (time input is outside the button) */}
                {consolidationCadence !== "daily" && (
                  <span className="mt-3 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300 text-[rgb(var(--foreground-muted))]/50">
                    {copy.consolidation.manualLabel}
                  </span>
                )}
              </button>

              {/* Time input badge — only in Daily mode, outside the toggle button */}
              {consolidationCadence === "daily" && (
                <button
                  type="button"
                  onClick={() => timeInputRef.current?.focus()}
                  className="mt-0 flex items-center gap-1 px-2 py-0.5 rounded-full cursor-text transition-all duration-200 hover:bg-[rgba(var(--accent),0.08)] group/time"
                  tabIndex={-1}
                  aria-label={copy.consolidation.timeFormatHint}
                >
                  <input
                    ref={timeInputRef}
                    type="text"
                    inputMode="numeric"
                    value={timeDraft}
                    onChange={handleTimeChange}
                    onBlur={handleTimeBlur}
                    placeholder="09:00"
                    maxLength={5}
                    aria-label={copy.consolidation.timeFormatHint}
                    className="w-[3.2rem] bg-transparent font-mono text-[11px] font-bold outline-none text-[rgb(var(--accent))] text-center tracking-wider caret-[rgb(var(--accent))] selection:bg-[rgba(var(--accent),0.25)] cursor-text"
                  />
                  <span className="text-[9px] font-mono font-bold uppercase tracking-[0.12em] text-[rgb(var(--accent))]/70">
                    {formatTimeDisplay(consolidationTime).split(" ")[1]}
                  </span>
                </button>
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
