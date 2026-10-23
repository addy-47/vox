import { memo, type ReactNode } from "react";
import { ArrowLeft, ChevronRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CARD_HEADER_COPY } from "@/data/settingsCopy";

export interface CardTitleBarProps {
  icon: ReactNode;
  title: string;
  /** Breadcrumb back arrow for nested views. */
  onBack?: () => void;
  /** Explicit override for nested view styling (defaults to Boolean(onBack)). */
  isNested?: boolean;
  /** Breadcrumb label on the right (e.g. Select voice). Omit for a bare header. */
  actionLabel?: string;
  onAction?: () => void;
  /** Custom trailing control. Takes precedence over `actionLabel`. */
  action?: ReactNode;
}

/**
 * The quiet header shared by settings cards: a glyph, an uppercase card name,
 * an optional breadcrumb back arrow, and one breadcrumb control on the right
 * that swaps the card face. It carries no search, sort, or filter — those
 * belong to the catalog header of the view they control.
 *
 * When nested (Step 2), icon and title step down to match CardHeader:
 * icon size 14px ([&>svg]:w-3.5 [&>svg]:h-3.5) and text-[12px] sm:text-[12.5px] font-bold.
 */
export const CardTitleBar = memo(({
  icon,
  title,
  onBack,
  isNested: explicitNested,
  actionLabel,
  onAction,
  action,
}: CardTitleBarProps) => {
  const isNested = explicitNested ?? Boolean(onBack);

  return (
    <div className="flex items-center justify-between gap-2 mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
      <div className="flex items-center gap-1.5 min-w-0">
        {onBack && (
          <>
            <Tooltip label={CARD_HEADER_COPY.back} side="bottom" asChild>
              <button
                type="button"
                onClick={onBack}
                className="inline-flex items-center gap-1 text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0 group"
                aria-label={CARD_HEADER_COPY.back}
              >
                <ArrowLeft
                  size={12}
                  strokeWidth={2.5}
                  className="group-hover:-translate-x-0.5 transition-transform"
                />
              </button>
            </Tooltip>
            <span className="text-[rgb(var(--foreground-muted))]/30 text-[10px] shrink-0">/</span>
          </>
        )}
        <span
          className={cn(
            "shrink-0 text-[rgb(var(--accent))]",
            isNested && "[&>svg]:w-3.5 [&>svg]:h-3.5"
          )}
        >
          {icon}
        </span>
        <span
          className={cn(
            "truncate",
            isNested
              ? "font-display font-bold text-[12px] sm:text-[12.5px] text-[rgb(var(--foreground))]/90"
              : "font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]"
          )}
        >
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
  );
});

CardTitleBar.displayName = "CardTitleBar";
