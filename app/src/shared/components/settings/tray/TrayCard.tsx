import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Eye, EyeOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Card, ToggleTile, SliderField } from "@/shared/ui";

interface TrayCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const trayStyles = `
@keyframes wave-bar-1 { 0%, 100% { height: 4px; } 50% { height: 16px; } }
@keyframes wave-bar-2 { 0%, 100% { height: 16px; } 50% { height: 6px; } }
@keyframes wave-bar-3 { 0%, 100% { height: 8px; } 50% { height: 18px; } }
@keyframes wave-bar-4 { 0%, 100% { height: 12px; } 50% { height: 4px; } }

.animate-wave-bar-1 { animation: wave-bar-1 1.2s ease-in-out infinite; }
.animate-wave-bar-2 { animation: wave-bar-2 1.2s ease-in-out infinite 0.2s; }
.animate-wave-bar-3 { animation: wave-bar-3 1.2s ease-in-out infinite 0.4s; }
.animate-wave-bar-4 { animation: wave-bar-4 1.2s ease-in-out infinite 0.6s; }
`;

export const TrayCard = memo(({ layoutMode = "full-max" }: TrayCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;
  const { ui, interaction } = draftSettings;

  const isSmall = layoutMode === "small";

  return (
    <Card
      layoutMode={layoutMode}
      elevation="card"
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between",
        !isSmall && cn(
          "p-5 lg:h-[240px]",
          layoutMode === "full-min" ? "lg:w-[300px] xl:w-[340px] 2xl:w-[380px]" : "lg:w-[380px]"
        )
      )}
    >
      <style>{trayStyles}</style>

      {/* Header */}
      {!isSmall && (
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Eye className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              HUD & Tray HUD
            </span>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-3 flex-1 justify-between mt-2">
        {/* Core Controls Dashboard Grid (2 Buttons) */}
        <div className={cn(
          "grid gap-2 shrink-0",
          layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
        )}>
          
          {/* Card 1: Enable HUD */}
          <ToggleTile
            title="HUD Window"
            active={ui.tray_enabled}
            activeLabel="Enabled"
            inactiveLabel="Disabled"
            activeSublabel="Overlay Active"
            inactiveSublabel="Background Run"
            icon={ui.tray_enabled ? Eye : EyeOff}
            onToggle={() => updateDraft("ui", "tray_enabled", !ui.tray_enabled)}
            layoutMode={layoutMode}
            visualizer={
              ui.tray_enabled ? (
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
            active={interaction.tray_mode === "Passive"}
            activeLabel={interaction.tray_mode === "Passive" ? "Continuous" : "Push-To-Talk"}
            inactiveLabel="Push-To-Talk"
            activeSublabel={interaction.tray_mode === "Passive" ? "Passive Sense" : "Manual Trigger"}
            inactiveSublabel="Manual Trigger"
            icon={interaction.tray_mode === "Passive" ? Activity : Radio}
            onToggle={() => updateDraft("interaction", "tray_mode", interaction.tray_mode === "Passive" ? "PTT" : "Passive")}
            layoutMode={layoutMode}
            visualizer={
              interaction.tray_mode === "Passive" ? (
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

        {/* History Limit Slider */}
        <SliderField
          label="History Limit"
          sublabel={!isSmall ? "Maximum stored dialogue turns in tray" : undefined}
          value={ui.tray_history_limit}
          min={1}
          max={15}
          step={1}
          formatValue={(v) => `${v} turns`}
          onChange={(v) => updateDraft("ui", "tray_history_limit", v)}
          className="mt-2"
        />
      </div>
    </Card>
  );
});

TrayCard.displayName = "TrayCard";
