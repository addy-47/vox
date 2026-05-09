import React from "react";
import { cn } from "@/shared/lib/utils";

interface TraySettingsProps {
  trayEnabled: boolean;
  setTrayEnabled: (enabled: boolean) => void;
  trayTextColor: string;
  setTrayTextColor: (color: string) => void;
  trayBlurDensity: number;
  setTrayBlurDensity: (density: number) => void;
  trayGlassTint: boolean;
  setTrayGlassTint: (tint: boolean) => void;
  trayHideDuration: number;
  setTrayHideDuration: (duration: number) => void;
  trayFadeTransition: string;
  setTrayFadeTransition: (transition: string) => void;
  interactionMode: "PASSIVE" | "PTT";
  handleInteractionModeChange: (mode: "PASSIVE" | "PTT") => void;
}

export const TraySettings: React.FC<TraySettingsProps> = ({
  trayEnabled,
  setTrayEnabled,
  trayTextColor,
  setTrayTextColor,
  trayBlurDensity,
  setTrayBlurDensity,
  trayGlassTint,
  setTrayGlassTint,
  trayHideDuration,
  setTrayHideDuration,
  trayFadeTransition,
  setTrayFadeTransition,
  interactionMode,
  handleInteractionModeChange,
}) => {
  return (
    <div className="max-w-4xl space-y-8 pb-20">
      <div className="premium-card p-8 space-y-10">
        <div className="flex items-center justify-between">
          <div className="space-y-2">
            <h2 className="text-xl font-bold text-[rgb(var(--foreground))]">Tray HUD Configuration</h2>
            <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed opacity-60">
              Adjust visual aesthetics and behavioral parameters of the background HUD.
            </p>
          </div>
          <button 
            onClick={() => setTrayEnabled(!trayEnabled)}
            className={cn(
              "w-14 h-7 rounded-full relative transition-colors duration-300",
              trayEnabled ? "bg-[rgb(var(--accent))]" : "bg-white/[0.05]"
            )}
          >
            <div className={cn(
              "absolute top-1 w-5 h-5 rounded-full bg-white transition-colors duration-300",
              trayEnabled ? "left-8" : "left-1"
            )} />
          </button>
        </div>

        {/* Interaction Mode toggle in Tray tab */}
        <div className="pt-8 border-t border-[rgba(var(--border),0.05)]">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">Interaction Mode</div>
              <div className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-50">{interactionMode === "PASSIVE" ? "Passive VAD" : "Push-To-Talk"}</div>
            </div>
            <div className="flex p-1 bg-white/[0.05] border border-[rgba(var(--border),0.05)] rounded-xl">
              <button 
                onClick={() => handleInteractionModeChange("PASSIVE")}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all duration-300",
                  interactionMode === "PASSIVE" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Passive
              </button>
              <button 
                onClick={() => handleInteractionModeChange("PTT")}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase transition-all duration-300",
                  interactionMode === "PTT" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                PTT
              </button>
            </div>
          </div>
          <p className="text-[10px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-40 mt-3">
            {interactionMode === "PASSIVE" 
              ? "Tray responds automatically when speech is detected. No button needed."
              : "Click the mic button in the tray header to start recording, click again to stop."}
          </p>
        </div>

        <div className="grid md:grid-cols-2 gap-12 pt-10 border-t border-[rgba(var(--border),0.03)]">
          {/* Visual Settings */}
          <div className="space-y-8">
            <h3 className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest">Aesthetics</h3>
            
            <div className="space-y-6">
              <div className="flex items-center justify-between">
                <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Text Color</span>
                <div className="flex gap-2">
                  {['accent', 'white', 'muted'].map(c => (
                    <button 
                      key={c}
                      className={cn(
                        "w-6 h-6 rounded-lg border border-[rgba(var(--border),0.05)] transition-colors duration-300 active:scale-90",
                        trayTextColor === c ? "ring-2 ring-[rgb(var(--accent))] ring-offset-2 ring-offset-[rgb(var(--background))]" : "opacity-50 hover:opacity-100"
                      )}
                      style={{ backgroundColor: c === 'accent' ? 'rgb(var(--accent))' : c === 'white' ? 'white' : 'rgba(255,255,255,0.4)' }}
                      onClick={() => setTrayTextColor(c)}
                    />
                  ))}
                </div>
              </div>

              <div className="space-y-3">
                <div className="flex justify-between">
                  <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Blur Density</span>
                  <span className="text-[10px] font-mono opacity-40">{trayBlurDensity}px</span>
                </div>
                <input 
                  type="range" 
                  min="0" max="100" 
                  value={trayBlurDensity}
                  onChange={(e) => setTrayBlurDensity(Number(e.target.value))}
                  className="w-full h-1.5 bg-white/5 rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]" 
                />
              </div>

              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Glass Tint</span>
                  <p className="text-[10px] opacity-30 uppercase tracking-widest">Enable backdrop colorization</p>
                </div>
                <button 
                  onClick={() => setTrayGlassTint(!trayGlassTint)}
                  className={cn(
                    "w-10 h-5 rounded-full relative transition-colors duration-300",
                    trayGlassTint ? "bg-[rgb(var(--accent))]" : "bg-white/10"
                  )}
                >
                  <div className={cn(
                    "absolute top-1 w-3 h-3 bg-white rounded-full transition-colors duration-300",
                    trayGlassTint ? "right-1" : "left-1"
                  )} />
                </button>
              </div>
            </div>
          </div>

          {/* Behavioral Settings */}
          <div className="space-y-8">
            <h3 className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest">Behavior</h3>

            <div className="space-y-8">
              <div className="space-y-4">
                <div className="flex justify-between">
                  <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Hide Delay</span>
                  <span className="text-[10px] font-mono opacity-40">{trayHideDuration}s</span>
                </div>
                <div className="grid grid-cols-5 gap-2">
                  {[1, 2, 3, 5, 10].map(s => (
                    <button 
                      key={s}
                      className={cn(
                        "py-2 rounded-lg text-[10px] font-bold transition-colors duration-300",
                        trayHideDuration === s 
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                          : "bg-white/5 border border-[rgba(var(--border),0.03)] hover:bg-white/10 text-white/60"
                      )}
                      onClick={() => setTrayHideDuration(s)}
                    >
                      {s}s
                    </button>
                  ))}
                </div>
              </div>

              <div className="space-y-4">
                <div className="flex justify-between">
                  <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Fade Transition</span>
                  <span className="text-[10px] font-mono opacity-40">0.8s</span>
                </div>
                <div className="grid grid-cols-3 gap-3">
                  {['Snappy', 'Smooth', 'Liquid'].map(f => (
                    <button 
                      key={f}
                      onClick={() => setTrayFadeTransition(f)}
                      className={cn(
                        "py-2.5 rounded-lg text-[10px] font-bold uppercase tracking-widest transition-colors duration-300",
                        trayFadeTransition === f 
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                          : "bg-white/5 border border-[rgba(var(--border),0.03)] hover:bg-white/10 text-white/60"
                      )}
                    >
                      {f}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="premium-card p-6 border-[rgb(var(--accent))]/20 bg-[rgb(var(--accent))]/5">
        <p className="text-[10px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase mb-4">Monitor Status</p>
        <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed opacity-70">
          Your Tray HUD utilizes a low-latency VAD (Voice Activity Detection) monitor. Adjust the sensitivity if the HUD appears during background noise.
        </p>
      </div>
    </div>
  );
};
