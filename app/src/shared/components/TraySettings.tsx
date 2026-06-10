import React from "react";
import { Monitor, Layers, Sliders, History } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";

export const TraySettings: React.FC = () => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;

  const { ui, interaction } = draftSettings;

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 -mr-2">
        <div className="grid lg:grid-cols-3 gap-8 pb-8">
          {/* HUD Setup */}
          <div className="lg:col-span-2 space-y-8">
            <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)]">
              <div className="flex items-center gap-3 mb-8 shrink-0">
                <Monitor className="text-[rgb(var(--accent))]" size={20} />
                <div className="space-y-1">
                  <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Display HUD</h2>
                  <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Control Vox Live window</p>
                </div>
              </div>

              <div className="flex items-center justify-between p-6 glass-whisper glass-base rounded-2xl">
                <div className="space-y-1">
                  <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">Enable HUD</div>
                  <p className="text-[11px] text-[rgb(var(--foreground-muted))] opacity-60 uppercase tracking-widest">Show or hide the always-on-top window</p>
                </div>
                <button 
                  onClick={() => updateDraft("ui", "tray_enabled", !ui.tray_enabled)}
                  className={cn(
                    "w-12 h-6 rounded-full relative transition-all duration-300",
                    ui.tray_enabled ? "bg-[rgb(var(--accent))]" : "bg-[rgb(var(--foreground))]/10"
                  )}
                >
                  <div className={cn(
                    "absolute top-1 w-4 h-4 rounded-full bg-white transition-all duration-300 shadow-sm",
                    ui.tray_enabled ? "left-7" : "left-1"
                  )} />
                </button>
              </div>
            </div>

            {/* Look & Feel (Appearance Card spans full width of this row) */}
            <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] space-y-8">
              <div className="flex items-center gap-3 mb-8 shrink-0">
                <Layers className="text-[rgb(var(--accent))]" size={20} />
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Appearance</h3>
                  <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Customize look & feel</p>
                </div>
              </div>

              <div className="space-y-6">
                <div className="space-y-3">
                  <div className="flex justify-between">
                    <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Blur Density</span>
                    <span className="text-[11px] font-mono opacity-60">{ui.tray_blur_density}px</span>
                  </div>
                  <div className="relative h-6 flex items-center">
                    <input 
                      type="range" 
                      min="0" max="100" 
                      value={ui.tray_blur_density}
                      onChange={(e) => updateDraft("ui", "tray_blur_density", Number(e.target.value))}
                      className="w-full" 
                    />
                  </div>
                </div>

                <div className="flex items-center justify-between">
                  <div className="space-y-1">
                    <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Glass Tint</span>
                    <p className="text-[11px] opacity-60 uppercase tracking-widest">Adds a subtle color glow</p>
                  </div>
                  <button 
                    onClick={() => updateDraft("ui", "tray_glass_tint", !ui.tray_glass_tint)}
                    className={cn(
                      "w-10 h-5 rounded-full relative transition-all duration-300",
                      ui.tray_glass_tint ? "bg-[rgb(var(--accent))]" : "bg-[rgb(var(--foreground))]/10"
                    )}
                  >
                    <div className={cn(
                      "absolute top-1 w-3 h-3 bg-white rounded-full transition-all duration-300",
                      ui.tray_glass_tint ? "right-1" : "left-1"
                    )} />
                  </button>
                </div>
              </div>
            </div>
          </div>

          {/* Tray Interaction */}
          <div className="space-y-8">
            <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] space-y-8">
              <div className="flex items-center gap-3 mb-8 shrink-0">
                <Sliders className="text-[rgb(var(--accent))]" size={20} />
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">HUD Interaction</h3>
                  <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Activation methods</p>
                </div>
              </div>
              
              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">HUD Method</div>
                  <div className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">
                    {interaction.tray_mode === "Passive" ? "Auto Show" : "Manual Only"}
                  </div>
                </div>
                <div className="flex p-1 glass-whisper glass-base rounded-xl">
                  <button 
                    onClick={() => updateDraft("interaction", "tray_mode", "Passive")}
                    className={cn(
                      "px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                      interaction.tray_mode === "Passive" 
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                        : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Auto
                  </button>
                  <button 
                    onClick={() => updateDraft("interaction", "tray_mode", "PTT")}
                    className={cn(
                      "px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase transition-all duration-300",
                      interaction.tray_mode === "PTT" 
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                        : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Manual
                  </button>
                </div>
              </div>
            </div>

            <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] space-y-8">
              <div className="flex items-center gap-3 mb-8 shrink-0">
                <History className="text-[rgb(var(--accent))]" size={20} />
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Session Memory</h3>
                  <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Memory depth</p>
                </div>
              </div>
              
              <div className="space-y-4">
                <div className="flex justify-between">
                  <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">History Depth</span>
                  <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{ui.tray_history_limit} sessions</span>
                </div>
                <div className="relative h-6 flex items-center">
                  <input 
                    type="range" 
                    min="1" max="15" 
                    value={ui.tray_history_limit}
                    onChange={(e) => updateDraft("ui", "tray_history_limit", Number(e.target.value))}
                    className="w-full" 
                  />
                </div>
                <p className="text-[11px] text-[rgb(var(--foreground-muted))] opacity-60 italic leading-relaxed">
                  Controls how many previous completed transcription sessions are stored in the Tray HUD's ephemeral memory.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
