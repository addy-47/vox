import React from 'react';

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
    <header className="mb-4 sm:mb-6 lg:mb-8 relative shrink-0">
      <div className="flex justify-between items-start gap-4">
        <div className="flex-1 min-w-0">
          <div className="mb-2 sm:mb-3">
            <span
              className="text-[11px] sm:text-[12px] font-mono font-bold tracking-[0.14em] uppercase"
              style={{ color: effectiveColor }}
            >
              {step}
            </span>
          </div>
          <h1 className="text-2xl sm:text-3xl lg:text-4xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight uppercase mb-2 sm:mb-3">
            {title}
          </h1>
          <p className="text-[rgb(var(--foreground-muted))] text-xs sm:text-sm leading-relaxed max-w-md">
            {description}
          </p>
        </div>
        {rightContent && (
          <div className="flex flex-col items-end text-right pt-1 shrink-0">
            {rightContent}
          </div>
        )}
      </div>
    </header>
  );
};

