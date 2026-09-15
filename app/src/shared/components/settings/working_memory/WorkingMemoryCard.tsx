import { memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { History, ShieldOff, FoldVertical, Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile } from "@/shared/ui";
import { WORKING_MEMORY_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";

interface WorkingMemoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const WorkingMemoryCard = memo(({ layoutMode = "full-max" }: WorkingMemoryCardProps) => {
  const workingMemory = useSettingsStore((s) => s.draftSettings?.working_memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isSmall = layoutMode === "small";
  const isMin = layoutMode === "full-min";

  const maxContextShare = workingMemory?.max_context_share ?? 0.15;
  const budgetPct = Math.round(maxContextShare * 100);
  const isBudgetCustom = ![10, 15, 25].includes(budgetPct);

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

  const copy = WORKING_MEMORY_SETTINGS_COPY;

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none transform-gpu",
        isSmall
          ? "bg-transparent p-0 h-auto"
          : cn(
              "glass-card p-5 lg:h-[340px] justify-between transition-all duration-300",
              isMin ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
            )
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <History className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            {copy.cardTitle}
          </span>
        </div>
        <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-[rgba(var(--accent),0.06)] border border-[rgba(var(--accent),0.12)]">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
          <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
            {copy.engineBadge}
          </span>
        </div>
      </div>

      {/* Card Body */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-0.5 gap-2.5">
        {/* Layer 1: Top Row Two Toggle Tiles */}
        <div
          className={cn(
            "grid gap-2 shrink-0",
            isSmall ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          {/* Incognito Mode */}
          <ToggleTile
            title={copy.privateModeTitle}
            active={workingMemory.private_mode}
            activeLabel={copy.privateModeActive}
            inactiveLabel={copy.privateModeInactive}
            activeSublabel={copy.privateModeActiveSub}
            inactiveSublabel={copy.privateModeInactiveSub}
            icon={ShieldOff}
            onToggle={() =>
              updateDraft("working_memory", "private_mode", !workingMemory.private_mode)
            }
            layoutMode={layoutMode}
          />

          {/* Auto Compaction Mode */}
          <ToggleTile
            title={copy.autoCompactionTitle}
            active={workingMemory.auto_compaction ?? false}
            activeLabel={copy.autoCompactionActive}
            inactiveLabel={copy.autoCompactionInactive}
            activeSublabel={copy.autoCompactionActiveSub}
            inactiveSublabel={copy.autoCompactionInactiveSub}
            icon={FoldVertical}
            onToggle={() =>
              updateDraft("working_memory", "auto_compaction", !workingMemory.auto_compaction)
            }
            layoutMode={layoutMode}
          />
        </div>

        {/* Layer 2: Context Share Budget Desk */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-3.5 sm:p-4 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between",
            isSmall ? "h-auto" : "h-full"
          )}
        >
          {/* Top: Header & Description */}
          <div className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <Cpu className="text-[rgb(var(--accent))]" size={15} />
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.budgetTitle}
              </span>
              <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] px-2 py-0.5 bg-[rgba(var(--accent),0.1)] rounded-md border border-[rgba(var(--accent),0.2)]">
                {budgetPct}%
              </span>
            </div>
            <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed font-medium">
              {copy.budgetDesc}
            </p>
          </div>

          {/* Middle: Visual Allocation Distribution Track */}
          <div className="flex flex-col gap-1.5 py-1">
            <div className="h-2.5 w-full rounded-full bg-[rgba(var(--foreground),0.06)] overflow-hidden flex p-0.5 relative">
              <div
                className="h-full rounded-full bg-[rgb(var(--accent))] transition-all duration-300 shadow-[0_0_10px_rgba(var(--accent),0.35)]"
                style={{ width: `${Math.min(100, Math.max(5, budgetPct))}%` }}
              />
            </div>
          </div>

          {/* Bottom: 4-Column Presets Row */}
          <div className="grid grid-cols-4 gap-2 w-full pt-1">
            {[10, 15, 25].map((pct) => (
              <button
                key={pct}
                type="button"
                onClick={() => updateDraft("working_memory", "max_context_share", pct / 100)}
                className={cn(
                  "py-1.5 rounded-lg text-[11px] font-mono font-bold transition-all duration-200 cursor-pointer flex items-center justify-center border",
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
                "rounded-lg border flex items-center justify-center transition-all duration-200 overflow-hidden",
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
                placeholder={COMPUTE_PROFILE_COPY.custom}
                className="w-full text-center text-[11px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1.5 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
              />
            </div>
          </div>
        </div>
      </div>
    </Card>
  );
});

WorkingMemoryCard.displayName = "WorkingMemoryCard";
