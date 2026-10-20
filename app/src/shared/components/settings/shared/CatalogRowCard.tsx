import { memo, type ReactNode } from "react";
import { cn } from "@/shared/lib/utils";

export interface CatalogRowCardProps {
  /** Slot for the leading mark (model glyph, provider logo). */
  leading: ReactNode;
  title: string;
  /** One dim line under the title: tier, footprint, endpoint, status. */
  meta: string;
  /** Persistent trailing controls. Never hover-only: they are the row's actions. */
  trailing?: ReactNode;
  active?: boolean;
  /** Dims the row and blocks selection (missing weights, unavailable provider). */
  disabled?: boolean;
  onSelect?: () => void;
  ariaLabel?: string;
  className?: string;
  /** Unconnected rows: dimmer by colour, not by opacity. */
  muted?: boolean;
  /**
   * Replaces the leading mark and the text block. A row that hosts an input
   * swaps its own content instead of growing a second block below.
   */
  body?: ReactNode;
}

/**
 * One row in a settings catalog. The shared geometry for every picker row:
 * a 36px leading mark, a two-line text block, and a trailing control cluster.
 *
 * Selection never changes geometry — same 1px border, same box. Only colour
 * and a faint wash move, so the chosen row cannot read larger than its
 * neighbours. Callers own the trailing controls; this shell owns the hit area,
 * keyboard semantics, and the disabled/dimmed state.
 */
export const CatalogRowCard = memo(
  ({
    leading,
    title,
    meta,
    trailing,
    active = false,
    disabled = false,
    onSelect,
    ariaLabel,
    className,
    muted = false,
    body,
  }: CatalogRowCardProps) => {
    const interactive = !disabled;

    return (
      <div className="w-full">
        <div
          role="button"
          tabIndex={interactive ? 0 : -1}
          aria-disabled={!interactive}
          aria-label={ariaLabel ?? title}
          aria-pressed={interactive ? active : undefined}
          onClick={() => {
            if (interactive) onSelect?.();
          }}
          onKeyDown={(e) => {
            if ((e.key === "Enter" || e.key === " ") && interactive) {
              e.preventDefault();
              onSelect?.();
            }
          }}
          className={cn(
            "group relative flex w-full select-none items-center gap-2.5 rounded-xl border",
            "min-h-[64px] py-2.5 pl-2.5 pr-2.5 transition-colors duration-200",
            muted
              ? "border-[rgba(var(--foreground),0.045)] bg-[rgba(var(--foreground),0.015)]"
              : "border-[rgba(var(--foreground),0.07)] bg-[rgba(var(--foreground),0.035)]",
            interactive &&
              "cursor-pointer hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--foreground),0.06)]",
            disabled && "opacity-70",
            active &&
              "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.07)] hover:bg-[rgba(var(--accent),0.07)]",
            className
          )}
        >
          {body ? (
            <div className="flex min-w-0 flex-1 items-center gap-2">{body}</div>
          ) : (
            <>
              <span className="shrink-0">{leading}</span>

              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span
                  className={cn(
                    "truncate text-[13px] font-semibold leading-tight tracking-tight",
                    active
                      ? "text-[rgb(var(--accent))]"
                      : muted
                        ? "text-[rgb(var(--foreground-muted))]"
                        : "text-[rgb(var(--foreground))]"
                  )}
                >
                  {title}
                </span>
                <span
                  className={cn(
                    "truncate text-[10.5px] leading-tight",
                    muted
                      ? "text-[rgb(var(--foreground-muted))]/55"
                      : "text-[rgb(var(--foreground-muted))]"
                  )}
                >
                  {meta}
                </span>
              </div>
            </>
          )}

          {trailing && <div className="flex shrink-0 items-center gap-0.5">{trailing}</div>}
        </div>
      </div>
    );
  }
);

CatalogRowCard.displayName = "CatalogRowCard";
