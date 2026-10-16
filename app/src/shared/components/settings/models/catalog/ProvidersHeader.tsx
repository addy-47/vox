import { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CSSProperties } from "react";
import { ArrowLeft, ArrowUpDown, Check, Filter, Plus, Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { PROVIDERS_HEADER_COPY } from "@/data/settingsCopy";

export type ProviderSortOrder = "az" | "za";

export interface ProvidersHeaderFilter {
  label: string;
  open: boolean;
  activeCount: number;
  onToggle: () => void;
  onClose: () => void;
  menu: React.ReactNode;
}

export interface ProvidersHeaderSort {
  order: ProviderSortOrder;
  onToggle: () => void;
}

export interface ProvidersHeaderSearch {
  open: boolean;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
  onToggle: () => void;
  onClose: () => void;
}

export interface InlineCustomActions {
  onSave: () => void;
  onCancel: () => void;
  isSaving?: boolean;
}

export interface ProvidersHeaderProps {
  icon?: React.ReactNode;
  title: string;
  onBack?: () => void;
  onCustomClick?: () => void;
  isCustomActive?: boolean;
  isCustomDisabled?: boolean;
  customTooltip?: string;
  inlineCustomActions?: InlineCustomActions | null;
  status?: React.ReactNode;
  filter?: ProvidersHeaderFilter;
  sort?: ProvidersHeaderSort;
  search?: ProvidersHeaderSearch;
}

const ICON_BUTTON =
  "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]";

const ICON_BUTTON_IDLE =
  "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]";

const ICON_BUTTON_ACTIVE = "text-[rgb(var(--accent))]";

export const ProvidersHeader = memo(function ProvidersHeader({
  icon,
  title,
  onBack,
  onCustomClick,
  isCustomActive = false,
  isCustomDisabled = false,
  customTooltip,
  inlineCustomActions,
  status,
  filter,
  sort,
  search,
}: ProvidersHeaderProps) {
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const filterRef = useRef<HTMLDivElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (search?.open) searchInputRef.current?.focus();
  }, [search?.open]);

  const onFilterCloseRef = useRef<(() => void) | null>(null);
  onFilterCloseRef.current = filter?.onClose ?? null;

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

  // Measure filter trigger for fixed portal positioning
  const [popoverStyle, setPopoverStyle] = useState<CSSProperties>({});
  useEffect(() => {
    if (!filterOpen) return;
    const trigger = filterRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const width = 192;
    const padding = 8;
    let left = rect.right - width;
    if (left < padding) left = padding;
    if (left + width > window.innerWidth - padding) {
      left = window.innerWidth - width - padding;
    }
    const top = rect.bottom + 4;
    setPopoverStyle({
      position: "fixed",
      top: `${Math.round(top)}px`,
      left: `${Math.round(left)}px`,
      width: `${width}px`,
      zIndex: 9999,
    });
  }, [filterOpen]);

  const searchActive = !!search && (search!.open || search!.value.length > 0);
  const filterActive = (filter?.activeCount ?? 0) > 0;

  return (
    <div className="relative flex items-center justify-between gap-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] min-h-[36px] pb-1.5 select-none">
      {/* ─── LEFT: Breadcrumb + Icon + Title + Status ─── */}
      <div className="flex items-center gap-1.5 shrink-0 z-10 min-w-0">
        {onBack && (
          <Tooltip label={PROVIDERS_HEADER_COPY.back} side="bottom" asChild>
            <button
              type="button"
              onClick={onBack}
              className="inline-flex items-center gap-1 text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0 group mr-0.5"
              aria-label={PROVIDERS_HEADER_COPY.back}
            >
              <ArrowLeft
                size={12}
                strokeWidth={2.5}
                className="group-hover:-translate-x-0.5 transition-transform"
              />
            </button>
          </Tooltip>
        )}

        {icon && <span className="shrink-0 text-[rgb(var(--accent))] flex items-center">{icon}</span>}

        <span className="font-display font-bold text-[12.5px] sm:text-[13px] text-[rgb(var(--foreground))]/90 truncate">
          {title}
        </span>

        {status && <div className="ml-1 shrink-0">{status}</div>}
      </div>

      {/* ─── MIDDLE: Spacer ─── */}
      <div className="flex-1 min-w-0" />

      {/* ─── RIGHT: + Custom, Filter, Sort, Search ─── */}
      <div className="flex items-center gap-1.5 shrink-0 z-10">
        {/* 1. Custom / Inline Actions */}
        {inlineCustomActions ? (
          <div className="flex items-center gap-1">
            <Tooltip label="Save custom provider" side="bottom">
              <button
                type="button"
                onClick={inlineCustomActions.onSave}
                disabled={inlineCustomActions.isSaving}
                aria-label="Save custom provider"
                className={cn(
                  "h-6 w-6 flex items-center justify-center rounded-md border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-all cursor-pointer",
                  inlineCustomActions.isSaving && "opacity-60 cursor-wait"
                )}
              >
                <Check size={13} strokeWidth={2.2} />
              </button>
            </Tooltip>
            <Tooltip label="Discard changes" side="bottom">
              <button
                type="button"
                onClick={inlineCustomActions.onCancel}
                disabled={inlineCustomActions.isSaving}
                className="h-6 px-1.5 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] transition-colors cursor-pointer"
              >
                <X size={12} strokeWidth={2.2} />
              </button>
            </Tooltip>
          </div>
        ) : isCustomDisabled ? (
          <Tooltip label={customTooltip || PROVIDERS_HEADER_COPY.comingSoon} side="bottom">
            <span className="inline-flex">
              <button
                type="button"
                disabled
                aria-disabled
                className="h-6 px-1.5 sm:px-2 flex items-center gap-1 rounded-md text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/40 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] cursor-not-allowed select-none"
              >
                <Plus size={11} strokeWidth={2.2} />
                <span className="hidden sm:inline">Custom</span>
              </button>
            </span>
          </Tooltip>
        ) : (
          <Tooltip
            label={customTooltip || PROVIDERS_HEADER_COPY.customAria}
            side="bottom"
            asChild
          >
            <button
              type="button"
              onClick={onCustomClick}
              aria-label={customTooltip || PROVIDERS_HEADER_COPY.customAria}
              className={cn(
                "h-6 px-1.5 sm:px-2 flex items-center gap-1 rounded-md text-[10.5px] font-bold transition-all cursor-pointer",
                isCustomActive
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_0_8px_rgba(var(--accent),0.35)]"
                  : "bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.16)] border border-[rgba(var(--accent),0.25)]"
              )}
            >
              <Plus size={11} strokeWidth={2.2} />
              <span className="hidden sm:inline">Custom</span>
            </button>
          </Tooltip>
        )}

        {/* 2. Filter */}
        {filter && (
          <div ref={filterRef} className="relative inline-flex">
            <Tooltip label={filter.label} side="bottom" asChild>
              <button
                type="button"
                onClick={filter.onToggle}
                aria-label={filter.label}
                aria-expanded={filter.open}
                className={cn(
                  ICON_BUTTON,
                  filterOpen || filterActive ? ICON_BUTTON_ACTIVE : ICON_BUTTON_IDLE,
                  "relative"
                )}
              >
                <Filter size={13} strokeWidth={1.75} />
                {filterActive && (
                  <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_4px_rgba(var(--accent),0.8)]" />
                )}
              </button>
            </Tooltip>

            {filterOpen &&
              createPortal(
                <div
                  ref={popoverRef}
                  style={popoverStyle}
                  className="rounded-xl bg-[rgba(var(--card),0.96)] border border-[rgba(var(--foreground),0.12)] shadow-2xl p-2.5 flex flex-col gap-1 animate-fade-in "
                >
                  {filter.menu}
                </div>,
                document.body
              )}
          </div>
        )}

{/* 3. Sort */}
        {sort && (
        <Tooltip
          label={sort!.order === "az" ? PROVIDERS_HEADER_COPY.sortDesc : PROVIDERS_HEADER_COPY.sortAsc}
          side="bottom"
          asChild
        >
          <button
            type="button"
            onClick={sort!.onToggle}
            aria-label={sort!.order === "az" ? PROVIDERS_HEADER_COPY.sortAsc : PROVIDERS_HEADER_COPY.sortDesc}
            className={cn(ICON_BUTTON, ICON_BUTTON_IDLE)}
          >
            <ArrowUpDown size={13} strokeWidth={1.75} />
          </button>
        </Tooltip>
        )}
        {search && (search!.open ? (
          <div className="flex items-center gap-1 w-[95px] sm:w-[115px] border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in">
            <Search size={11} className="text-[rgb(var(--accent))] shrink-0" />
            <input
              ref={searchInputRef}
              type="text"
              value={search!.value}
              onChange={(e) => search!.onChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  if (search!.value) search!.onChange("");
                  else search!.onClose();
                } else if (e.key === "Enter") {
                  searchInputRef.current?.blur();
                }
              }}
              placeholder={search!.placeholder || PROVIDERS_HEADER_COPY.searchPlaceholder}
              aria-label={search!.placeholder || PROVIDERS_HEADER_COPY.searchPlaceholder}
              className="w-full min-w-0 bg-transparent text-[11px] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            <button
              type="button"
              onClick={search!.onClose}
              className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 cursor-pointer"
              aria-label={PROVIDERS_HEADER_COPY.closeSearch}
            >
              <X size={11} />
            </button>
          </div>
        ) : (
          <Tooltip
            label={searchActive ? PROVIDERS_HEADER_COPY.clearSearch : PROVIDERS_HEADER_COPY.search}
            side="bottom"
            asChild
          >
            <button
              type="button"
              onClick={search!.onToggle}
              aria-label={searchActive ? PROVIDERS_HEADER_COPY.clearSearch : PROVIDERS_HEADER_COPY.search}
              className={cn(
                ICON_BUTTON,
                searchActive ? ICON_BUTTON_ACTIVE : ICON_BUTTON_IDLE
              )}
            >
              <Search size={13} strokeWidth={1.75} />
            </button>
          </Tooltip>
        )
        )}
      </div>
    </div>
  );
});

ProvidersHeader.displayName = "ProvidersHeader";
