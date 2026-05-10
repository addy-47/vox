import React, { useState } from "react";
import { Save, Sun, Moon, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { CoreSettings } from "@/shared/components/CoreSettings";
import { TraySettings } from "@/shared/components/TraySettings";

export const Settings: React.FC = () => {
  const { draftSettings, hasChanges, toggleTheme, commitChanges, discardChanges, restoreDefaults } = useSettings();
  const [activeTab, setActiveTab] = useState<"core" | "tray">("core");

  if (!draftSettings) return null;

  const theme = draftSettings.ui.theme;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-[rgb(var(--background))]">
      {/* Page header - Fixed */}
      <header className="border-b border-[rgba(var(--border),0.05)] glass-panel shrink-0">
        <div className="max-w-7xl mx-auto w-full px-6 md:px-10 py-6 md:py-10 flex flex-col md:flex-row md:items-center justify-between gap-6">
          <div className="flex items-start justify-between w-full md:w-auto">
            <div className="space-y-2">
              <div className="flex items-center gap-2 mb-1">
                <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                <span className="text-[10px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Configuration</span>
              </div>
              <h1 className="text-2xl md:text-3xl font-bold text-[rgb(var(--foreground))] tracking-tight">System <span className="text-[rgb(var(--foreground-muted))] opacity-40 font-medium">Core</span></h1>
            </div>

            <div className="flex items-center gap-1 md:hidden">
              <button 
                onClick={restoreDefaults}
                className="p-2.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-white/[0.03] transition-colors duration-300"
                title="Restore Defaults"
              >
                <RotateCcw size={18} strokeWidth={1.5} />
              </button>
              <button
                onClick={toggleTheme}
                className="p-2.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-white/[0.03] transition-colors duration-300"
                title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
              >
                {theme === 'dark'
                  ? <Sun size={18} strokeWidth={1.5} />
                  : <Moon size={18} strokeWidth={1.5} />}
              </button>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5 p-1 bg-white/[0.03] border border-[rgba(var(--border),0.05)] rounded-2xl">
              <button 
                onClick={() => setActiveTab("core")}
                className={cn(
                  "px-5 py-2 rounded-xl text-[10px] font-bold uppercase tracking-widest transition-all duration-300",
                  activeTab === "core" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Core
              </button>
              <button 
                onClick={() => setActiveTab("tray")}
                className={cn(
                  "px-5 py-2 rounded-xl text-[10px] font-bold uppercase tracking-widest transition-all duration-300",
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
      <div className="flex-1 overflow-hidden relative">
        <div className="h-full max-w-7xl mx-auto px-6 md:px-10 py-6 md:py-8">
          {activeTab === "core" ? (
            <CoreSettings />
          ) : (
            <TraySettings />
          )}
        </div>
      </div>

      {/* Footer Actions - Fixed at bottom */}
      <footer className="shrink-0 border-t border-[rgba(var(--border),0.05)] glass-panel bg-[rgb(var(--background))]/80 backdrop-blur-xl">
        <div className="max-w-7xl mx-auto px-6 md:px-10 py-6 md:py-8 flex items-center justify-between gap-4">
          <button 
            onClick={restoreDefaults}
            className="hidden md:flex items-center gap-2 text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:text-red-400 transition-colors duration-300 opacity-50 hover:opacity-100"
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
                 "flex-1 md:flex-none px-4 md:px-8 py-2.5 rounded-xl text-[10px] font-bold uppercase tracking-[0.2em] transition-all duration-300",
                 hasChanges 
                  ? "text-[rgb(var(--foreground))] hover:bg-white/[0.05]" 
                  : "text-[rgb(var(--foreground-muted))] opacity-20 cursor-not-allowed"
               )}
             >
               <span className="hidden sm:inline">Discard Changes</span>
               <span className="sm:hidden">Discard</span>
             </button>
             <button 
               onClick={commitChanges}
               disabled={!hasChanges}
               className={cn(
                 "flex-1 md:flex-none px-6 md:px-10 py-2.5 rounded-xl font-bold text-[10px] tracking-[0.2em] uppercase flex items-center justify-center gap-2 transition-all duration-300 shadow-lg",
                 hasChanges
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-105 active:scale-95 shadow-[rgb(var(--accent))]/20"
                  : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] opacity-40 cursor-not-allowed shadow-none"
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
