import React from "react";
import { Save, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { CoreSettings } from "@/shared/components/CoreSettings";
import { ModelSettings } from "@/shared/components/ModelSettings";
import { TraySettings } from "@/shared/components/TraySettings";
import { GlassSkeleton } from "@/shared/components/GlassSkeleton";

export const Settings: React.FC = () => {
  const { draftSettings, hasChanges, commitChanges, discardChanges, restoreDefaults } = useSettings();

  if (!draftSettings) return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent px-6 md:px-10 py-6 md:py-10 items-center justify-center">
      <div className="w-full max-w-md space-y-6">
        <GlassSkeleton variant="card" />
        <GlassSkeleton variant="card" />
      </div>
    </div>
  );

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none">
      
      {/* ── Header ──────────────────────────────────────────────────────────── */}
      <header className="px-6 md:px-10 py-6 md:py-8 border-b border-[rgba(var(--accent),0.08)] shrink-0">
        <div className="max-w-[1600px] mx-auto flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="space-y-1">
            <div className="flex items-center gap-3">
              <span className="signal-text text-[14px]">SYSTEM configuration</span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-light uppercase tracking-wider">
              Configure engine intelligence parameters, interface traits, and assistant behaviors.
            </p>
          </div>
        </div>
      </header>

      {/* ── Scrollable Settings Panel ───────────────────────────────────────── */}
      <div className="flex-1 overflow-y-auto custom-scrollbar px-6 md:px-10">
        <div className="max-w-[1600px] mx-auto py-8 space-y-16 pb-28">
          
          {/* Section 1: Core Engine Settings */}
          <div className="space-y-6">
            <div className="text-[10px] font-mono tracking-[0.25em] text-[rgb(var(--accent))]/75 uppercase flex items-center gap-4">
              <span>── CORE INTELLIGENCE ──</span>
              <div className="h-[1px] flex-1 bg-[rgba(var(--accent),0.08)]" />
            </div>
            <CoreSettings />
          </div>

          {/* Section 2: HUD & Overlay Interface */}
          <div className="space-y-6">
            <div className="text-[10px] font-mono tracking-[0.25em] text-[rgb(var(--accent))]/75 uppercase flex items-center gap-4">
              <span>── HUD & DISPLAY INTERFACE ──</span>
              <div className="h-[1px] flex-1 bg-[rgba(var(--accent),0.08)]" />
            </div>
            <TraySettings />
          </div>

          {/* Section 3: Models Catalog */}
          <div className="space-y-6">
            <div className="text-[10px] font-mono tracking-[0.25em] text-[rgb(var(--accent))]/75 uppercase flex items-center gap-4">
              <span>── ENGINE MODELS CATALOG ──</span>
              <div className="h-[1px] flex-1 bg-[rgba(var(--accent),0.08)]" />
            </div>
            <ModelSettings />
          </div>

        </div>
      </div>

      {/* ── Sticky Control Footer ───────────────────────────────────────────── */}
      <footer className="shrink-0 bg-black/40 backdrop-blur-xl border-t border-[rgba(var(--accent),0.08)] px-6 md:px-10 py-4 z-20">
        <div className="max-w-[1600px] mx-auto flex items-center justify-between gap-4">
          <button 
            onClick={restoreDefaults}
            className="flex items-center gap-2 text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.15em] hover:text-red-400 transition-colors duration-300 opacity-60 hover:opacity-100 hover:bg-red-500/10 px-3 py-1.5 rounded-lg border border-transparent hover:border-red-500/20"
            aria-label="Restore Defaults"
          >
             <RotateCcw size={13} />
             <span>Restore Defaults</span>
          </button>
          
          <div className="flex items-center gap-3">
             <button 
               onClick={discardChanges}
               disabled={!hasChanges}
               className={cn(
                  "px-6 py-2 rounded-xl text-[10px] font-bold uppercase tracking-[0.15em] transition-all duration-300 border",
                  hasChanges 
                   ? "text-[rgb(var(--foreground))] border-[rgba(var(--accent),0.2)] bg-black/20 hover:border-[rgb(var(--accent))]/40" 
                   : "text-[rgb(var(--foreground-muted))]/40 border-transparent cursor-not-allowed"
               )}
             >
               Discard
             </button>
             
             <button 
               onClick={commitChanges}
               disabled={!hasChanges}
               className={cn(
                 "px-8 py-2 rounded-xl font-bold text-[10px] tracking-[0.15em] uppercase flex items-center justify-center gap-2 transition-all duration-300",
                 hasChanges
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/15 hover:scale-102 active:scale-98"
                  : "bg-[rgb(var(--foreground))]/[0.02] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
               )}
             >
               <Save size={12} /> 
               <span>Save Changes</span>
             </button>
          </div>
        </div>
      </footer>
    </div>
  );
};
