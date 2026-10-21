import {
  memo,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { CATALOG_REVEAL_COPY } from "@/data/settingsCopy";

/**
 * The "there is more below" affordance, shared by every card catalog.
 *
 * A sentinel sits under the last row. While the sentinel is below the fold,
 * the list is clipped by a feather and a `Show all` label offers the full
 * catalog. The moment the end enters the visible band, the label disappears:
 * it has nothing left to offer.
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
  /** Track native scroll directly instead of waiting for IntersectionObserver. */
  syncOnScroll?: boolean;
}

export const CatalogReveal = memo(({
  children,
  onReveal,
  label = CATALOG_REVEAL_COPY.showAll,
  scrollerClassName,
  className,
  scrollRef: externalScrollRef,
  featherHeight = DEFAULT_FEATHER_HEIGHT,
  syncOnScroll = false,
}: CatalogRevealProps) => {
  const localScrollRef = useRef<HTMLDivElement | null>(null);
  const scrollRef = externalScrollRef ?? localScrollRef;
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  const [atEnd, setAtEnd] = useState(false);
  const { open } = useExpandableList();

  /**
   * The end state is measured, not waited for. An observer callback lands a
   * frame late, which showed up as the label appearing one or two frames
   * after a catalog opened. The layout effect measures the sentinel before the
   * first paint; the observer keeps up with scrolling and late content.
   */
  const band = Math.max(40, featherHeight - 8);
  const bandRef = useRef(band);
  bandRef.current = band;

  const measure = useCallback(() => {
    const root = scrollRef.current;
    const sentinel = sentinelRef.current;
    if (!root || !sentinel) return;
    const rootRect = root.getBoundingClientRect();
    const sentinelTop = sentinel.getBoundingClientRect().top;
    setAtEnd(
      sentinelTop >= rootRect.top + bandRef.current &&
      sentinelTop <= rootRect.bottom
    );
  }, [scrollRef]);

  useLayoutEffect(() => {
    const root = scrollRef.current;
    const sentinel = sentinelRef.current;
    if (!root || !sentinel) return undefined;

    measure();

    if (syncOnScroll) {
      root.addEventListener("scroll", measure, { passive: true });
      const resizeObserver = new ResizeObserver(measure);
      resizeObserver.observe(root);
      if (root.firstElementChild) resizeObserver.observe(root.firstElementChild);

      return () => {
        root.removeEventListener("scroll", measure);
        resizeObserver.disconnect();
      };
    }

    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[entries.length - 1];
        if (entry) setAtEnd(entry.isIntersecting);
      },
      // The band excludes the feather, so the label hides when the final row is
      // readable rather than when it is merely clipped.
      { root, rootMargin: `-${band}px 0px 0px 0px`, threshold: 0 }
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [measure, scrollRef, band, syncOnScroll]);

  return (
    <div className={cn("relative flex min-h-0 w-full flex-col", className)}>
      <div ref={scrollRef} className={cn("relative z-0", scrollerClassName)}>
        {children}
        <div ref={sentinelRef} aria-hidden="true" className="h-px w-full" />
      </div>

      {/* Feather: the last rows dissolve into the card instead of being cut. */}
      {!atEnd && (
        <div
          className="pointer-events-none absolute inset-x-0 bottom-0 z-10 bg-gradient-to-t from-[rgba(var(--card),0.97)] via-[rgba(var(--card),0.8)] to-transparent"
          style={{ height: featherHeight }}
        />
      )}

      {!atEnd && (
        <div className="pointer-events-none absolute inset-x-0 bottom-0 z-20 flex justify-center">
          <button
            type="button"
            onClick={onReveal ?? open}
            className="group pointer-events-auto inline-flex cursor-pointer flex-col items-center gap-0.5 text-[rgb(var(--accent))] transition-colors hover:text-[rgb(var(--foreground))]"
          >
            <span className="text-[11px] font-bold uppercase tracking-[0.14em]">{label}</span>
            <ChevronDown
              size={13}
              strokeWidth={2.5}
              className="transition-transform group-hover:translate-y-0.5"
            />
          </button>
        </div>
      )}
    </div>
  );
});

CatalogReveal.displayName = "CatalogReveal";
