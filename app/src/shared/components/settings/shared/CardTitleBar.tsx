import { memo, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

export interface CardTitleBarProps {
  icon: ReactNode;
  title: string;
  /** Breadcrumb label on the right (e.g. Select voice). Omit for a bare header. */
  actionLabel?: string;
  onAction?: () => void;
  /** Custom trailing control. Takes precedence over `actionLabel`. */
  action?: ReactNode;
}

/**
 * The quiet header shared by the speech and language cards: a glyph, an
 * uppercase card name, and one breadcrumb control on the right that swaps the
 * card face. It carries no search, sort, or filter — those belong to the
 * catalog header of the view they control.
 */
export const CardTitleBar = memo(({
  icon,
  title,
  actionLabel,
  onAction,
  action,
}: CardTitleBarProps) => (
  <div className="flex items-center justify-between gap-2 mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
    <div className="flex items-center gap-2 min-w-0">
      <span className="shrink-0 text-[rgb(var(--accent))]">{icon}</span>
      <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
        {title}
      </span>
    </div>

    {action ??
      (actionLabel && onAction ? (
        <button
          type="button"
          onClick={onAction}
          className="group flex items-center gap-1.5 text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
        >
          <span className="text-[11.5px] font-bold tracking-tight whitespace-nowrap">
            {actionLabel}
          </span>
          <ChevronRight
            size={12}
            strokeWidth={2.5}
            className="shrink-0 opacity-70 transition-transform group-hover:translate-x-0.5"
          />
        </button>
      ) : null)}
  </div>
));

CardTitleBar.displayName = "CardTitleBar";
