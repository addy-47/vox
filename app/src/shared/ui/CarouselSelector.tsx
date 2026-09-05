import { memo, ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { cn } from "@/shared/lib/utils";

export interface CarouselSelectorProps {
  label?: string;
  value: string;
  onPrev: () => void;
  onNext: () => void;
  icon?: ReactNode;
  containerClassName?: string;
  className?: string;
  disabled?: boolean;
}

export const CarouselSelector = memo(
  ({
    label,
    value,
    onPrev,
    onNext,
    icon,
    containerClassName,
    className,
    disabled = false,
  }: CarouselSelectorProps) => {
    return (
      <div className={cn("space-y-1 w-full", containerClassName)}>
        {label && (
          <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5 block tracking-wider">
            {label}
          </label>
        )}

        <div
          className={cn(
            "flex items-center justify-between bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] rounded-lg h-[34px] px-1.5 transition-all select-none",
            disabled && "opacity-40 pointer-events-none",
            className
          )}
        >
          <button
            type="button"
            onClick={onPrev}
            disabled={disabled}
            className="p-1 rounded text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all active:scale-90 cursor-pointer flex items-center justify-center shrink-0"
            title={LAYOUT_COPY.carousel.previous}
            aria-label={LAYOUT_COPY.carousel.previousItem}
          >
            <ChevronLeft size={16} />
          </button>

          <div className="flex items-center justify-center gap-1.5 min-w-0 px-1.5 flex-1">
            {icon && <span className="shrink-0 text-[rgb(var(--accent))]">{icon}</span>}
            <span className="text-[12px] font-bold text-[rgb(var(--accent))] uppercase tracking-wider truncate text-center animate-fade-in">
              {value}
            </span>
          </div>

          <button
            type="button"
            onClick={onNext}
            disabled={disabled}
            className="p-1 rounded text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all active:scale-90 cursor-pointer flex items-center justify-center shrink-0"
            title={LAYOUT_COPY.carousel.next}
            aria-label={LAYOUT_COPY.carousel.nextItem}
          >
            <ChevronRight size={16} />
          </button>
        </div>
      </div>
    );
  }
);

CarouselSelector.displayName = "CarouselSelector";
