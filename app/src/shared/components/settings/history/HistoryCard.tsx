import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { History, ShieldOff, FoldVertical } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, RotaryKnob, ToggleTile } from "@/shared/ui";
import { HISTORY_SETTINGS_COPY } from "@/data/settingsCopy";

interface HistoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const HistoryCard = memo(({ layoutMode = "full-max" }: HistoryCardProps) => {
  const history = useSettingsStore((s) => s.draftSettings?.history);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!history) return null;

  const isSmall = layoutMode === "small";

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between select-none transform-gpu",
        !isSmall && cn(
          "p-5 min-h-[310px] h-full justify-between transition-all duration-300",
          layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
        )
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
        <div className="flex items-center gap-2">
          <History className="text-[rgb(var(--accent))]" size={17} />
          <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
            History & Storage
          </span>
        </div>
      </div>

      {/* Card Content */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-1 gap-2.5">
        {/* Top Row: Two Toggle Tiles Side by Side */}
        <div
          className={cn(
            "grid gap-2 shrink-0",
            isSmall ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          {/* Incognito Mode: Standardized ToggleTile */}
          <ToggleTile
            title={HISTORY_SETTINGS_COPY.privateModeTitle}
            active={history.private_mode}
            activeLabel={HISTORY_SETTINGS_COPY.privateModeActive}
            inactiveLabel={HISTORY_SETTINGS_COPY.privateModeInactive}
            activeSublabel={HISTORY_SETTINGS_COPY.privateModeActiveSub}
            inactiveSublabel={HISTORY_SETTINGS_COPY.privateModeInactiveSub}
            icon={ShieldOff}
            onToggle={() =>
              updateDraft("history", "private_mode", !history.private_mode)
            }
            layoutMode={layoutMode}
          />

          {/* Auto Compaction Mode: Standardized ToggleTile */}
          <ToggleTile
            title={HISTORY_SETTINGS_COPY.autoCompactionTitle}
            active={history.auto_compaction ?? false}
            activeLabel={HISTORY_SETTINGS_COPY.autoCompactionActive}
            inactiveLabel={HISTORY_SETTINGS_COPY.autoCompactionInactive}
            activeSublabel={HISTORY_SETTINGS_COPY.autoCompactionActiveSub}
            inactiveSublabel={HISTORY_SETTINGS_COPY.autoCompactionInactiveSub}
            icon={FoldVertical}
            onToggle={() =>
              updateDraft("history", "auto_compaction", !history.auto_compaction)
            }
            layoutMode={layoutMode}
          />
        </div>

        {/* Lower Section: HUD Limit Rotary Knob & Database Status (Unified Glass Container) */}
        <div className="flex-1 flex items-center gap-4 justify-between p-3 rounded-xl border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]">
          <div className="flex flex-col gap-1 min-w-0 flex-1">
            <span className="text-[12px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]">
              {HISTORY_SETTINGS_COPY.engineTitle}
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-normal">
              {HISTORY_SETTINGS_COPY.engineDesc}
            </span>
          </div>

          {/* Rotary Knob for HUD History Limit */}
          <div className="shrink-0 flex items-center justify-center pl-1">
            <RotaryKnob
              value={history.tray_history_limit ?? 5}
              min={1}
              max={15}
              step={1}
              defaultValue={5}
              formatValue={(v) => `${v}`}
              formatPreset={(v) => `${v}`}
              onChange={(v) => updateDraft("history", "tray_history_limit", v)}
              presetSteps={[3, 5, 10]}
            />
          </div>
        </div>
      </div>
    </Card>
  );
});

HistoryCard.displayName = "HistoryCard";
