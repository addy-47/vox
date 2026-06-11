import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Eye } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export const TrayCard = memo(() => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { ui, interaction } = draftSettings;

  return (
    <div className="w-full lg:w-[320px] bg-transparent lg:bg-black/15 lg:backdrop-blur-md border-0 lg:border border-[rgba(var(--accent),0.10)] rounded-none lg:rounded-2xl p-0 lg:p-5 shadow-none lg:shadow-xl shadow-black/30 text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85">
      {/* Header */}
      <div className="flex items-center gap-2 mb-4 shrink-0">
        <Eye className="text-[rgb(var(--accent))]" size={16} />
        <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
          HUD & Tray HUD
        </span>
      </div>

      <div className="space-y-4">
        {/* Toggle Tray Enabled */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-bold text-[rgb(var(--foreground))]/80">Enable HUD</span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70">Show overlay window</span>
          </div>
          <button
            onClick={() => updateDraft("ui", "tray_enabled", !ui.tray_enabled)}
            className={cn(
              "w-10 h-5 rounded-full relative transition-all duration-300",
              ui.tray_enabled ? "bg-[rgb(var(--accent))]" : "bg-[rgb(var(--foreground))]/15"
            )}
            role="switch"
            aria-checked={ui.tray_enabled}
            aria-label="Toggle HUD"
          >
            <div
              className={cn(
                "absolute top-0.5 w-4 h-4 rounded-full bg-white transition-all duration-300 shadow-sm",
                ui.tray_enabled ? "left-5" : "left-0.5"
              )}
            />
          </button>
        </div>

        {/* Tray Interaction Mode: Passive/PTT */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-bold text-[rgb(var(--foreground))]/80">Tray Mode</span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70">
              {interaction.tray_mode === "Passive" ? "Continuous listening" : "Push-to-talk"}
            </span>
          </div>
          <div className="flex p-0.5 bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.08)] rounded-xl">
            <button
              onClick={() => updateDraft("interaction", "tray_mode", "Passive")}
              className={cn(
                "px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                interaction.tray_mode === "Passive"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              Passive
            </button>
            <button
              onClick={() => updateDraft("interaction", "tray_mode", "PTT")}
              className={cn(
                "px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                interaction.tray_mode === "PTT"
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                  : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              PTT
            </button>
          </div>
        </div>

        {/* History Limit */}
        <div className="space-y-1">
          <div className="flex justify-between items-center">
            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
              History Limit
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">
              {ui.tray_history_limit} turns
            </span>
          </div>
          <input
            type="range"
            min="1"
            max="15"
            value={ui.tray_history_limit}
            onChange={(e) => updateDraft("ui", "tray_history_limit", Number(e.target.value))}
            className="w-full"
          />
        </div>
      </div>
    </div>
  );
});

TrayCard.displayName = "TrayCard";
