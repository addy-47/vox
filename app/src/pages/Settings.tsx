import React, { useState, useEffect } from "react";
import { Activity, Save, Sun, Moon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useTheme } from "@/shared/context/ThemeContext";
import { invoke } from "@tauri-apps/api/core";
import { CoreSettings } from "@/shared/components/CoreSettings";
import { TraySettings } from "@/shared/components/TraySettings";

export const Settings: React.FC = () => {
  const { theme, toggleTheme } = useTheme();
  const [activeTab, setActiveTab] = useState<"core" | "tray">("core");
  const [selectedModel] = useState("VOX-ENGINE-8B (LATEST)");
  const [selectedVoice, setSelectedVoice] = useState("ETHER");
  const [interactionMode, setInteractionMode] = useState<"PASSIVE" | "PTT">("PASSIVE");

  // Tray Settings State
  const [trayEnabled, setTrayEnabled] = useState(() => localStorage.getItem('isTrayEnabled') !== 'false');
  const [trayTextColor, setTrayTextColor] = useState(() => localStorage.getItem('trayTextColor') || 'accent');
  const [trayBlurDensity, setTrayBlurDensity] = useState(() => parseInt(localStorage.getItem('trayBlurDensity') || '40'));
  const [trayGlassTint, setTrayGlassTint] = useState(() => localStorage.getItem('trayGlassTint') !== 'false');
  const [trayHideDuration, setTrayHideDuration] = useState(() => parseFloat(localStorage.getItem('trayHideDuration') || '5.0'));
  const [trayFadeTransition, setTrayFadeTransition] = useState(() => localStorage.getItem('trayFadeTransition') || 'Smooth');

  useEffect(() => {
    const initSettings = async () => {
      try {
        const settings = await invoke<any>("get_settings");
        if (settings.interaction_mode) {
          setInteractionMode(settings.interaction_mode.toUpperCase() as "PASSIVE" | "PTT");
        }
      } catch (e) {
        console.error("Failed to load settings", e);
      }
    };
    initSettings();
  }, []);

  const handleInteractionModeChange = async (mode: "PASSIVE" | "PTT") => {
    setInteractionMode(mode);
    try {
      await invoke("update_interaction_mode", { mode });
    } catch (e) {
      console.error("Failed to update interaction mode", e);
    }
  };

  // Sync tray settings to localStorage
  useEffect(() => { localStorage.setItem('isTrayEnabled', String(trayEnabled)); }, [trayEnabled]);
  useEffect(() => { localStorage.setItem('trayTextColor', trayTextColor); }, [trayTextColor]);
  useEffect(() => { localStorage.setItem('trayBlurDensity', String(trayBlurDensity)); }, [trayBlurDensity]);
  useEffect(() => { localStorage.setItem('trayGlassTint', String(trayGlassTint)); }, [trayGlassTint]);
  useEffect(() => { localStorage.setItem('trayHideDuration', String(trayHideDuration)); }, [trayHideDuration]);
  useEffect(() => { localStorage.setItem('trayFadeTransition', trayFadeTransition); }, [trayFadeTransition]);

  const voices = ["ETHER", "SOLAS", "KRYPTOS", "LYRA"];

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative">
      {/* Page header */}
      <header className="p-6 md:p-12 border-b border-[rgba(var(--border),0.05)] glass-panel shrink-0">
        <div className="max-w-7xl mx-auto w-full flex flex-col md:flex-row md:items-center justify-between gap-8">
          <div className="flex items-start justify-between w-full md:w-auto">
            <div className="space-y-4">
              <div className="flex items-center gap-2 mb-1">
                <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                <span className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Configuration</span>
              </div>
              <h1 className="text-3xl md:text-4xl font-bold text-[rgb(var(--foreground))] tracking-tight">System <span className="text-[rgb(var(--foreground-muted))] opacity-40">Core</span></h1>
            </div>

            <button
              onClick={toggleTheme}
              className="md:hidden p-2.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-white/[0.03] transition-colors duration-300"
              title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
            >
              {theme === 'dark'
                ? <Sun size={18} strokeWidth={1.5} />
                : <Moon size={18} strokeWidth={1.5} />}
            </button>
          </div>

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2 p-1.5 bg-white/[0.03] border border-[rgba(var(--border),0.05)] rounded-2xl">
              <button 
                onClick={() => setActiveTab("core")}
                className={cn(
                  "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-colors duration-300",
                  activeTab === "core" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Core Settings
              </button>
              <button 
                onClick={() => setActiveTab("tray")}
                className={cn(
                  "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-colors duration-300",
                  activeTab === "tray" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Tray HUD
              </button>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto custom-scrollbar p-6 md:p-12 pb-12 md:pb-12">
        <div className="max-w-7xl mx-auto">
          {activeTab === "core" ? (
            <CoreSettings 
              selectedModel={selectedModel}
              selectedVoice={selectedVoice}
              setSelectedVoice={setSelectedVoice}
              interactionMode={interactionMode}
              handleInteractionModeChange={handleInteractionModeChange}
              voices={voices}
            />
          ) : (
            <TraySettings 
              trayEnabled={trayEnabled}
              setTrayEnabled={setTrayEnabled}
              trayTextColor={trayTextColor}
              setTrayTextColor={setTrayTextColor}
              trayBlurDensity={trayBlurDensity}
              setTrayBlurDensity={setTrayBlurDensity}
              trayGlassTint={trayGlassTint}
              setTrayGlassTint={setTrayGlassTint}
              trayHideDuration={trayHideDuration}
              setTrayHideDuration={setTrayHideDuration}
              trayFadeTransition={trayFadeTransition}
              setTrayFadeTransition={setTrayFadeTransition}
              interactionMode={interactionMode}
              handleInteractionModeChange={handleInteractionModeChange}
            />
          )}
        </div>

        {/* Footer Actions */}
        <div className="max-w-7xl mx-auto mt-16 pt-10 border-t border-[rgba(var(--border),0.05)] space-y-8">
          <div className="flex flex-col sm:flex-row items-center justify-between gap-6">
            <button className="flex items-center gap-2 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:text-[rgb(var(--accent))] transition-colors duration-300 opacity-40 hover:opacity-100">
               <Activity size={14} />
               Restore Factory Synthesis
            </button>
            <div className="flex items-center gap-4 w-full sm:w-auto">
               <button className="flex-1 sm:flex-none px-8 py-3.5 rounded-xl text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:bg-white/[0.03] transition-colors duration-300">
                 Discard
               </button>
               <button className="flex-1 sm:flex-none px-10 py-3.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-bold text-[11px] tracking-[0.2em] uppercase flex items-center justify-center gap-3 hover:scale-105 active:scale-95 transition-colors duration-300 shadow-lg">
                 <Save size={14} /> Commit Changes
               </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
