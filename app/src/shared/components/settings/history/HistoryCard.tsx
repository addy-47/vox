import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { History, ShieldAlert } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, RotaryKnob } from "@/shared/ui";

interface HistoryCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const HistoryCard = memo(({ layoutMode = "full-max" }: HistoryCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;
  const { history } = draftSettings;

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
      {!isSmall ? (
        <div className="flex items-center justify-between mb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-1.5 w-full">
          <div className="flex items-center gap-2">
            <History className="text-[rgb(var(--accent))]" size={16} />
            <span className="font-display text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              History & Storage
            </span>
          </div>
        </div>
      ) : (
        <div className="flex items-center justify-between mb-4 w-full shrink-0">
          <span className="font-display text-[13px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/80">
            History Settings
          </span>
        </div>
      )}

      {/* Card Content */}
      <div className="flex-1 flex flex-col justify-between min-h-0 pt-1 gap-3">
        {/* Incognito Mode Hover Slide-Out Tile */}
        <div className="group flex items-center w-full h-[58px] relative shrink-0 overflow-hidden rounded-xl">
          <div
            onClick={() => updateDraft("history", "private_mode", !history.private_mode)}
            className="flex-1 h-full flex items-center justify-between px-3.5 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] rounded-xl cursor-pointer hover:border-[rgba(var(--accent),0.3)] transition-all duration-200"
          >
            <div className="flex items-center gap-2.5">
              <ShieldAlert
                size={16}
                className={history.private_mode ? "text-[rgb(var(--pink))]" : "text-[rgb(var(--foreground-muted))]/40"}
              />
              <div className="flex flex-col">
                <span className="text-[12px] font-bold tracking-tight text-[rgb(var(--foreground))]">
                  Incognito Mode
                </span>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/60">
                  {history.private_mode
                    ? "Ephemeral session (No traces persisted)"
                    : "Standard persistent storage"}
                </span>
              </div>
            </div>

            <div className="flex items-center gap-1.5">
              <span
                className={cn(
                  "font-mono text-[10.5px] font-bold uppercase tracking-wider",
                  history.private_mode ? "text-rose-400" : "text-[rgb(var(--foreground))]/90 group-hover:text-[rgb(var(--accent))]"
                )}
              >
                {history.private_mode ? "Incognito Active" : "Logging Active"}
              </span>
              <span className="relative flex h-2 w-2 items-center justify-center">
                {history.private_mode && (
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75" />
                )}
                <span className={cn("w-1 h-1 rounded-full", history.private_mode ? "bg-rose-400" : "bg-[rgb(var(--foreground-muted))]/40")} />
              </span>
            </div>
          </div>

          <div
            onClick={() => updateDraft("history", "private_mode", !history.private_mode)}
            className="w-0 group-hover:w-7 h-full bg-[rgba(var(--accent),0.1)] border-y border-r border-[rgba(var(--accent),0.2)] rounded-r-xl transition-all duration-200 overflow-hidden flex items-center justify-center cursor-pointer"
          >
            <span className="text-[11px] font-black uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
              TOGGLE
            </span>
          </div>
        </div>

        {/* Lower Section: HUD Limit Rotary Knob & Database Status */}
        <div className="flex-1 flex items-center gap-4 justify-between p-3 rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.01)]">
          <div className="flex flex-col gap-1 min-w-0 flex-1">
            <span className="text-[12px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]">
              Session History Engine
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-normal">
              Turso SQLite storage active. Conversations are recorded with zero arbitrary retention limits.
            </span>
          </div>

          {/* Rotary Knob for HUD History Limit */}
          <div className="shrink-0 flex items-center justify-center pl-1">
            <RotaryKnob
              label="HUD Limit"
              value={history.tray_history_limit ?? 5}
              min={1}
              max={15}
              step={1}
              defaultValue={5}
              formatValue={(v) => `${v}`}
              formatPreset={(v) => `${v}`}
              onChange={(v) => updateDraft("history", "tray_history_limit", v)}
              presetSteps={[3, 5, 8, 10, 15]}
            />
          </div>
        </div>
      </div>
    </Card>
  );
});

HistoryCard.displayName = "HistoryCard";
