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
  if (!stats) return null;

  return (
    <div className="px-7 py-4 mt-auto flex items-center justify-between z-10">
       <div className="flex items-center gap-6 opacity-40 hover:opacity-100 transition-opacity">
          <div className="flex items-center gap-2">
            <Cpu size={12} className="text-cyan-400" />
            <span className="text-[10px] font-mono text-white/80 font-bold">{stats.cpu_usage.toFixed(1)}%</span>
          </div>
          <div className="flex items-center gap-2">
            <Zap size={12} className="text-cyan-400" />
            <span className="text-[10px] font-mono text-white/80 font-bold">{stats.memory_used_mb}MB</span>
          </div>
       </div>
       
       {historyCount > 0 && (
         <div className="flex items-center gap-2 bg-white/5 rounded-lg p-0.5 border border-white/5">
            <button 
              onClick={onPrev}
              disabled={historyIndex === 0}
              className="p-1.5 rounded-md hover:bg-white/5 disabled:opacity-10 transition-all text-white/40 hover:text-cyan-400"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="m15 18-6-6 6-6"/></svg>
            </button>
            <div className="w-[1px] h-3 bg-white/10" />
            <button 
              onClick={onNext}
              disabled={!viewingHistory}
              className="p-1.5 rounded-md hover:bg-white/5 disabled:opacity-10 transition-all text-white/40 hover:text-cyan-400"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="m9 18 6-6-6-6"/></svg>
            </button>
         </div>
       )}
    </div>
  );
};
