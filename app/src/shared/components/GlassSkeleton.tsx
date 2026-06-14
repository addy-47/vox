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
      <div className={cn('glass p-5 space-y-4', !noPulse && 'animate-pulse', className)}>
        <div className="h-3 glass rounded w-1/4" />
        <div className="h-4 glass rounded w-3/4" />
        <div className="h-3 glass rounded w-1/2" />
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
