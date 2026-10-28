"use client";

import * as React from "react";
import {
  motion,
  useMotionValue,
  useReducedMotion,
  useSpring,
  useTransform,
  type MotionValue,
} from "framer-motion";
import { ChevronDown, ChevronUp } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "./Tooltip";

export interface Chapter {
  /** Stable, unique identifier for the chapter. */
  id: string;
  /** Bold heading shown at the top of the preview card. */
  title: string;
  /** Supporting copy shown under the title (clamped to three lines). */
  description?: React.ReactNode;
  /** Small muted label rendered above the title (e.g. a timestamp or step no.). */
  meta?: React.ReactNode;
}

export interface ChapterScrubberProps {
  /** Chapters rendered top-to-bottom, one uniform tick each. */
  chapters: Chapter[];
  /** Which side the preview card opens toward. Auto-flips near a viewport edge. Default `"right"`. */
  side?: "left" | "right";
  /** Length a tick reaches at the crest of the magnification, in pixels. Default `56`. */
  peakLength?: number;
  /** Resting length of every tick, in pixels. Keep it small. Default `14`. */
  restLength?: number;
  /** Height of each row in pixels; the gap between ticks. Smaller = denser. Default `10`. */
  rowHeight?: number;
  /** Radius of the magnification wave, in rows — how far the rise reaches from the pointer. Default `4`. */
  radius?: number;
  /** Marks one chapter as the persistent "current" position (e.g. where an agent is now). */
  currentIndex?: number;
  /**
   * Maximum rows rendered at once. When `chapters` exceeds this, the rail pages
   * through history with chevrons instead of shrinking the ticks. Default: all.
   */
  maxVisible?: number;
  /** Fires when the active (hovered/focused) chapter changes. */
  onActiveChange?: (chapter: Chapter | null, index: number) => void;
  /** Fires when a chapter is chosen via click, Enter or Space. */
  onSelect?: (chapter: Chapter, index: number) => void;
  /** Accessible name for the rail. Default `"Chapters"`. */
  label?: string;
  className?: string;
}

const CARD_WIDTH = 240;
const GAP = 20;
const CHEVRON_SIZE = 28;
// Tight, near-critically-damped spring: tracks the cursor with almost no lag
// and never overshoots — the wave feels attached to the pointer.
const POINTER_SPRING = { stiffness: 700, damping: 52, mass: 0.5 };
// Softer spring for the rise/settle so the wave swells and relaxes gracefully.
const STRENGTH_SPRING = { stiffness: 260, damping: 30, mass: 0.6 };

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

// Raised-cosine bump: 1 at the crest, 0 beyond the radius, with zero slope at
// both ends so the wave has no seams — the source of the buttery falloff.
function bump(distance: number, radius: number) {
  if (distance >= radius) return 0;
  return 0.5 * (1 + Math.cos(Math.PI * (distance / radius)));
}

interface TickProps {
  index: number;
  pointer: MotionValue<number>;
  strength: MotionValue<number>;
  radius: number;
  restLength: number;
  peakLength: number;
  isCurrent: boolean;
}

const Tick = React.memo(function Tick({
  index,
  pointer,
  strength,
  radius,
  restLength,
  peakLength,
  isCurrent,
}: TickProps) {
  const width = useTransform(() => {
    const rise = strength.get() * bump(Math.abs(index - pointer.get()), radius);
    return restLength + rise * (peakLength - restLength);
  });
  const opacity = useTransform(() => {
    const rise = strength.get() * bump(Math.abs(index - pointer.get()), radius);
    const base = isCurrent ? 0.9 : 0.45;
    return base + rise * (1 - base);
  });
  const scaleY = useTransform(() => {
    const rise = strength.get() * bump(Math.abs(index - pointer.get()), radius);
    // Slight thickening at the crest (2px -> ~2.8px); the length change
    // carries the rise, thickness is a quiet secondary cue.
    return 1 + rise * 0.4;
  });

  return (
    <motion.span
      aria-hidden="true"
      style={{ width, opacity, scaleY }}
      className={cn(
        "block h-[2px] rounded-full",
        isCurrent
          ? "bg-[rgb(var(--accent))]"
          : "bg-[rgb(var(--foreground))]",
      )}
    />
  );
});

export function ChapterScrubber({
  chapters,
  side = "right",
  peakLength = 56,
  restLength = 14,
  rowHeight = 10,
  radius = 4,
  currentIndex,
  maxVisible: maxVisibleProp,
  onActiveChange,
  onSelect,
  label = "Chapters",
  className,
}: ChapterScrubberProps) {
  const prefersReducedMotion = useReducedMotion();
  const containerRef = React.useRef<HTMLDivElement>(null);
  const listRef = React.useRef<HTMLDivElement>(null);
  const cardRef = React.useRef<HTMLDivElement>(null);
  const buttonsRef = React.useRef<Array<HTMLButtonElement | null>>([]);
  // Namespaced so option ids stay unique across instances and don't depend on
  // chapter.id being a valid, collision-free DOM id.
  const baseId = React.useId();
  const optionId = (index: number) => `${baseId}-opt-${index}`;

  const rawPointer = useMotionValue(0);
  const rawStrength = useMotionValue(0);
  const springPointer = useSpring(rawPointer, POINTER_SPRING);
  const springStrength = useSpring(rawStrength, STRENGTH_SPRING);
  // Reduced motion: drop the temporal easing but keep the spatial wave, so the
  // rise is instant rather than sprung.
  const pointer = prefersReducedMotion ? rawPointer : springPointer;
  const strength = prefersReducedMotion ? rawStrength : springStrength;

  const [activeIndex, setActiveIndex] = React.useState(0);
  const [engaged, setEngaged] = React.useState(false);
  const [flipped, setFlipped] = React.useState(false);
  const [cardHeight, setCardHeight] = React.useState(0);
  // Pages scrolled back from the newest turn. A zero value pins the rail to the
  // end, which is the default and the state a freshly spoken turn restores.
  const [pageBack, setPageBack] = React.useState(0);
  const [prevTotal, setPrevTotal] = React.useState(chapters.length);
  const hoveringRef = React.useRef(false);
  const focusedRef = React.useRef<number | null>(null);
  const activeRef = React.useRef(0);

  // Render-phase adjustment (no effect, no extra commit): any change in turn
  // count — a new spoken turn or a session switch — snaps back to the newest
  // page so the latest turn is the highlighted one.
  if (chapters.length !== prevTotal) {
    setPrevTotal(chapters.length);
    if (pageBack !== 0) setPageBack(0);
  }

  const total = chapters.length;
  const maxVisible = Math.max(
    1,
    Math.min(maxVisibleProp ?? total, total) || 1,
  );
  const newestStart = Math.max(0, total - maxVisible);
  const windowStart = Math.max(0, newestStart - pageBack * maxVisible);
  const visibleChapters =
    total > 0 ? chapters.slice(windowStart, windowStart + maxVisible) : [];
  const hasPages = total > maxVisible;
  const canPageOlder = windowStart > 0;
  const canPageNewer = windowStart < newestStart;
  const last = visibleChapters.length - 1;

  const commitActive = React.useCallback((index: number) => {
    if (index !== activeRef.current) {
      activeRef.current = index;
      setActiveIndex(index);
    }
  }, []);

  React.useEffect(() => {
    const globalIndex = windowStart + Math.min(activeIndex, Math.max(0, last));
    onActiveChange?.(
      engaged ? chapters[globalIndex] : null,
      engaged ? globalIndex : -1,
    );
  }, [engaged, activeIndex, chapters, windowStart, last, onActiveChange]);

  // Measure the card so its vertical travel can be clamped to the rail.
  React.useEffect(() => {
    if (cardRef.current) setCardHeight(cardRef.current.offsetHeight);
  }, [activeIndex]);

  // Flip toward the roomier side if the card would spill past the viewport.
  React.useEffect(() => {
    if (!engaged) return;
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const vw = el.ownerDocument.defaultView?.innerWidth ?? 0;
    const need = CARD_WIDTH + GAP + 8;
    let useRight = side === "right";
    if (useRight && vw - rect.right < need && rect.left >= need)
      useRight = false;
    if (!useRight && rect.left < need && vw - rect.right >= need)
      useRight = true;
    setFlipped(useRight !== (side === "right"));
  }, [engaged, activeIndex, side]);

  const resolvedSide =
    side === "right"
      ? flipped
        ? "left"
        : "right"
      : flipped
        ? "right"
        : "left";

  const totalHeight = visibleChapters.length * rowHeight;
  const listOffset = hasPages ? CHEVRON_SIZE : 0;
  // Exactly one tick is tabbable at a time (roving tabindex). When the current
  // turn lives on a newer page, fall back to the last visible tick.
  const currentVisibleIndex =
    currentIndex === undefined ? 0 : currentIndex - windowStart;
  const rovingIndex = engaged
    ? activeIndex
    : currentVisibleIndex >= 0 && currentVisibleIndex <= last
      ? currentVisibleIndex
      : Math.max(0, last);

  const cardTop = useTransform(pointer, (p) => {
    const half = cardHeight / 2;
    const center = clamp(
      (p + 0.5) * rowHeight,
      half,
      Math.max(half, totalHeight - half),
    );
    return center - half + listOffset;
  });
  const cardScale = useTransform(strength, [0, 1], [0.97, 1]);
  const cardX = useTransform(
    strength,
    [0, 1],
    [resolvedSide === "right" ? -6 : 6, 0],
  );

  const engageAt = (pointerRow: number, activeAt: number) => {
    rawPointer.set(pointerRow);
    rawStrength.set(1);
    commitActive(clamp(activeAt, 0, last));
    if (!engaged) setEngaged(true);
  };

  // Chevrons move the window one full page and take over the preview card, so
  // the newly revealed chapter becomes the engaged (and highlighted) one. The
  // card then springs to that row through the same pointer -> card pipeline.
  const pageOlder = () => {
    const nextPageBack = pageBack + 1;
    const nextWindowStart = Math.max(0, newestStart - nextPageBack * maxVisible);
    setPageBack(nextPageBack);
    const anchor = nextWindowStart;
    rawPointer.set(anchor + 0.5);
    rawStrength.set(1);
    commitActive(Math.min(anchor, Math.max(0, last)));
    setEngaged(true);
  };

  const pageNewer = () => {
    const nextPageBack = Math.max(0, pageBack - 1);
    const nextWindowStart = Math.max(0, newestStart - nextPageBack * maxVisible);
    setPageBack(nextPageBack);
    const anchor = Math.max(0, Math.min(maxVisible - 1, nextWindowStart + maxVisible - 1));
    rawPointer.set(anchor + 0.5);
    rawStrength.set(1);
    commitActive(Math.min(anchor, Math.max(0, last)));
    setEngaged(true);
  };

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    // When touched on mobile, immediately engage and track the touch position
    const el = listRef.current;
    if (!el) return;
    hoveringRef.current = true;
    try {
      el.setPointerCapture(event.pointerId);
    } catch {
      // Best-effort pointer capture
    }
    const rect = el.getBoundingClientRect();
    const row = (event.clientY - rect.top) / rowHeight - 0.5;
    engageAt(clamp(row, -0.5, last + 0.5), Math.round(row));
  };

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    const el = listRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const row = (event.clientY - rect.top) / rowHeight - 0.5;
    hoveringRef.current = true;
    engageAt(clamp(row, -0.5, last + 0.5), Math.round(row));
  };

  const disengageScrubber = () => {
    hoveringRef.current = false;
    focusedRef.current = null;
    rawStrength.set(0);
    setEngaged(false);
  };

  const handlePointerUp = (event: React.PointerEvent<HTMLDivElement>) => {
    const el = listRef.current;
    if (el) {
      try {
        if (el.hasPointerCapture(event.pointerId)) {
          el.releasePointerCapture(event.pointerId);
        }
      } catch {
        // Safe pointer release
      }
    }
    disengageScrubber();
  };

  const handlePointerCancel = (event: React.PointerEvent<HTMLDivElement>) => {
    const el = listRef.current;
    if (el) {
      try {
        if (el.hasPointerCapture(event.pointerId)) {
          el.releasePointerCapture(event.pointerId);
        }
      } catch {
        // Safe pointer release
      }
    }
    disengageScrubber();
  };

  const handlePointerLeave = () => {
    if (hoveringRef.current) {
      disengageScrubber();
    }
  };

  const handleBlur = (event: React.FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      disengageScrubber();
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    let next = focusedRef.current ?? activeRef.current;
    switch (event.key) {
      case "ArrowDown":
      case "ArrowRight":
        next = Math.min(last, next + 1);
        break;
      case "ArrowUp":
      case "ArrowLeft":
        next = Math.max(0, next - 1);
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = last;
        break;
      default:
        return;
    }
    event.preventDefault();
    buttonsRef.current[next]?.focus();
  };

  const activeChapter: Chapter | undefined =
    last >= 0 ? visibleChapters[Math.min(activeIndex, last)] : undefined;
  // Direction of the last window move: -1 = paged to older turns, +1 = newer.
  // The tick list rides 8px against that direction as it settles, so a page
  // change reads as a scroll, not a silent swap.
  const [pageAnim, setPageAnim] = React.useState({ start: windowStart, dir: 0 });
  if (pageAnim.start !== windowStart) {
    setPageAnim({
      start: windowStart,
      dir: windowStart < pageAnim.start ? -1 : 1,
    });
  }
  const chevronClass = cn(
    "flex w-full items-center rounded outline-none cursor-pointer",
    "text-[rgb(var(--foreground-muted))] transition-colors duration-150",
    "hover:text-[rgb(var(--accent))]",
    "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
    "disabled:cursor-default disabled:opacity-25 disabled:hover:text-[rgb(var(--foreground-muted))]",
    // Share the ticks' alignment axis: the rail hugs the right edge when the
    // preview opens left, and the left edge when it opens right.
    resolvedSide === "left" ? "justify-end" : "justify-start",
  );

  return (
    <div
      ref={containerRef}
      style={{ width: peakLength }}
      className={cn("relative", className)}
    >
      {hasPages ? (
        <Tooltip label="Older turns" side="left" asChild>
          <button
            type="button"
            onClick={pageOlder}
            disabled={!canPageOlder}
            aria-label="Show older turns"
            style={{ height: CHEVRON_SIZE }}
            className={chevronClass}
          >
            {/* Same restLength box the ticks occupy, so the icon is centred on
                the exact tick line rather than on the rail's edge. */}
            <span
              aria-hidden="true"
              style={{ width: restLength }}
              className="flex items-center justify-center"
            >
              <ChevronUp size={14} strokeWidth={1.5} />
            </span>
          </button>
        </Tooltip>
      ) : null}
      <motion.div
        key={windowStart}
        initial={
          prefersReducedMotion ? false : { y: pageAnim.dir * 8, opacity: 0.25 }
        }
        animate={{ y: 0, opacity: 1 }}
        transition={{ duration: 0.18, ease: "easeOut" }}
      >
        <div
          ref={listRef}
          role="listbox"
          aria-label={label}
          aria-orientation="vertical"
          aria-activedescendant={
            engaged ? optionId(Math.max(0, Math.min(activeIndex, last))) : undefined
          }
          className="flex w-full flex-col touch-none select-none"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={handlePointerCancel}
          onPointerLeave={handlePointerLeave}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
        >
          {visibleChapters.map((chapter, index) => {
            const isCurrent = windowStart + index === currentIndex;
            const descText =
              typeof chapter.description === "string"
                ? `. ${chapter.description}`
                : "";
            return (
              <button
                ref={(el) => {
                  buttonsRef.current[index] = el;
                }}
                key={chapter.id}
                id={optionId(index)}
                type="button"
                role="option"
                aria-selected={isCurrent}
                aria-label={`${chapter.title}${descText}`}
                tabIndex={index === rovingIndex ? 0 : -1}
                onFocus={() => {
                  focusedRef.current = index;
                  engageAt(index, index);
                }}
                onClick={() => onSelect?.(chapter, windowStart + index)}
                style={{ height: rowHeight }}
                className={cn(
                  "flex w-full items-center rounded-sm outline-none cursor-pointer",
                  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
                  resolvedSide === "left" ? "justify-end" : "justify-start",
                )}
              >
                <Tick
                  index={index}
                  pointer={pointer}
                  strength={strength}
                  radius={radius}
                  restLength={restLength}
                  peakLength={peakLength}
                  isCurrent={isCurrent}
                />
              </button>
            );
          })}
        </div>
      </motion.div>
      {hasPages ? (
        <Tooltip label="Newer turns" side="left" asChild>
          <button
            type="button"
            onClick={pageNewer}
            disabled={!canPageNewer}
            aria-label="Show newer turns"
            style={{ height: CHEVRON_SIZE }}
            className={chevronClass}
          >
            <span
              aria-hidden="true"
              style={{ width: restLength }}
              className="flex items-center justify-center"
            >
              <ChevronDown size={14} strokeWidth={1.5} />
            </span>
          </button>
        </Tooltip>
      ) : null}
      {hasPages ? (
        <div
          className={cn(
            "flex w-full pt-1 text-[11px] font-mono tabular-nums text-[rgb(var(--foreground-muted))]/70 select-none",
            resolvedSide === "left" ? "justify-end" : "justify-start",
          )}
        >
          <span style={{ width: restLength }} className="flex items-center justify-center">
            {windowStart + 1}–{windowStart + visibleChapters.length}
          </span>
        </div>
      ) : null}

      {activeChapter ? (
        <motion.div
          ref={cardRef}
          aria-hidden="true"
          style={{
            top: cardTop,
            x: cardX,
            scale: cardScale,
            opacity: strength,
            ...(resolvedSide === "right"
              ? { left: peakLength + GAP }
              : { right: peakLength + GAP }),
          }}
          className={cn(
            "pointer-events-none absolute z-10 w-[240px] rounded-xl border border-[rgba(var(--border),0.08)] bg-[rgb(var(--card))]/95 px-3.5 py-3 text-[rgb(var(--foreground))]",
            "shadow-[0_1px_3px_rgba(0,0,0,0.12)]",
            resolvedSide === "right" ? "origin-left" : "origin-right",
          )}
        >
          {activeChapter.meta ? (
            <div className="mb-0.5 text-[11px] font-mono tabular-nums text-[rgb(var(--accent))]">
              {activeChapter.meta}
            </div>
          ) : null}
          <div className="truncate text-[13px] font-semibold leading-snug tracking-[-0.01em] text-[rgb(var(--foreground))]">
            {activeChapter.title}
          </div>
          {activeChapter.description ? (
            <p className="mt-1 line-clamp-3 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]">
              {activeChapter.description}
            </p>
          ) : null}
        </motion.div>
      ) : null}
    </div>
  );
}
