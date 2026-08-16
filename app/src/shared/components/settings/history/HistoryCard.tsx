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
  const { persistence, ui } = draftSettings;

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
        <div className="group flex items-center w-full h-[58px] relative shrink-0">
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
              <span className="text-[11px] font-black tracking-widest text-[rgb(var(--foreground-muted))]/60 whitespace-nowrap uppercase">
                Incognito Mode
              </span>
              <ShieldAlert
                size={13}
                className={persistence.private_mode ? "text-rose-400 animate-pulse" : "text-[rgb(var(--foreground-muted))]/40"}
              />
            </div>

            <div className="flex items-end justify-between leading-none mt-1">
              <span
                className={cn(
                  "text-[13px] font-black transition-colors truncate capitalize",
                  persistence.private_mode ? "text-rose-400" : "text-[rgb(var(--foreground))]/90 group-hover:text-[rgb(var(--accent))]"
                )}
              >
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
              value={ui.tray_history_limit ?? 5}
              min={1}
              max={15}
              step={1}
              defaultValue={5}
              formatValue={(v) => `${v}`}
              formatPreset={(v) => `${v}`}
              onChange={(v) => updateDraft("ui", "tray_history_limit", v)}
              presetSteps={[3, 5, 8, 10, 15]}
            />
          </div>
        </div>
      </div>
    </Card>
  );
});

HistoryCard.displayName = "HistoryCard";
