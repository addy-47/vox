import React from 'react';
import { Cpu, Zap } from 'lucide-react';

interface SystemStats {
  cpu_usage: number;
  memory_used_mb: number;
}

interface FooterProps {
  stats: SystemStats | null;
  onPrev?: () => void;
  onNext?: () => void;
  historyIndex: number;
  viewingHistory: boolean;
  historyCount: number;
}

export const Footer: React.FC<FooterProps> = ({ 
  stats, 
  onPrev, 
  onNext, 
  historyIndex, 
  viewingHistory, 
  historyCount 
}) => {
  return (
    <div className="px-7 py-4 mt-auto flex items-center justify-between z-10 min-h-[60px]">
       <div className="flex items-center gap-6 opacity-40 hover:opacity-100 transition-opacity">
          {stats ? (
            <>
              <div className="flex items-center gap-2">
                <Cpu size={12} className="text-[rgb(var(--accent))]" />
                <span className="text-[10px] font-mono text-[rgb(var(--foreground))]/80 font-bold">{stats.cpu_usage.toFixed(1)}%</span>
              </div>
              <div className="flex items-center gap-2">
                <Zap size={12} className="text-[rgb(var(--accent))]" />
                <span className="text-[10px] font-mono text-[rgb(var(--foreground))]/80 font-bold">{stats.memory_used_mb}MB</span>
              </div>
            </>
          ) : (
            <div className="text-[9px] font-mono text-[rgb(var(--foreground))]/20 uppercase tracking-widest">System Ready</div>
          )}
       </div>
       
       <div className="flex items-center gap-1">
          <button 
            onClick={onPrev}
            disabled={historyIndex === 0 || historyCount === 0}
            className="p-1.5 rounded-md hover:bg-[rgb(var(--accent))]/10 disabled:opacity-30 transition-all text-[rgb(var(--accent))] hover:scale-110 active:scale-95"
            title="Previous Transcription"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="m15 18-6-6 6-6"/></svg>
          </button>
          <button 
            onClick={onNext}
            disabled={!viewingHistory || historyCount === 0}
            className="p-1.5 rounded-md hover:bg-[rgb(var(--accent))]/10 disabled:opacity-30 transition-all text-[rgb(var(--accent))] hover:scale-110 active:scale-95"
            title="Next Transcription"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
          </button>
       </div>
    </div>
  );
};
