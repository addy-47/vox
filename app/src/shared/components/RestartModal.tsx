import React from "react";
import { RefreshCcw, AlertTriangle, X } from "lucide-react";

interface RestartModalProps {
  isOpen: boolean;
  onClose: () => void;
  onRestart: () => void;
  changedSettings: string[];
}

export const RestartModal: React.FC<RestartModalProps> = ({
  isOpen,
  onClose,
  onRestart,
  changedSettings,
}) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center p-6">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-black/60 backdrop-blur-sm animate-in fade-in duration-300"
        onClick={onClose}
      />
      
      {/* Modal Card */}
      <div className="relative w-full max-w-md glass-elevated glass-base p-8 space-y-8 animate-in zoom-in-95 fade-in duration-300 shadow-2xl transform-gpu">
        <button 
          onClick={onClose}
          className="absolute top-4 right-4 p-2 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
        >
          <X size={18} />
        </button>

        <div className="flex flex-col items-center text-center space-y-6">
          <div className="w-16 h-16 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 flex items-center justify-center text-[rgb(var(--accent))]">
            <RefreshCcw size={32} className="animate-spin" />
          </div>

          <div className="space-y-2">
            <h2 className="text-2xl font-bold text-[rgb(var(--foreground))] tracking-tight">Restart Required</h2>
            <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed">
              To apply changes to the following core systems, a full application relaunch is required:
            </p>
          </div>

          <div className="w-full bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] rounded-xl p-4 space-y-2">
            {changedSettings.map((setting) => (
              <div key={setting} className="flex items-center gap-3 text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest">
                <div className="w-1 h-1 rounded-full bg-[rgb(var(--accent))]" />
                {setting.replace("_", " ")}
              </div>
            ))}
          </div>

          <div className="flex items-center gap-3 w-full pt-4">
            <button 
              onClick={onClose}
              className="flex-1 px-6 py-3.5 rounded-xl text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest hover:bg-[rgb(var(--foreground))]/[0.05] transition-colors"
            >
              Later
            </button>
            <button 
              onClick={onRestart}
              className="flex-1 px-6 py-3.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-bold text-[11px] uppercase tracking-widest flex items-center justify-center gap-3 hover:brightness-110 active:opacity-90 transition-all shadow-lg shadow-[rgb(var(--accent))]/20"
            >
              <RefreshCcw size={14} /> Relaunch Now
            </button>
          </div>
        </div>

        <div className="flex items-start gap-3 p-4 bg-yellow-500/5 border border-yellow-500/20 rounded-xl">
          <AlertTriangle size={16} className="text-yellow-500 shrink-0 mt-0.5" />
          <p className="text-[11px] text-yellow-500/80 leading-relaxed font-medium">
            Active sessions and unsaved transcripts will be cleared upon relaunch. Ensure all tasks are completed.
          </p>
        </div>
      </div>
    </div>
  );
};
