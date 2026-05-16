import React from 'react';
import { Check, X, Loader2 } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface StatusCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  subValue?: string;
  ok?: boolean;
  loading?: boolean;
}

export const StatusCard: React.FC<StatusCardProps> = ({ 
  icon, 
  label, 
  value, 
  subValue, 
  ok, 
  loading 
}) => (
  <div className={cn(
    "p-5 bg-white/[0.02] border border-white/5 rounded-2xl group transition-all duration-500 hover:bg-white/[0.04] hover:border-white/10 relative overflow-hidden",
    loading && "opacity-70"
  )}>
    {/* Subtle Loading Bar */}
    {loading && (
        <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-[#00dbe9]/40 to-transparent animate-[shimmer_2s_infinite]" />
    )}

    <div className="flex items-center justify-between mb-4 relative z-10">
      <div className={cn(
        "w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-500",
        loading ? 'bg-white/5 text-white/10' : 
        ok ? 'bg-[#00dbe9]/10 text-[#00dbe9] shadow-[0_0_15px_rgba(0,219,233,0.1)]' : 'bg-red-500/10 text-red-400'
      )}>
        {icon}
      </div>
      
      <div className={cn(
        "w-5 h-5 rounded-full flex items-center justify-center border transition-all duration-500",
        loading ? 'border-white/5 bg-white/5' : 
        ok ? 'bg-[#00dbe9]/10 border-[#00dbe9]/20 text-[#00dbe9]' : 'bg-red-500/10 border-red-500/20 text-red-400'
      )}>
        {loading ? (
            <Loader2 className="w-2.5 h-2.5 animate-spin text-white/20" />
        ) : ok ? (
            <Check className="w-2.5 h-2.5" />
        ) : (
            <X className="w-2.5 h-2.5" />
        )}
      </div>
    </div>

    <div className="relative z-10">
        <div className="text-[11px] font-bold text-white/80 tracking-widest uppercase mb-1">{label}</div>
        <div className={cn(
        "text-sm font-medium leading-none mb-1 transition-colors duration-500",
        loading ? "text-white/20" : "text-white"
        )}>{value}</div>
        {subValue && (
        <div className={cn(
            "text-[11px] font-medium transition-colors duration-500",
            loading ? "text-white/10" : "text-white/50"
        )}>{subValue}</div>
        )}
    </div>
  </div>
);

