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
  color = '#00dbe9',
  rightContent
}) => {
  return (
    <header className="mb-8 relative">
      <div className="flex justify-between items-start">
        <div className="flex-1">
          <div className="flex items-center gap-4 mb-4">
            <div className="h-[1px] w-8 transition-colors duration-500" style={{ backgroundColor: `${color}4D` }} />
            <span className="text-[11px] font-black tracking-[0.4em] uppercase transition-colors duration-500" style={{ color }}>
              {step}
            </span>
          </div>
          <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">
            {title}
          </h1>
          <p className="text-white/40 text-sm leading-relaxed max-w-md">
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

