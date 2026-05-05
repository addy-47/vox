import React from 'react';
import { Cpu, Zap } from 'lucide-react';

interface SystemStats {
  cpu_usage: number;
  memory_used_mb: number;
}

interface FooterProps {
  stats: SystemStats | null;
}

export const Footer: React.FC<FooterProps> = ({ stats }) => {
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
       <div className="text-[9px] font-black text-white/10 tracking-[0.2em] uppercase">
          Obsidian Engine v0.3
       </div>
    </div>
  );
};
