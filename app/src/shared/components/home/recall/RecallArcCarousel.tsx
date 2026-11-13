import React, { memo, useRef, useEffect, useCallback, useState, useMemo } from "react";
import {
  calculateArcTransform,
  RECALL_CARD_WIDTH,
  RECALL_CARD_HEIGHT,
} from "./recallArcMath";
import { RecallSessionCard } from "./RecallSessionCard";
import {
  isBoundaryCard,
  isSkeletonCard,
  type CarouselItemData,
} from "./recallTypes";
import type { SessionRow } from "@/services/persistenceService";

export interface RecallArcCarouselProps {
  items: CarouselItemData[];
  activeIndex: number;
  projectNameMap?: Map<string, string>;
  onSelectIndex: (index: number) => void;
  onLiveIndexChange?: (index: number) => void;
  onOpenSession: (session: SessionRow) => void;
  onDeleteSession?: (id: number) => void;
}

const DRAG_THRESHOLD_PX = 4;
const SLOT_PX = 200;

export const RecallArcCarousel = memo(({
  items,
  activeIndex,
  projectNameMap,
  onSelectIndex,
  onLiveIndexChange,
  onOpenSession,
  onDeleteSession,
}: RecallArcCarouselProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [stageWidth, setStageWidth] = useState<number>(1000);

  // Maximum selectable index (excludes skeletons)
  const maxSelectableIndex = useMemo(() => {
    return Math.max(0, items.filter((it) => !isSkeletonCard(it)).length - 1);
  }, [items]);

  // Synchronous active card index for visual active styling
  const [liveActiveIndex, setLiveActiveIndex] = useState<number>(activeIndex);
  const liveActiveIndexRef = useRef<number>(activeIndex);
  const onLiveIndexChangeRef = useRef(onLiveIndexChange);
  useEffect(() => {
    onLiveIndexChangeRef.current = onLiveIndexChange;
  }, [onLiveIndexChange]);

  // Physics animation state stored in refs for 60 FPS zero-re-render direct DOM writes
  const offsetRef = useRef<number>(activeIndex);
  const targetOffsetRef = useRef<number>(activeIndex);
  const velocityRef = useRef<number>(0);
  const draggingRef = useRef<boolean>(false);
  const dragStartXRef = useRef<number>(0);
  const dragStartOffsetRef = useRef<number>(0);
  const dragMovedRef = useRef<boolean>(false);
  const lastPointerXRef = useRef<number>(0);
  const lastPointerTimeRef = useRef<number>(0);
  const rafRef = useRef<number | null>(null);
  const wheelTimerRef = useRef<number | null>(null);

  // Map of card wrapper DOM elements for direct transform writes
  const cardNodesRef = useRef<Map<number, HTMLDivElement>>(new Map());

  // Measure stage width
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const updateSize = () => {
      setStageWidth(el.clientWidth || 1000);
    };
    updateSize();

    const ro = new ResizeObserver(() => updateSize());
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Update DOM transforms for each card directly without triggering React re-renders
  const applyTransforms = useCallback(() => {
    const currentOffset = offsetRef.current;
    const width = stageWidth;
    const currentCenter = Math.max(0, Math.min(maxSelectableIndex, Math.round(currentOffset)));

    // Only update state when center card actually shifts
    if (currentCenter !== liveActiveIndexRef.current) {
      liveActiveIndexRef.current = currentCenter;
      setLiveActiveIndex(currentCenter);
      onLiveIndexChangeRef.current?.(currentCenter);
    }

    items.forEach((item, idx) => {
      const node = cardNodesRef.current.get(idx);
      if (!node) return;

      const offsetFromCenter = idx - currentOffset;
      // Cards beyond viewable range are hidden via visibility to conserve GPU layers without layout recalculation
      if (Math.abs(offsetFromCenter) > 4.5) {
        if (node.style.visibility !== "hidden") {
          node.style.visibility = "hidden";
        }
        return;
      }
      if (node.style.visibility !== "visible") {
        node.style.visibility = "visible";
      }

      const t = calculateArcTransform(offsetFromCenter, width);
      node.style.transform = t.transform;
      node.style.opacity = String(t.opacity);
      node.style.zIndex = String(t.zIndex);

      const isCurrentCenter = idx === currentCenter && !isSkeletonCard(item);
      node.setAttribute("data-card-center", isCurrentCenter ? "true" : "false");
    });
  }, [items, stageWidth, maxSelectableIndex]);

  // 60 FPS smooth spring physics loop with idle dormancy (0% CPU at rest)
  const isLoopActiveRef = useRef<boolean>(false);
  const isWheelingRef = useRef<boolean>(false);

  const wakeLoop = useCallback(() => {
    if (isLoopActiveRef.current) return;
    isLoopActiveRef.current = true;

    const tick = () => {
      let keepRunning = false;

      if (draggingRef.current || isWheelingRef.current) {
        keepRunning = true;
      } else {
        // Smooth exponential ease-out toward targetOffsetRef.current
        const diff = targetOffsetRef.current - offsetRef.current;
        if (Math.abs(diff) > 0.002) {
          offsetRef.current += diff * 0.16;
          keepRunning = true;
        } else {
          offsetRef.current = targetOffsetRef.current;
        }
      }

      applyTransforms();

      if (keepRunning) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        isLoopActiveRef.current = false;
        rafRef.current = null;
      }
    };

    rafRef.current = requestAnimationFrame(tick);
  }, [applyTransforms]);

  // Sync target when activeIndex prop changes externally (e.g. from scrubber or search)
  useEffect(() => {
    if (draggingRef.current || isWheelingRef.current) return;
    if (Math.abs(targetOffsetRef.current - activeIndex) > 0.01) {
      targetOffsetRef.current = activeIndex;
      liveActiveIndexRef.current = activeIndex;
      setLiveActiveIndex(activeIndex);
      wakeLoop();
    }
  }, [activeIndex, wakeLoop]);

  // Re-trigger loop on resize or items update
  useEffect(() => {
    wakeLoop();
  }, [stageWidth, items, wakeLoop]);

  useEffect(() => {
    console.info(
      `[trace-search] RecallArcCarousel mounted @${performance.now().toFixed(1)}ms. Items: ${items.length}, activeIndex: ${activeIndex}`
    );
  }, [items.length, activeIndex]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      if (wheelTimerRef.current !== null) {
        clearTimeout(wheelTimerRef.current);
      }
      isLoopActiveRef.current = false;
    };
  }, []);

  // Pointer drag gestures
  const pointerTargetRef = useRef<HTMLElement | null>(null);

  const handlePointerDown = (e: React.PointerEvent) => {
    if (items.length <= 1) return;
    draggingRef.current = true;
    dragMovedRef.current = false;
    dragStartXRef.current = e.clientX;
    dragStartOffsetRef.current = offsetRef.current;
    lastPointerXRef.current = e.clientX;
    lastPointerTimeRef.current = performance.now();
    velocityRef.current = 0;
    pointerTargetRef.current = e.currentTarget as HTMLElement;
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const deltaX = e.clientX - dragStartXRef.current;
    if (Math.abs(deltaX) > DRAG_THRESHOLD_PX) {
      if (!dragMovedRef.current) {
        dragMovedRef.current = true;
        try {
          pointerTargetRef.current?.setPointerCapture(e.pointerId);
        } catch {
          /* ignore */
        }
      }
    }

    if (!dragMovedRef.current) return;

    const now = performance.now();
    const dt = Math.max(1, now - lastPointerTimeRef.current);
    const dx = e.clientX - lastPointerXRef.current;
    lastPointerXRef.current = e.clientX;
    lastPointerTimeRef.current = now;

    // Moving average of velocity
    const instVelocity = (-dx / SLOT_PX) * (16.67 / dt);
    velocityRef.current = velocityRef.current * 0.4 + instVelocity * 0.6;

    // Continuous offset with smooth elastic rubber-banding beyond boundaries
    const rawOffset = dragStartOffsetRef.current - deltaX / SLOT_PX;
    if (rawOffset < 0) {
      offsetRef.current = rawOffset * 0.25;
    } else if (rawOffset > maxSelectableIndex) {
      const over = rawOffset - maxSelectableIndex;
      offsetRef.current = maxSelectableIndex + over * 0.25;
    } else {
      offsetRef.current = rawOffset;
    }

    targetOffsetRef.current = Math.max(0, Math.min(maxSelectableIndex, Math.round(offsetRef.current)));
    wakeLoop();
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    try {
      if (pointerTargetRef.current?.hasPointerCapture(e.pointerId)) {
        pointerTargetRef.current.releasePointerCapture(e.pointerId);
      }
    } catch {
      /* ignore */
    }

    if (dragMovedRef.current) {
      // Calculate flick destination with bounded projection
      const flickProjection = velocityRef.current * 4.2;
      const predicted = offsetRef.current + flickProjection;
      const snapIndex = Math.max(0, Math.min(maxSelectableIndex, Math.round(predicted)));
      targetOffsetRef.current = snapIndex;
      wakeLoop();
      onSelectIndex(snapIndex);
    }
  };

  // Continuous fluid trackpad and mouse wheel navigation
  const handleWheel = (e: React.WheelEvent) => {
    const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    if (Math.abs(delta) < 1) return;

    isWheelingRef.current = true;
    const step = (delta / SLOT_PX) * 0.65;
    let nextOffset = offsetRef.current + step;
    if (nextOffset < 0) {
      nextOffset = offsetRef.current + step * 0.2;
    } else if (nextOffset > maxSelectableIndex) {
      nextOffset = offsetRef.current + step * 0.2;
    }
    offsetRef.current = nextOffset;
    targetOffsetRef.current = nextOffset;
    wakeLoop();

    // Settle cleanly onto nearest card when wheel scrolling rests
    if (wheelTimerRef.current !== null) {
      clearTimeout(wheelTimerRef.current);
    }
    wheelTimerRef.current = window.setTimeout(() => {
      isWheelingRef.current = false;
      const snapTarget = Math.max(0, Math.min(maxSelectableIndex, Math.round(offsetRef.current)));
      targetOffsetRef.current = snapTarget;
      wakeLoop();
      onSelectIndex(snapTarget);
    }, 120);
  };

  // Referentially stable card selection callback to protect React.memo across non-active cards
  const handleCardSelect = useCallback(
    (item: CarouselItemData) => {
      if (dragMovedRef.current || isSkeletonCard(item)) return;

      const idx = items.findIndex((it) => it.id === item.id);
      if (idx < 0) return;

      if (idx === liveActiveIndexRef.current) {
        if (!isBoundaryCard(item)) {
          onOpenSession(item);
        }
      } else {
        const clamped = Math.max(0, Math.min(maxSelectableIndex, idx));
        targetOffsetRef.current = clamped;
        wakeLoop();
        onSelectIndex(clamped);
      }
    },
    [items, maxSelectableIndex, onOpenSession, onSelectIndex, wakeLoop]
  );

  return (
    <div
      ref={containerRef}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      onWheel={handleWheel}
      className="relative w-full h-[220px] flex items-center justify-center overflow-visible select-none cursor-grab active:cursor-grabbing touch-pan-y"
      style={{
        perspective: "1200px",
        perspectiveOrigin: "50% 50%",
      }}
    >
      {items.map((item, idx) => {
        // Windowing optimization: only mount cards within +/- 8 slots of liveActiveIndex
        // (Visible arc is +/- 4.5 slots; +/- 8 provides wide headroom buffer for gestures).
        // Prevents mounting 100+ complex DOM nodes and allocating 100+ GPU layers on first render.
        const distFromCenter = Math.abs(idx - liveActiveIndex);
        if (distFromCenter > 8) {
          return null;
        }

        const isCenter = idx === liveActiveIndex;
        const isSkeleton = isSkeletonCard(item);
        const isBoundary = isBoundaryCard(item);
        const session = !isSkeleton && !isBoundary ? item : undefined;
        const projectName = session?.project_id
          ? projectNameMap?.get(session.project_id) ?? session.project_id
          : undefined;

        return (
          <div
            key={item.id}
            ref={(el) => {
              if (el) cardNodesRef.current.set(idx, el);
              else cardNodesRef.current.delete(idx);
            }}
            style={{
              position: "absolute",
              width: RECALL_CARD_WIDTH,
              height: RECALL_CARD_HEIGHT,
              transformStyle: "preserve-3d",
              willChange: "transform, opacity",
            }}
          >
            <RecallSessionCard
              item={item}
              isActive={isCenter}
              projectName={projectName}
              onSelect={handleCardSelect}
              onDelete={onDeleteSession}
            />
          </div>
        );
      })}
    </div>
  );
});

RecallArcCarousel.displayName = "RecallArcCarousel";
