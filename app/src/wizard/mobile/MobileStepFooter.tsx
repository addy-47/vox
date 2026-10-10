import React from 'react';

interface MobileStepFooterProps {
  currentIndex: number;
  totalSteps: number;
  label: string;
  onClick: () => void;
  isLoading?: boolean;
  disabled?: boolean;
}

export const MobileStepFooter: React.FC<MobileStepFooterProps> = ({
  currentIndex,
  totalSteps,
  label,
  onClick,
  isLoading = false,
  disabled = false,
}) => {
  return (
    <div className="w-full px-6 pt-2 pb-[max(1.75rem,env(safe-area-inset-bottom))] shrink-0 bg-transparent flex flex-col items-center gap-3.5 select-none z-30 pointer-events-auto">
      {/* Fixed Step Transition Slider (Stays stationary while center content swipes) */}
      <div className="flex items-center justify-center gap-2 py-1">
        {Array.from({ length: totalSteps }).map((_, i) => (
          <div
            key={i}
            className={`h-1.5 rounded-full transition-all duration-300 ${
              i === currentIndex
                ? 'w-7 bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]'
                : 'w-2 bg-[rgba(var(--foreground),0.15)]'
            }`}
          />
        ))}
      </div>

      {/* Contextual Action Button (Clean button without arrows or back button) */}
      <div className="w-full max-w-sm">
        <button
          type="button"
          onClick={onClick}
          disabled={disabled || isLoading}
          className="w-full h-12 bg-[rgba(var(--accent),0.14)] hover:bg-[rgba(var(--accent),0.22)] active:bg-[rgba(var(--accent),0.3)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--foreground))] hover:text-white font-display font-semibold text-[13px] tracking-wide rounded-xl flex items-center justify-center shadow-[0_2px_16px_rgba(var(--accent),0.1)] active:scale-[0.98] transition-all touch-manipulation cursor-pointer disabled:opacity-40"
        >
          <span>{label}</span>
        </button>
      </div>
    </div>
  );
};
