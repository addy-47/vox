import React from 'react';
import { cn } from '@/shared/lib/utils';

interface WizardHeaderProps {
  step: string;
  title: string;
  description: string;
  color?: string;
  rightContent?: React.ReactNode;
}

export const WizardHeader: React.FC<WizardHeaderProps> = ({ 
  step, 
  title, 
  description, 
  color,
  rightContent
}) => {
  const accentVar = 'rgb(var(--accent))';
  const effectiveColor = color || accentVar;

  return (
    <header className="mb-8 relative shrink-0">
      <div className="flex justify-between items-start">
        <div className="flex-1">
          <div className="flex items-center gap-4 mb-4">
            <div className="h-[1px] w-8" style={{ backgroundColor: `${effectiveColor}4D` }} />
            <span className={cn(
              "inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-[11px] font-black tracking-[0.4em] uppercase glass-whisper"
            )} style={{ color: effectiveColor }}>
              {step}
            </span>
          </div>
          <h1 className="text-4xl font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">
            {title}
          </h1>
          <p className="text-[rgb(var(--foreground-muted))] text-sm leading-relaxed max-w-md">
            {description}
          </p>
        </div>
        {rightContent && (
          <div className="flex flex-col items-end text-right pt-1">
            {rightContent}
          </div>
        )}
      </div>
    </header>
  );
};

