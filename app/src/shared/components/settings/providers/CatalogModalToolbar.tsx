import { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CSSProperties } from "react";
import { ArrowDownAZ, ArrowUpZA, Filter, Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CATALOG_HEADER_COPY } from "@/data/settingsCopy";

export interface CatalogModalToolbarFilter {
  label: string;
  open: boolean;
  activeCount: number;
  onToggle: () => void;
  onClose: () => void;
  menu: React.ReactNode;
}

export interface CatalogModalToolbarProps {
  search: {
    value: string;
    placeholder: string;
    onChange: (val: string) => void;
  };
  sort: {
    order: "az" | "za";
    onToggle: () => void;
  };
  itemCount: {
    current: number;
    total: number;
    entityName?: string;
  };
  filter?: CatalogModalToolbarFilter;
  action?: React.ReactNode;
  layoutMode?: "full-max" | "full-min" | "small";
}

/**
 * Portaled filter popover anchored below the filter trigger.
 * Clamps to viewport margins to avoid modal scroll clipping.
 */
const CatalogModalFilterPopover = memo(function CatalogModalFilterPopover({
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
      const width = Math.min(260, window.innerWidth - margin * 2);
      const left = Math.max(margin, Math.min(r.left, window.innerWidth - width - margin));
      setStyle({
        position: "fixed",
        top: r.bottom + 6,
        left,
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
      className="z-[80] overflow-y-auto overscroll-contain rounded-xl bg-[rgba(var(--card),0.98)] border border-[rgba(var(--accent),0.2)] shadow-2xl p-2 flex flex-col gap-1.5 animate-fade-in custom-scrollbar backdrop-blur-md"
    >
      {menu}
    </div>,
    document.body
  );
});

CatalogModalFilterPopover.displayName = "CatalogModalFilterPopover";

export const CatalogModalToolbar = memo(function CatalogModalToolbar({
  search,
  sort,
  itemCount,
  filter,
  action,
  layoutMode,
}: CatalogModalToolbarProps) {
  const filterRef = useRef<HTMLDivElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);

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

  const isSmall = layoutMode === "small";

  return (
    <div className="flex items-center gap-2 w-full shrink-0 pb-2 border-b border-[rgba(var(--accent),0.08)]">
      {/* ── Left Filter Trigger (Stage 3): Surfaced on mobile or small layout ── */}
      {filter && (
        <div
          ref={filterRef}
          className={cn("relative shrink-0", isSmall ? "flex" : "flex lg:hidden")}
        >
          <Tooltip label={filter.label} side="bottom" asChild>
            <button
              type="button"
              onClick={filter.onToggle}
              aria-label={filter.label}
              aria-expanded={filter.open}
              className={cn(
                "p-2 rounded-xl border transition-all cursor-pointer flex items-center justify-center gap-1.5 shrink-0",
                filter.open || filter.activeCount > 0
                  ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.35)] shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <Filter size={14} strokeWidth={1.8} />
              {filter.activeCount > 0 && (
                <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" />
              )}
            </button>
          </Tooltip>

          {filter.open && (
            <CatalogModalFilterPopover
              menu={filter.menu}
              anchorRef={filterRef}
              panelRef={popoverRef}
            />
          )}
        </div>
      )}

      {/* ── Expanded Search Input (Stages 2 & 3): Always visible and open ── */}
      <div className="relative flex items-center flex-1 min-w-[120px] rounded-xl border border-[rgba(var(--foreground),0.10)] bg-[rgba(var(--foreground),0.02)] px-3 py-1.5 focus-within:border-[rgb(var(--accent))]/60 focus-within:bg-[rgba(var(--accent),0.02)] transition-colors">
        <Search size={13} className="text-[rgb(var(--foreground-muted))]/60 shrink-0 mr-2" />
        <input
          type="text"
          value={search.value}
          onChange={(e) => search.onChange(e.target.value)}
          placeholder={search.placeholder}
          aria-label={search.placeholder}
          className="w-full bg-transparent text-[11.5px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
        />
        {search.value && (
          <button
            type="button"
            onClick={() => search.onChange("")}
            aria-label={CATALOG_HEADER_COPY.clearSearch}
            className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 cursor-pointer ml-1"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* ── Sort Toggle Button (A-Z / Z-A) ── */}
      <Tooltip
        label={sort.order === "az" ? CATALOG_HEADER_COPY.sortDesc : CATALOG_HEADER_COPY.sortAsc}
        side="bottom"
        asChild
      >
        <button
          type="button"
          onClick={sort.onToggle}
          aria-label={sort.order === "az" ? CATALOG_HEADER_COPY.sortAsc : CATALOG_HEADER_COPY.sortDesc}
          className="p-2 rounded-xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.2)] transition-colors cursor-pointer shrink-0 flex items-center justify-center"
        >
          {sort.order === "az" ? (
            <ArrowDownAZ size={14} strokeWidth={1.8} />
          ) : (
            <ArrowUpZA size={14} strokeWidth={1.8} />
          )}
        </button>
      </Tooltip>

      {/* ── Item Count Badge: Desktop full text, Mobile compact X/Y ── */}
      <div className="shrink-0 select-none">
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] hidden sm:inline">
          Showing <span className="text-[rgb(var(--foreground))] font-bold">{itemCount.current}</span> of {itemCount.total}{itemCount.entityName ? ` ${itemCount.entityName}` : ""}
        </span>
        <span className="text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] px-2 py-1 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] inline sm:hidden">
          <span className="text-[rgb(var(--foreground))] font-bold">{itemCount.current}</span>/{itemCount.total}
        </span>
      </div>

      {/* ── Action Slot: Far right action button (e.g. Clone Voice, Guide) ── */}
      {action && <div className="shrink-0 flex items-center">{action}</div>}
    </div>
  );
});

CatalogModalToolbar.displayName = "CatalogModalToolbar";
