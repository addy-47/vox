import React from 'react';
import { cn } from '@/shared/lib/utils';

interface GlassSkeletonProps {
  /** Number of skeleton lines to render */
  lines?: number;
  /** Optional className for the outer container */
  className?: string;
  /** Width variant: one line or full block */
  variant?: 'text' | 'card' | 'circle';
  /** Custom width override (e.g. 'w-3/4', 'w-48') */
  width?: string;
  /** Optional pulse animation disable */
  noPulse?: boolean;
}

export const GlassSkeleton: React.FC<GlassSkeletonProps> = ({
  lines = 1,
  className,
  variant = 'text',
  width,
  noPulse = false,
}) => {
  if (variant === 'circle') {
    return (
      <div className={cn('flex items-center justify-center', className)}>
        <div className={cn(
          'glass rounded-full',
          width || 'w-10 h-10',
          !noPulse && 'animate-pulse'
        )} />
      </div>
    );
  }

  if (variant === 'card') {
    return (
      <div
        className={cn(
          'relative overflow-hidden rounded-2xl glass p-5 space-y-4 border border-[rgba(var(--accent),0.14)]',
          !noPulse && 'shadow-[0_0_28px_rgba(var(--accent),0.08)]',
          className
        )}
      >
        {/* ambient accent glow orb */}
        {!noPulse && (
          <span
            className="absolute -top-4 -right-4 w-16 h-16 rounded-full pointer-events-none"
            style={{ background: 'radial-gradient(circle, rgba(var(--accent),0.18) 0%, transparent 70%)' }}
          />
        )}

        {/* shimmer sweep */}
        {!noPulse && (
          <span className="absolute inset-y-0 left-0 w-1/2 -skew-x-12 pointer-events-none bg-gradient-to-r from-transparent via-[rgba(var(--accent),0.08)] to-transparent animate-[skeleton-shimmer_1.6s_ease-in-out_infinite]" />
        )}

        {/* charging header: brand bar + pulsing accent orb */}
        <div className="flex items-center justify-between">
          <div className="h-3 rounded w-1/4 bg-[rgba(var(--foreground),0.10)]" />
          {!noPulse && (
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full rounded-full bg-[rgb(var(--accent))] opacity-50 animate-ping" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-[rgb(var(--accent))]" />
            </span>
          )}
        </div>

        <div className="h-4 rounded w-3/4 bg-[rgba(var(--foreground),0.08)]" />
        <div className="h-3 rounded w-1/2 bg-[rgba(var(--foreground),0.06)]" />
        <div className="h-3 rounded w-2/3 bg-[rgba(var(--foreground),0.05)]" />
      </div>
    );
  }

  // variant === 'text'
  return (
    <div className={cn('space-y-2', className)}>
      {Array.from({ length: lines }).map((_, i) => (
        <div
          key={i}
          className={cn(
            'h-3 glass rounded',
            !noPulse && 'animate-pulse',
            width || (i === lines - 1 ? 'w-3/4' : 'w-full')
          )}
        />
      ))}
    </div>
  );
};
