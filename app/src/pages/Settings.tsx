import React, { useEffect, useState } from "react";
import { Save, RotateCcw, Settings as SettingsIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { CoreSettings } from "@/shared/components/CoreSettings";
import { ModelSettings } from "@/shared/components/ModelSettings";
import { TraySettings } from "@/shared/components/TraySettings";

export const Settings: React.FC = () => {
  const { draftSettings, hasChanges, commitChanges, discardChanges, restoreDefaults } = useSettings();
  const [activeTab, setActiveTab] = useState<"core" | "models" | "tray">("core");

  // Deep link support: ?tab=models or ?tab=tray
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const tab = params.get("tab");
    if (tab === "models" || tab === "tray") {
      setActiveTab(tab);
    }
  }, []);

  if (!draftSettings) return null;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-[rgb(var(--background))]">
      <header className="px-6 md:px-10 py-6 md:py-10 shrink-0">
        <div className="max-w-[1600px] mx-auto flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-xl bg-[rgb(var(--accent))]/10">
                <SettingsIcon className="text-[rgb(var(--accent))]" size={24} />
              </div>
              <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-[rgb(var(--foreground))]">
                System <span className="text-[rgb(var(--foreground-muted))] opacity-60 font-medium">Core</span>
              </h1>
            </div>
            <p className="text-sm text-[rgb(var(--foreground-muted))] max-w-md">Configure intelligence, interface, and assistant behavior.</p>
          </div>

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5 p-1 bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] rounded-2xl">
              <button 
                onClick={() => {
                  setActiveTab("core");
                  window.history.replaceState(null, '', '/settings');
                }}
                className={cn(
                  "px-5 py-2 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all duration-300",
                  activeTab === "core" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Core
              </button>
              <button 
                onClick={() => {
                  setActiveTab("models");
                  window.history.replaceState(null, '', '/settings?tab=models');
                }}
                className={cn(
                  "px-5 py-2 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all duration-300",
                  activeTab === "models" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Models
              </button>
              <button 
                onClick={() => {
                  setActiveTab("tray");
                  window.history.replaceState(null, '', '/settings?tab=tray');
                }}
                className={cn(
                  "px-5 py-2 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all duration-300",
                  activeTab === "tray" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Tray
              </button>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content Area - Fixed viewport, internal scrolling only */}
      <div className="flex-1 overflow-hidden relative px-6 md:px-10">
        <div className="h-full max-w-[1600px] mx-auto py-6 md:py-8">
          {activeTab === "core" ? (
            <CoreSettings />
          ) : activeTab === "models" ? (
            <ModelSettings />
          ) : (
            <TraySettings />
          )}
        </div>
      </div>

      {/* Footer Actions - Fixed at bottom */}
      <footer className="shrink-0 border-t border-[rgba(var(--border),0.05)] glass-panel bg-[rgb(var(--background))]/80 backdrop-blur-xl px-6 md:px-10">
        <div className="max-w-[1600px] mx-auto py-6 md:py-8 flex items-center justify-between gap-4">
          <button 
            onClick={restoreDefaults}
            className="hidden md:flex items-center gap-2 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:text-red-400 transition-colors duration-300 opacity-60 hover:opacity-600"
            title="Restore Defaults"
          >
             <RotateCcw size={16} />
             <span>Restore Defaults</span>
          </button>
          <div className="flex items-center justify-between gap-3 w-full md:w-auto">
             <button 
               onClick={discardChanges}
               disabled={!hasChanges}
               className={cn(
                 "flex-1 md:flex-none px-4 md:px-8 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-[0.2em] transition-all duration-300",
                 hasChanges 
                  ? "text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.05]" 
                  : "text-[rgb(var(--foreground-muted))] opacity-60 cursor-not-allowed"
               )}
             >
               <span className="hidden sm:inline">Discard Changes</span>
               <span className="sm:hidden">Discard</span>
             </button>
             <button 
               onClick={commitChanges}
               disabled={!hasChanges}
               className={cn(
                 "flex-1 md:flex-none px-6 md:px-10 py-2.5 rounded-xl font-bold text-[11px] tracking-[0.2em] uppercase flex items-center justify-center gap-2 transition-all duration-300 shadow-lg",
                 hasChanges
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-105 active:scale-95 shadow-[rgb(var(--accent))]/20"
                  : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] opacity-60 cursor-not-allowed shadow-none"
               )}
             >
               <Save size={14} /> 
               <span className="hidden sm:inline">Commit Changes</span>
               <span className="sm:hidden">Save</span>
             </button>
          </div>
        </div>
      </footer>
    </div>
  );
};
