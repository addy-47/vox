import {
  memo,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { Maximize } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { CATALOG_REVEAL_COPY } from "@/data/settingsCopy";

/**
 * The catalog's expand affordance, shared by every card catalog.
 *
 * A sentinel sits under the last row. While the sentinel is below the fold,
 * the list is clipped by a feather and an `Expand` label sits on it. The list
 * is a native scroller that already holds every row, so the label does not
 * reveal rows: it opens the modal, which is the same catalog at full size.
 *
 * It renders even when the end is reached, because it is the only expand
 * affordance left (the corner maximize glyph is gone). In the modal it stands
 * down: the list is already full size.
 *
 * The reveal target defaults to the enclosing `ExpandableList`, so a list that
 * knows how to open its own modal needs no extra wiring.
 */
/** Default height of the feather band, in pixels. */
const DEFAULT_FEATHER_HEIGHT = 48;

export interface CatalogRevealProps {
  /** The scrolling list. Rendered inside the reveal container. */
  children: ReactNode;
  /** Opens the full catalog. Defaults to the enclosing expandable list. */
  onReveal?: () => void;
  /** Label under the chevron. */
  label?: string;
  /** Classes for the scrolling element. */
  scrollerClassName?: string;
  /** Classes for the reveal container. */
  className?: string;
  /**
   * The caller's own handle on the scroller. Catalogs that already own a
   * scroll ref (for a save-gated scroll to top) pass it in instead of
   * duplicating the element.
   */
  scrollRef?: RefObject<HTMLDivElement | null>;
  /**
   * Height of the feather band. The band grows with the catalog: a list of
   * tall rows needs more fade before the label can sit on it. The reveal band
   * follows, so the label only hides once the final row is readable.
   */
  featherHeight?: number;
}

export const CatalogReveal = memo(({
  children,
  onReveal,
  label = CATALOG_REVEAL_COPY.expand,
  scrollerClassName,
  className,
  scrollRef: externalScrollRef,
  featherHeight = DEFAULT_FEATHER_HEIGHT,
}: CatalogRevealProps) => {
  const localScrollRef = useRef<HTMLDivElement | null>(null);
  const scrollRef = externalScrollRef ?? localScrollRef;
  const [atEnd, setAtEnd] = useState(false);
  const [isScrollable, setIsScrollable] = useState(false);
  const { open, isExpanded } = useExpandableList();

  const measure = useCallback(() => {
    const root = scrollRef.current;
    if (!root) return;

    const sh = root.scrollHeight;
    const ch = root.clientHeight;
    const scrollable = sh > ch + 4;
    setIsScrollable((prev) => (prev === scrollable ? prev : scrollable));

    const reachedEnd = !scrollable || root.scrollTop + ch >= sh - 4;
    setAtEnd((prev) => (prev === reachedEnd ? prev : reachedEnd));
  }, [scrollRef]);

  useLayoutEffect(() => {
    const root = scrollRef.current;
    if (!root) return undefined;

    measure();

    root.addEventListener("scroll", measure, { passive: true });
    const resizeObserver = new ResizeObserver(measure);
    resizeObserver.observe(root);
    if (root.firstElementChild) resizeObserver.observe(root.firstElementChild);

    return () => {
      root.removeEventListener("scroll", measure);
      resizeObserver.disconnect();
    };
  }, [measure, scrollRef]);

  return (
    <div className={cn("relative flex min-h-0 w-full flex-col", className)}>
      <div ref={scrollRef} className={cn("relative z-0", scrollerClassName)}>
        {children}
      </div>

      {/* Feather: only shown when inline (stands down in modal) and content overflows.
          Fades out smoothly via CSS transition when reaching the bottom or content shrinks so the mask doesn't vanish abruptly. */}
      {!isExpanded && (
        <div
          className={cn(
            "pointer-events-none absolute inset-x-0 bottom-7 z-10 bg-gradient-to-t from-[rgba(var(--card),0.97)] via-[rgba(var(--card),0.8)] to-transparent transition-opacity duration-300 ease-out",
            isScrollable && !atEnd ? "opacity-100" : "opacity-0"
          )}
          style={{ height: featherHeight }}
        />
      )}

      {/* Expand button: stable in-flow position, eliminating layout oscillation */}
      {!isExpanded && (
        <div className="z-20 flex justify-center shrink-0 pt-1 pb-0.5">
          <button
            type="button"
            onClick={onReveal ?? open}
            aria-label={label}
            className="group pointer-events-auto inline-flex cursor-pointer items-center gap-1 text-[rgb(var(--accent))] transition-colors hover:text-[rgb(var(--foreground))]"
          >
            <span className="text-[11px] font-bold uppercase tracking-[0.14em]">{label}</span>
            <Maximize
              size={13}
              strokeWidth={2.2}
              className="transition-transform group-hover:-translate-y-px group-hover:translate-x-px"
            />
          </button>
        </div>
      )}
    </div>
  );
});

CatalogReveal.displayName = "CatalogReveal";
