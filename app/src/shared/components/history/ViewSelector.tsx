import { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { HISTORY_COPY } from "@/data/historyCopy";

export type HistoryView = "day" | "month";

export interface ViewSelectorProps {
  view: HistoryView;
  onChange: (view: HistoryView) => void;
}

export const ViewSelector = memo(({ view, onChange }: ViewSelectorProps) => {
  const options: { value: HistoryView; label: string }[] = [
    { value: "day", label: HISTORY_COPY.viewDay },
    { value: "month", label: HISTORY_COPY.viewMonth },
  ];

  return (
    <div
      className="flex items-center gap-4 select-none"
      role="group"
      aria-label={HISTORY_COPY.viewSelectorLabel}
    >
      {options.map((option) => {
        const isActive = view === option.value;
        return (
          <button
            key={option.value}
            onClick={() => onChange(option.value)}
            aria-pressed={isActive}
            className={cn(
              "text-[12px] font-mono font-bold uppercase tracking-[0.2em] transition-all cursor-pointer relative pb-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              isActive
                ? "text-[rgb(var(--accent))] drop-shadow-[0_0_8px_rgba(var(--accent),0.6)]"
                : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] opacity-60 hover:opacity-100"
            )}
          >
            {option.label}
            {isActive && (
              <span className="absolute bottom-0 left-0 right-0 h-[2px] bg-[rgb(var(--accent))] rounded-full shadow-[0_0_6px_rgb(var(--accent))]" />
            )}
          </button>
        );
      })}
    </div>
  );
});

ViewSelector.displayName = "ViewSelector";