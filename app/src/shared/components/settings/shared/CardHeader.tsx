import { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CSSProperties } from "react";
import { ArrowDownAZ, ArrowLeft, ArrowUpAZ, Filter, Search, X } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { CARD_HEADER_COPY } from "@/data/settingsCopy";

/**
 * The card-level header. Generic by design: any settings card can mount it
 * (the speech card does now, the language card will later).
 *
 * Shape: `[back] icon Title · [action] [filter] [sort] [search]`.
 * Clicking search swaps the right cluster for an underline input in place, so
 * the header never changes height. Every icon slot carries the custom
 * `Tooltip` — native `title` is banned.
 */

export type CardSortOrder = "az" | "za";

export interface CardHeaderFilter {
  /** Tooltip + aria-label for the filter trigger. */
  label: string;
  open: boolean;
  /** Number of active facets; renders the accent dot. */
  activeCount: number;
  onToggle: () => void;
  onClose: () => void;
  /** Popover body. Rendered by `CardFilterMenu` in most call sites. */
  menu: React.ReactNode;
}

export interface CardHeaderSort {
  order: CardSortOrder;
  onToggle: () => void;
}

export interface CardHeaderSearch {
  open?: boolean;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
  onToggle?: () => void;
  onClose?: () => void;
}

export interface CardHeaderProps {
  icon: React.ReactNode;
  title: string;
  /** Breadcrumb back arrow for nested views. */
  onBack?: () => void;
  /** Card-specific action slot (e.g. speech rate). Omitted when absent. */
  action?: React.ReactNode;
  /** Status affordance after the title (fetching, connection failure). */
  status?: React.ReactNode;
  filter?: CardHeaderFilter;
  sort?: CardHeaderSort;
  search?: CardHeaderSearch;
}

const ICON_BUTTON =
  "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]";

const ICON_BUTTON_IDLE =
  "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]";

const ICON_BUTTON_ACTIVE = "text-[rgb(var(--accent))]";

export const CardHeader = memo(function CardHeader({
  icon,
  title,
  onBack,
  action,
  status,
  filter,
  sort,
  search,
}: CardHeaderProps) {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  // Anchors the filter popover: the trigger wrapper, measured on open.
  const filterRef = useRef<HTMLDivElement | null>(null);
  // The popover is portaled to <body>, so outside-click must test it too.
  const popoverRef = useRef<HTMLDivElement | null>(null);

  // The search input replaces the icon cluster, so it must take focus itself.
  useEffect(() => {
    if (search?.open) searchInputRef.current?.focus();
  }, [search?.open]);

  useOverlay({
    active: Boolean(search?.open),
    onClose: () => {
      if (search?.value) search.onChange("");
      else search?.onClose?.();
    },
    dismissOnOutside: false,
  });

  const searchActive = Boolean(search && (search.open || search.value.length > 0));

  return (
    <div className="flex items-center justify-between gap-2 shrink-0 pb-2 border-b border-[rgba(var(--accent),0.08)]">
      {/* Left: breadcrumb + icon + title */}
      <div className="flex items-center gap-1 min-w-0">
        {onBack && (
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
        )}
        {onBack && (
          <span className="text-[rgb(var(--foreground-muted))]/30 text-[10px] shrink-0">/</span>
        )}
        <div className="flex items-center gap-1.5 min-w-0">
          <span className="shrink-0 text-[rgb(var(--accent))]">{icon}</span>
          <span className="font-display font-bold text-[12px] sm:text-[12.5px] text-[rgb(var(--foreground))]/90 truncate">
            {title}
          </span>
          {status}
        </div>
      </div>

      {/* Right: action / filter / sort / search, or the search input in place */}
      <div className="flex items-center gap-1 min-w-0 ml-auto shrink-0">
        {search?.open ? (
          <div className="flex h-7 items-center gap-1.5 w-[140px] sm:w-[200px] lg:w-[260px] min-w-0 border-b border-[rgb(var(--accent))] animate-fade-in">
            <Search size={12} className="text-[rgb(var(--accent))] shrink-0" />
            <input
              ref={searchInputRef}
              type="text"
              value={search.value}
              onChange={(e) => search.onChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.stopPropagation();
                  if (search.value) search.onChange("");
                  else search.onClose?.();
                } else if (e.key === "Enter") {
                  searchInputRef.current?.blur();
                }
              }}
              placeholder={search.placeholder}
              aria-label={search.placeholder}
              className="flex-1 min-w-0 bg-transparent text-[11px] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            <button
              type="button"
              onClick={search.onClose}
              className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 cursor-pointer"
              aria-label={CARD_HEADER_COPY.closeSearch}
            >
              <X size={12} />
            </button>
          </div>
        ) : (
          <>
            {action}

            {filter && (
              <div ref={filterRef} className="relative shrink-0">
                <Tooltip label={filter.label} side="bottom" asChild>
                  <button
                    type="button"
                    onClick={filter.onToggle}
                    aria-label={filter.label}
                    aria-expanded={filter.open}
                    className={cn(
                      ICON_BUTTON,
                      filter.open || filter.activeCount > 0
                        ? ICON_BUTTON_ACTIVE
                        : ICON_BUTTON_IDLE
                    )}
                  >
                    <Filter size={13} strokeWidth={1.75} />
                  </button>
                </Tooltip>

                {filter.open && (
                  <CardPopover
                    menu={filter.menu}
                    anchorRef={filterRef}
                    panelRef={popoverRef}
                    onClose={filter.onClose}
                  />
                )}
              </div>
            )}

            {sort && (
              <Tooltip
                label={sort.order === "az" ? CARD_HEADER_COPY.sortDesc : CARD_HEADER_COPY.sortAsc}
                side="bottom"
                asChild
              >
                <button
                  type="button"
                  onClick={sort.onToggle}
                  aria-label={sort.order === "az" ? CARD_HEADER_COPY.sortAsc : CARD_HEADER_COPY.sortDesc}
                  className={cn(ICON_BUTTON, ICON_BUTTON_IDLE)}
                >
                  {/* Icon inverts with the order: A→Z points down, Z→A points up. */}
                  {sort.order === "az" ? (
                    <ArrowDownAZ size={13} strokeWidth={1.75} />
                  ) : (
                    <ArrowUpAZ size={13} strokeWidth={1.75} />
                  )}
                </button>
              </Tooltip>
            )}

            {search && (
              <Tooltip
                label={searchActive ? CARD_HEADER_COPY.clearSearch : CARD_HEADER_COPY.search}
                side="bottom"
                asChild
              >
                <button
                  type="button"
                  onClick={search.onToggle}
                  aria-label={searchActive ? CARD_HEADER_COPY.clearSearch : CARD_HEADER_COPY.search}
                  className={cn(
                    ICON_BUTTON,
                    searchActive ? ICON_BUTTON_ACTIVE : ICON_BUTTON_IDLE
                  )}
                >
                  <Search size={13} strokeWidth={1.75} />
                </button>
              </Tooltip>
            )}
          </>
        )}
      </div>
    </div>
  );
});

CardHeader.displayName = "CardHeader";

/**
 * A popover rendered in a portal pinned to the anchor's bottom-right corner.
 * Anchored positioning would be clipped by a card's scroll container, so the
 * panel measures the anchor and clamps itself to the viewport instead.
 */
export const CardPopover = memo(function CardPopover({
  menu,
  anchorRef,
  panelRef,
  onClose,
}: {
  menu: React.ReactNode;
  anchorRef: React.RefObject<HTMLElement | null>;
  panelRef: React.RefObject<HTMLDivElement | null>;
  /** Dismissal callback: Escape and outside-press route through the overlay stack. */
  onClose: () => void;
}) {
  const [style, setStyle] = useState<CSSProperties | null>(null);

  // Registered on the global overlay stack: Escape and outside-press close the
  // popover with app-wide FILO semantics instead of per-surface listeners.
  useOverlay({
    onClose,
    ref: panelRef,
    dismissOnOutside: true,
    shouldDismissOnPointerDown: (target) =>
      !(anchorRef.current?.contains(target) ?? false) &&
      !(panelRef.current?.contains(target) ?? false),
  });

  useEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    let rafId: number | null = null;

    const place = () => {
      rafId = null;
      const el = anchorRef.current;
      if (!el) return;
      const r = el.getBoundingClientRect();
      const margin = 8;
      const width = Math.min(240, window.innerWidth - margin * 2);
      const top = Math.round(r.bottom + 6);
      const left = Math.round(
        Math.max(margin, Math.min(r.right - width, window.innerWidth - width - margin))
      );
      const maxHeight = Math.round(Math.max(160, window.innerHeight - r.bottom - 6 - margin));

      setStyle((prev) => {
        if (
          prev &&
          prev.top === top &&
          prev.left === left &&
          prev.width === width &&
          prev.maxHeight === maxHeight
        ) {
          return prev;
        }
        return { position: "fixed", top, left, width, maxHeight };
      });
    };

    const schedulePlace = (e?: Event) => {
      if (
        e &&
        panelRef.current &&
        e.target instanceof Node &&
        panelRef.current.contains(e.target)
      ) {
        return;
      }
      if (rafId !== null) return;
      rafId = requestAnimationFrame(place);
    };

    place();
    window.addEventListener("resize", schedulePlace, { passive: true });
    window.addEventListener("scroll", schedulePlace, { capture: true, passive: true });
    return () => {
      window.removeEventListener("resize", schedulePlace);
      window.removeEventListener("scroll", schedulePlace, true);
      if (rafId !== null) cancelAnimationFrame(rafId);
    };
  }, [anchorRef, panelRef]);

  if (!style) return null;

  return createPortal(
    <div
      ref={panelRef}
      style={style}
      className="z-[70] overflow-y-auto overscroll-contain rounded-lg bg-[rgba(var(--card),0.97)] border border-[rgba(var(--accent),0.18)] shadow-xl p-1.5 flex flex-col gap-1 animate-fade-in custom-scrollbar"
    >
      {menu}
    </div>,
    document.body
  );
});

CardPopover.displayName = "CardPopover";

export interface CardFilterMenuProps {
  title: string;
  /** Label for the reset control. Omit when no facet is active. */
  resetLabel?: string;
  onReset?: () => void;
  children: React.ReactNode;
}

/** Panel chrome for the filter popover: title row with an optional reset. */
export const CardFilterMenu = memo(function CardFilterMenu({
  title,
  resetLabel,
  onReset,
  children,
}: CardFilterMenuProps) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between px-1.5 pb-1">
        <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80">
          {title}
        </span>
        {resetLabel && onReset && (
          <button
            type="button"
            onClick={onReset}
            className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline cursor-pointer"
          >
            {resetLabel}
          </button>
        )}
      </div>
      {children}
    </div>
  );
});

CardFilterMenu.displayName = "CardFilterMenu";

export interface CardFilterItemProps {
  /** Optional facet glyph. Omitted for facets that need none. */
  icon?: LucideIcon;
  label: string;
  count: number;
  active: boolean;
  onSelect: () => void;
}

/** One facet row inside `CardFilterMenu`. */
export const CardFilterItem = memo(function CardFilterItem({
  icon: Icon,
  label,
  count,
  active,
  onSelect,
}: CardFilterItemProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "w-full flex items-center justify-between gap-2 px-2 py-1 rounded-md text-[11.5px] transition-colors cursor-pointer font-medium",
        active
          ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold"
          : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
      )}
    >
      <span className="flex items-center gap-1.5 truncate">
        {Icon && (
          <Icon
            size={12}
            strokeWidth={1.75}
            className={cn("shrink-0", active ? "text-[rgb(var(--accent))]" : "opacity-55")}
          />
        )}
        {/* Reserve the glyph slot so mixed rows stay aligned. */}
        <span className={cn("truncate", !Icon && "pl-[18px]")}>{label}</span>
      </span>
      <span
        className={cn(
          "text-[10px] font-mono px-1.5 rounded-md font-bold shrink-0 leading-[16px]",
          active
            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
            : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
        )}
      >
        {count}
      </span>
    </button>
  );
});

CardFilterItem.displayName = "CardFilterItem";
