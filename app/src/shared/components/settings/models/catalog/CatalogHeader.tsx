import { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CSSProperties } from "react";
import { ArrowLeft, ArrowUpDown, Filter, Search, X } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CATALOG_HEADER_COPY } from "@/data/settingsCopy";

/**
 * The standardized inline catalog header (design-spec §10.1).
 *
 * Grammar: `[icon] Title  ·  gap  ·  [action] [filter] [sort] [search]`.
 * Clicking search swaps the whole right cluster for an underline input in
 * place, so the header never changes height. Every icon slot carries the custom
 * `Tooltip` — native `title` is banned.
 */

export type CatalogSortOrder = "az" | "za";

export interface CatalogHeaderFilter {
  /** Tooltip + aria-label for the filter trigger. */
  label: string;
  open: boolean;
  /** Number of active facets; renders the accent dot. */
  activeCount: number;
  onToggle: () => void;
  onClose: () => void;
  /** Popover body. Rendered by `CatalogFilterMenu` in most call sites. */
  menu: React.ReactNode;
}

export interface CatalogHeaderSort {
  order: CatalogSortOrder;
  onToggle: () => void;
}

export interface CatalogHeaderSearch {
  open: boolean;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
  onToggle: () => void;
  onClose: () => void;
}

export interface CatalogHeaderProps {
  icon?: React.ReactNode;
  title: string;
  /** Breadcrumb back arrow for nested catalogs. */
  onBack?: () => void;
  /** Catalog-specific action slot (e.g. add-provider). Omitted when absent. */
  action?: React.ReactNode;
  /** Status affordance after the title (fetching, connection failure). */
  status?: React.ReactNode;
  filter?: CatalogHeaderFilter;
  sort: CatalogHeaderSort;
  search: CatalogHeaderSearch;
}

const ICON_BUTTON =
  "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]";

const ICON_BUTTON_IDLE =
  "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]";

const ICON_BUTTON_ACTIVE = "text-[rgb(var(--accent))]";

export const CatalogHeader = memo(function CatalogHeader({
  icon: _icon,
  title,
  onBack,
  action,
  status,
  filter,
  sort,
  search,
}: CatalogHeaderProps) {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  // Anchors the filter popover: the trigger wrapper, measured on open.
  const filterRef = useRef<HTMLDivElement | null>(null);
  // The popover is portaled to <body>, so outside-click must test it too.
  const popoverRef = useRef<HTMLDivElement | null>(null);

  // The search input replaces the icon cluster, so it must take focus itself.
  useEffect(() => {
    if (search.open) searchInputRef.current?.focus();
  }, [search.open]);

  // Latest filter callbacks without re-binding the listeners every render.
  const onFilterCloseRef = useRef<(() => void) | null>(null);
  onFilterCloseRef.current = filter?.onClose ?? null;

  // Outside pointerdown and Escape close the filter popover (Tier 1 popover).
  const filterOpen = filter?.open ?? false;
  useEffect(() => {
    if (!filterOpen) return;
    const handleDown = (e: PointerEvent) => {
      const target = e.target as Node;
      const inTrigger = filterRef.current?.contains(target) ?? false;
      const inPopover = popoverRef.current?.contains(target) ?? false;
      if (!inTrigger && !inPopover) onFilterCloseRef.current?.();
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onFilterCloseRef.current?.();
    };
    document.addEventListener("pointerdown", handleDown);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("pointerdown", handleDown);
      document.removeEventListener("keydown", handleKey);
    };
  }, [filterOpen]);

  const searchActive = search.open || search.value.length > 0;

  return (
    <div className="relative flex items-center justify-between gap-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] min-h-[34px]">
      {/* Left: onBack + Search (compact, never shifts center title) */}
      <div className="flex items-center gap-1 shrink-0 z-10 min-w-0">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            className="inline-flex items-center gap-1 text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0 group mr-0.5"
            aria-label={CATALOG_HEADER_COPY.back}
          >
            <ArrowLeft
              size={12}
              strokeWidth={2.5}
              className="group-hover:-translate-x-0.5 transition-transform"
            />
          </button>
        )}
        {search.open ? (
          <div className="flex items-center gap-1 w-[95px] sm:w-[110px] border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in">
            <Search size={11} className="text-[rgb(var(--accent))] shrink-0" />
            <input
              ref={searchInputRef}
              type="text"
              value={search.value}
              onChange={(e) => search.onChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  if (search.value) search.onChange("");
                  else search.onClose();
                } else if (e.key === "Enter") {
                  searchInputRef.current?.blur();
                }
              }}
              placeholder={search.placeholder}
              aria-label={search.placeholder}
              className="w-full min-w-0 bg-transparent text-[11px] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            <button
              type="button"
              onClick={search.onClose}
              className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 cursor-pointer"
              aria-label={CATALOG_HEADER_COPY.closeSearch}
            >
              <X size={11} />
            </button>
          </div>
        ) : (
          <Tooltip
            label={searchActive ? CATALOG_HEADER_COPY.clearSearch : CATALOG_HEADER_COPY.search}
            side="bottom"
            asChild
          >
            <button
              type="button"
              onClick={search.onToggle}
              aria-label={searchActive ? CATALOG_HEADER_COPY.clearSearch : CATALOG_HEADER_COPY.search}
              className={cn(
                ICON_BUTTON,
                searchActive ? ICON_BUTTON_ACTIVE : ICON_BUTTON_IDLE
              )}
            >
              <Search size={13} strokeWidth={1.75} />
            </button>
          </Tooltip>
        )}
      </div>

      {/* Center: title + status, anchored to center so it never moves when search expands */}
      <div className="absolute left-1/2 -translate-x-1/2 flex items-center justify-center gap-1.5 max-w-[50%] pointer-events-none text-center">
        <span className="font-display font-bold text-[12.5px] sm:text-[13px] text-[rgb(var(--foreground))]/90 truncate">
          {title}
        </span>
        {status && <div className="pointer-events-auto shrink-0">{status}</div>}
      </div>

      {/* Right: action / sort / filter */}
      <div className="flex items-center gap-1 shrink-0 ml-auto z-10">
        {action}

        <Tooltip
          label={sort.order === "az" ? CATALOG_HEADER_COPY.sortDesc : CATALOG_HEADER_COPY.sortAsc}
          side="bottom"
          asChild
        >
          <button
            type="button"
            onClick={sort.onToggle}
            aria-label={sort.order === "az" ? CATALOG_HEADER_COPY.sortAsc : CATALOG_HEADER_COPY.sortDesc}
            className={cn(ICON_BUTTON, ICON_BUTTON_IDLE)}
          >
            <ArrowUpDown size={13} strokeWidth={1.75} />
          </button>
        </Tooltip>

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
              <CatalogFilterPopover
                menu={filter.menu}
                anchorRef={filterRef}
                panelRef={popoverRef}
              />
            )}
          </div>
        )}
      </div>
    </div>
  );
});

CatalogHeader.displayName = "CatalogHeader";

/**
 * The filter popover renders in a portal pinned to the trigger's bottom-right
 * corner. Anchored positioning would be clipped by the card's scroll container,
 * so the panel measures the trigger and clamps itself to the viewport instead.
 */
const CatalogFilterPopover = memo(function CatalogFilterPopover({
  menu,
  anchorRef,
  panelRef,
}: {
  menu: React.ReactNode;
  anchorRef: React.RefObject<HTMLDivElement | null>;
  panelRef: React.RefObject<HTMLDivElement | null>;
}) {
  const [style, setStyle] = useState<CSSProperties | null>(null);

  useEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const place = () => {
      const r = anchor.getBoundingClientRect();
      const margin = 8;
      const width = Math.min(240, window.innerWidth - margin * 2);
      setStyle({
        position: "fixed",
        top: r.bottom + 6,
        left: Math.max(margin, Math.min(r.right - width, window.innerWidth - width - margin)),
        width,
        maxHeight: Math.max(160, window.innerHeight - r.bottom - 6 - margin),
      });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [anchorRef]);

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

CatalogFilterPopover.displayName = "CatalogFilterPopover";

export interface CatalogFilterMenuProps {
  title: string;
  /** Label for the reset control. Omit when no facet is active. */
  resetLabel?: string;
  onReset?: () => void;
  children: React.ReactNode;
}

/** Panel chrome for the filter popover: title row with an optional reset. */
export const CatalogFilterMenu = memo(function CatalogFilterMenu({
  title,
  resetLabel,
  onReset,
  children,
}: CatalogFilterMenuProps) {
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

CatalogFilterMenu.displayName = "CatalogFilterMenu";

export interface CatalogFilterItemProps {
  /** Optional facet glyph. Omitted for facets that need none. */
  icon?: LucideIcon;
  label: string;
  count: number;
  active: boolean;
  onSelect: () => void;
}

/** One facet row inside `CatalogFilterMenu`. */
export const CatalogFilterItem = memo(function CatalogFilterItem({
  icon: Icon,
  label,
  count,
  active,
  onSelect,
}: CatalogFilterItemProps) {
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
          "text-[10px] font-mono px-1.5 rounded-full font-bold shrink-0 leading-[16px]",
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

CatalogFilterItem.displayName = "CatalogFilterItem";