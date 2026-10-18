import React, { memo, useCallback, useMemo, useRef } from "react";
import { cn } from "@/shared/lib/utils";

export interface CatalogFacetItem {
  id: string;
  label: string;
  count?: number;
  Icon?: React.ComponentType<{ size?: number; className?: string }>;
  disabled?: boolean;
}

export interface CatalogFacetGroup {
  id?: string;
  title?: string;
  items: CatalogFacetItem[];
}

export interface CatalogFacetRailProps {
  groups: CatalogFacetGroup[];
  selectedId: string;
  onSelect: (id: string) => void;
  footer?: React.ReactNode;
  className?: string;
}

export const CatalogFacetRail = memo(({
  groups,
  selectedId,
  onSelect,
  footer,
  className,
}: CatalogFacetRailProps) => {
  const containerRef = useRef<HTMLDivElement>(null);

  // Flatten all enabled items for sequential keyboard navigation
  const flatItems = useMemo(
    () => groups.flatMap((g) => g.items).filter((item) => !item.disabled),
    [groups]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (flatItems.length === 0) return;

      const currentIndex = flatItems.findIndex((item) => item.id === selectedId);

      if (e.key === "ArrowDown") {
        e.preventDefault();
        const nextIndex = currentIndex < flatItems.length - 1 ? currentIndex + 1 : 0;
        const target = flatItems[nextIndex];
        if (target) {
          onSelect(target.id);
          const el = containerRef.current?.querySelector<HTMLButtonElement>(`[data-facet-id="${target.id}"]`);
          el?.focus();
        }
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const prevIndex = currentIndex > 0 ? currentIndex - 1 : flatItems.length - 1;
        const target = flatItems[prevIndex];
        if (target) {
          onSelect(target.id);
          const el = containerRef.current?.querySelector<HTMLButtonElement>(`[data-facet-id="${target.id}"]`);
          el?.focus();
        }
      } else if (e.key === "Home") {
        e.preventDefault();
        const target = flatItems[0];
        if (target) {
          onSelect(target.id);
          const el = containerRef.current?.querySelector<HTMLButtonElement>(`[data-facet-id="${target.id}"]`);
          el?.focus();
        }
      } else if (e.key === "End") {
        e.preventDefault();
        const target = flatItems[flatItems.length - 1];
        if (target) {
          onSelect(target.id);
          const el = containerRef.current?.querySelector<HTMLButtonElement>(`[data-facet-id="${target.id}"]`);
          el?.focus();
        }
      }
    },
    [flatItems, selectedId, onSelect]
  );

  return (
    <div
      ref={containerRef}
      role="radiogroup"
      aria-label="Filter facets"
      onKeyDown={handleKeyDown}
      className={cn(
        "flex flex-col gap-3.5 w-full h-full overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5",
        className
      )}
    >
      {groups.map((group, groupIdx) => (
        <div
          key={group.id || group.title || groupIdx}
          className={cn(
            "flex flex-col gap-1",
            groupIdx > 0 && "pt-2 border-t border-[rgba(var(--foreground),0.06)]"
          )}
        >
          {group.title && (
            <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold select-none">
              {group.title}
            </div>
          )}

          {group.items.map((item) => {
            const isSelected = item.id === selectedId;
            const ItemIcon = item.Icon;

            return (
              <button
                key={item.id}
                type="button"
                role="radio"
                aria-checked={isSelected}
                tabIndex={isSelected ? 0 : -1}
                disabled={item.disabled}
                data-facet-id={item.id}
                onClick={() => onSelect(item.id)}
                className={cn(
                  "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]/50",
                  isSelected
                    ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]",
                  item.disabled && "opacity-40 cursor-not-allowed pointer-events-none"
                )}
              >
                <div className="flex items-center gap-2.5 truncate">
                  {ItemIcon && (
                    <ItemIcon
                      size={14}
                      className={cn(
                        "shrink-0",
                        isSelected ? "text-[rgb(var(--accent))]" : "opacity-60"
                      )}
                    />
                  )}
                  <span className="truncate">{item.label}</span>
                </div>

                {item.count !== undefined && (
                  <span
                    className={cn(
                      "text-[10px] font-mono px-1.5 py-0.5 rounded-md font-bold shrink-0 transition-colors",
                      isSelected
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                        : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
                    )}
                  >
                    {item.count}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      ))}

      {footer && (
        <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] mt-auto flex flex-col gap-1">
          {footer}
        </div>
      )}
    </div>
  );
});

CatalogFacetRail.displayName = "CatalogFacetRail";
