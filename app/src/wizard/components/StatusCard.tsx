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
    "glass px-5 py-5 group transition-all duration-500 hover:bg-[rgba(var(--foreground),0.06)] relative overflow-hidden",
    loading && "opacity-70"
  )}>
    {/* Subtle Loading Bar */}
    {loading && (
        <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-[rgb(var(--accent))]/40 to-transparent animate-[shimmer_2s_infinite]" />
    )}

    <div className="flex items-center justify-between mb-4 relative z-10">
      <div className={cn(
        "w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-500",
        loading ? 'bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40' : 
        ok ? 'bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.1)]' : 'bg-red-500/10 text-red-400'
      )}>
        {icon}
      </div>
      
      <div className={cn(
        "w-5 h-5 rounded-full flex items-center justify-center border transition-all duration-500",
        loading ? 'border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--foreground),0.05)]' : 
        ok ? 'bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]' : 'bg-red-500/10 border-red-500/20 text-red-400'
      )}>
        {loading ? (
            <Loader2 className="w-2.5 h-2.5 animate-spin text-[rgb(var(--foreground-muted))]/40" />
        ) : ok ? (
            <Check className="w-2.5 h-2.5" />
        ) : (
            <X className="w-2.5 h-2.5" />
        )}
      </div>
    </div>

    <div className="relative z-10">
        <div className="text-[12px] font-bold text-[rgb(var(--foreground))]/80 tracking-widest uppercase mb-1">{label}</div>
        <div className={cn(
        "text-sm font-medium leading-none mb-1 transition-colors duration-500",
        loading ? "text-[rgb(var(--foreground-muted))]/40" : "text-[rgb(var(--foreground))]"
        )}>{value}</div>
        {subValue && (
        <div className={cn(
            "text-[12px] font-medium transition-colors duration-500",
            loading ? "text-[rgb(var(--foreground-muted))]/30" : "text-[rgb(var(--foreground-muted))]/80"
        )}>{subValue}</div>
        )}
    </div>
  </div>
);

