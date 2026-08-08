import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Eye, EyeOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile, RotaryKnob } from "@/shared/ui";

interface TrayCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const TrayCard = memo(({ layoutMode = "full-max" }: TrayCardProps) => {
  const trayEnabled = useSettingsStore((s) => s.draftSettings?.ui.tray_enabled ?? true);
  const trayMode = useSettingsStore((s) => s.draftSettings?.interaction.tray_mode ?? "Passive");
  const trayHistoryLimit = useSettingsStore((s) => s.draftSettings?.ui.tray_history_limit ?? 5);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isSmall = layoutMode === "small";

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between transform-gpu",
        !isSmall && cn(
          "p-5 lg:h-[265px]",
          layoutMode === "full-min" ? "lg:w-[330px] xl:w-[370px] 2xl:w-[410px]" : "lg:w-[420px]"
        )
      )}
    >
      {/* Header */}
      {!isSmall && (
        <div className="flex items-center justify-between mb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Eye className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              HUD & Tray HUD
            </span>
          </div>
        </div>
      )}

      {/* Main Body: Vertical Toggles on Left, Rotary Knob on Right */}
      <div className={cn(
        "flex flex-1 items-center gap-4 mt-1",
        isSmall ? "flex-col" : "flex-row justify-between"
      )}>
        {/* Left Side: Vertically Stacked Toggle Tiles */}
        <div className="flex-1 flex flex-col gap-2.5 min-w-0 w-full">
          {/* Card 1: Enable HUD */}
          <ToggleTile
            title="HUD Window"
            active={trayEnabled}
            activeLabel="Enabled"
            inactiveLabel="Disabled"
            activeSublabel="Overlay Active"
            inactiveSublabel="Background Run"
            icon={trayEnabled ? Eye : EyeOff}
            onToggle={() => updateDraft("ui", "tray_enabled", !trayEnabled)}
            layoutMode={layoutMode}
            visualizer={
              trayEnabled ? (
                <div className="w-3 h-3 rounded border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                  <span className="absolute inset-0 rounded border border-[rgb(var(--accent))] animate-ping opacity-60" />
                  <span className="w-1.5 h-1.5 rounded bg-[rgb(var(--accent))]" />
                </div>
              ) : (
                <div className="w-3 h-3 rounded border border-[rgb(var(--foreground))]/15 flex items-center justify-center">
                  <span className="w-1.5 h-1.5 rounded bg-[rgb(var(--foreground-muted))]/40" />
                </div>
              )
            }
          />

          {/* Card 2: Tray Mode */}
          <ToggleTile
            title="Tray Mode"
            active={trayMode === "Passive"}
            activeLabel={trayMode === "Passive" ? "Continuous" : "Push-To-Talk"}
            inactiveLabel="Push-To-Talk"
            activeSublabel={trayMode === "Passive" ? "Passive Sense" : "Manual Trigger"}
            inactiveSublabel="Manual Trigger"
            icon={trayMode === "Passive" ? Activity : Radio}
            onToggle={() => updateDraft("interaction", "tray_mode", trayMode === "Passive" ? "PTT" : "Passive")}
            layoutMode={layoutMode}
            visualizer={
              trayMode === "Passive" ? (
                <div className="flex items-end gap-[1.5px] h-3">
                  <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-1" />
                  <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-2" />
                  <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-3" />
                  <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-4" />
                </div>
              ) : (
                <div className="w-3 h-3 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                  <span className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-60" />
                  <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                </div>
              )
            }
          />
        </div>

        {/* Right Side: Rotary Studio Knob for History Limit */}
        <div className="shrink-0 flex items-center justify-center pl-1">
          <RotaryKnob
            label="History Limit"
            value={trayHistoryLimit}
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
    </Card>
  );
});

TrayCard.displayName = "TrayCard";
