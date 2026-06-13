import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Eye, EyeOff, RefreshCw, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";

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
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { ui, interaction } = draftSettings;

  const isSmall = layoutMode === "small";

  return (
    <div 
      className={cn(
        "text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 flex flex-col justify-between",
        isSmall
          ? "w-full bg-transparent p-0 h-auto"
          : cn(
              "glass-card glass-base p-5 lg:h-[240px]",
              layoutMode === "full-min" ? "lg:w-[300px] xl:w-[340px] 2xl:w-[380px]" : "lg:w-[380px]"
            )
      )}
    >
      <style>{trayStyles}</style>

      {/* Header */}
      {!isSmall && (
        <div className="flex items-center justify-between mb-2 shrink-0">
          <div className="flex items-center gap-2">
            <Eye className="text-[rgb(var(--accent))]" size={16} />
            <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
              HUD & Tray HUD
            </span>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-3 flex-1 justify-between mt-2">
        {/* Core Controls Dashboard Grid (2 Buttons) */}
        <div className="grid grid-cols-2 gap-2 shrink-0">
          
          {/* Card 1: Enable HUD */}
          <div 
            onClick={() => updateDraft("ui", "tray_enabled", !ui.tray_enabled)}
            className="p-3 rounded-xl border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between min-h-[85px] h-[85px] cursor-pointer group relative overflow-hidden"
          >
            <div className="flex items-center justify-between">
              <span className="text-[11px] font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70 whitespace-nowrap">HUD Window</span>
              <div className="flex items-center gap-3">
                {isSmall ? (
                  <RefreshCw size={10} className="text-[rgb(var(--accent))]/70 group-hover:rotate-180 transition-transform duration-500 shrink-0" />
                ) : (
                  <span className="text-[9px] tracking-wider text-[rgb(var(--accent))]/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300">Click to Toggle</span>
                )}
                {ui.tray_enabled ? <Eye size={12} className="text-[rgb(var(--accent))]" /> : <EyeOff size={12} className="text-[rgb(var(--accent))]" />}
              </div>
            </div>
            
            <div className="flex items-end justify-between mt-2">
              <div className="flex flex-col">
                <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
                  {ui.tray_enabled ? "Enabled" : "Disabled"}
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
                  {ui.tray_enabled ? "Overlay Active" : "Background Run"}
                </span>
              </div>
              
              {/* Visualizer widget */}
              <div className="h-4 flex items-end">
                {ui.tray_enabled ? (
                  <div className="w-3 h-3 rounded border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                    <span className="absolute inset-0 rounded border border-[rgb(var(--accent))] animate-ping opacity-60" />
                    <span className="w-1.5 h-1.5 rounded bg-[rgb(var(--accent))]" />
                  </div>
                ) : (
                  <div className="w-3 h-3 rounded border border-[rgb(var(--foreground))]/15 flex items-center justify-center">
                    <span className="w-1.5 h-1.5 rounded bg-[rgb(var(--foreground-muted))]/40" />
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Card 2: Tray Mode (Continuous vs PTT) */}
          <div 
            onClick={() => updateDraft("interaction", "tray_mode", interaction.tray_mode === "Passive" ? "PTT" : "Passive")}
            className="p-3 rounded-xl border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between min-h-[85px] h-[85px] cursor-pointer group relative overflow-hidden"
          >
            <div className="flex items-center justify-between">
              <span className="text-[11px] font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70 whitespace-nowrap">Tray Mode</span>
              <div className="flex items-center gap-3">
                {isSmall ? (
                  <RefreshCw size={10} className="text-[rgb(var(--accent))]/70 group-hover:rotate-180 transition-transform duration-500 shrink-0" />
                ) : (
                  <span className="text-[9px] tracking-wider text-[rgb(var(--accent))]/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300">Click to Switch</span>
                )}
                {interaction.tray_mode === "Passive" ? <Activity size={12} className="text-[rgb(var(--accent))]" /> : <Radio size={12} className="text-[rgb(var(--accent))]" />}
              </div>
            </div>
            
            <div className="flex items-end justify-between mt-2">
              <div className="flex flex-col">
                <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
                  {interaction.tray_mode === "Passive" ? "Continuous" : "Push-To-Talk"}
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
                  {interaction.tray_mode === "Passive" ? "Passive Sense" : "Manual Trigger"}
                </span>
              </div>
              
              {/* Visualizer widget */}
              <div className="h-4 flex items-end">
                {interaction.tray_mode === "Passive" ? (
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
                )}
              </div>
            </div>
          </div>

        </div>

        {/* History Limit Slider */}
        <div className="space-y-2 mt-4">
          <div className="flex justify-between items-center">
            <div className="flex flex-col">
              <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
                History Limit
              </span>
              {!isSmall && (
                <span className="text-[11px] text-[rgb(var(--foreground-muted))]/55">
                  Maximum stored dialogue turns in tray
                </span>
              )}
            </div>
            <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 px-2.5 py-0.5 rounded-lg shrink-0">
              {ui.tray_history_limit} turns
            </span>
          </div>
          <input
            type="range"
            min="1"
            max="15"
            value={ui.tray_history_limit}
            onChange={(e) => updateDraft("ui", "tray_history_limit", Number(e.target.value))}
            className="w-full mt-2"
          />
        </div>
      </div>
    </div>
  );
});

TrayCard.displayName = "TrayCard";
