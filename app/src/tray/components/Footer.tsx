import React from 'react';
import { Cpu, Zap } from 'lucide-react';

interface SystemStats {
  system_cpu: number;
  system_ram_pct: number;
  vox_cpu: number;
  vox_ram_mb: number;
  threads: number;
}

interface FooterProps {
  stats: SystemStats | null;
  onPrev?: () => void;
  onNext?: () => void;
  historyIndex: number;
  viewingHistory: boolean;
  historyCount: number;
}

export const Footer: React.FC<FooterProps> = React.memo(({ 
  stats, 
  onPrev, 
  onNext, 
  historyIndex, 
  viewingHistory, 
  historyCount 
}) => {
  return (
    <div className="px-7 py-4 mt-auto flex items-center justify-between z-10 min-h-[60px]">
       <div className="flex items-center gap-3 opacity-80 hover:opacity-100 transition-opacity">
          {stats ? (
            <>
              <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-[rgb(var(--foreground))]/5" aria-label="Vox CPU Usage">
                <Cpu size={12} className="text-[rgb(var(--accent))]" />
                <span className="text-[11px] font-mono text-[rgb(var(--foreground))]/80 font-bold">{stats.vox_cpu.toFixed(1)}%</span>
              </div>
              <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-[rgb(var(--foreground))]/5" aria-label="Vox RAM Usage">
                <Zap size={12} className="text-[rgb(var(--accent))]" />
                <span className="text-[11px] font-mono text-[rgb(var(--foreground))]/80 font-bold">{stats.vox_ram_mb}MB</span>
              </div>
            </>
          ) : (
            <div className="px-2.5 py-1.5 rounded-lg bg-[rgb(var(--foreground))]/5 text-[11px] font-mono text-[rgb(var(--foreground))]/40 uppercase tracking-widest">System Ready</div>
          )}
       </div>
       
       <div className="flex items-center gap-1 px-1.5 py-1 rounded-lg bg-[rgb(var(--foreground))]/5">
          <button 
            onClick={onPrev}
            disabled={historyIndex === 0 || historyCount === 0}
            className="p-1.5 rounded-md text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/5 disabled:opacity-40 disabled:hover:bg-transparent transition-all hover:scale-110 active:scale-90"
            aria-label="Previous Transcription"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="m15 18-6-6 6-6"/></svg>
          </button>
          <button 
            onClick={onNext}
            disabled={!viewingHistory || historyCount === 0}
            className="p-1.5 rounded-md text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/5 disabled:opacity-40 disabled:hover:bg-transparent transition-all hover:scale-110 active:scale-90"
            aria-label="Next Transcription"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
          </button>
       </div>
    </div>
  );
});
