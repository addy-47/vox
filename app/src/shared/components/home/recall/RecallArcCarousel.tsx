import React, { memo, useRef, useEffect, useCallback, useState, useMemo } from "react";
import {
  calculateArcTransform,
  RECALL_CARD_WIDTH,
  RECALL_CARD_HEIGHT,
} from "./recallArcMath";
import {
  RecallSessionCard,
} from "./RecallSessionCard";
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
  onOpenSession: (session: SessionRow) => void;
  onDeleteSession?: (id: number) => void;
}

const DRAG_THRESHOLD_PX = 4;
const MOMENTUM_DAMPING = 0.92;
const MOMENTUM_STOP = 0.001;

export const RecallArcCarousel = memo(({
  items,
  activeIndex,
  projectNameMap,
  onSelectIndex,
  onOpenSession,
  onDeleteSession,
}: RecallArcCarouselProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [stageWidth, setStageWidth] = useState<number>(1000);

  // Maximum selectable index (excludes skeletons to forbid scrolling into placeholders)
  const maxSelectableIndex = useMemo(() => {
    return Math.max(0, items.filter((it) => !isSkeletonCard(it)).length - 1);
  }, [items]);

  // Synchronous live active index updated immediately per frame during drag
  const [liveActiveIndex, setLiveActiveIndex] = useState<number>(activeIndex);

  // High-frequency animation and physics state stored in refs (zero React re-renders per frame)
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

  // Update DOM transforms and active attributes for each card directly
  const applyTransforms = useCallback(() => {
    const currentOffset = offsetRef.current;
    const width = stageWidth;
    const currentCenter = Math.max(0, Math.min(maxSelectableIndex, Math.round(currentOffset)));

    items.forEach((item, idx) => {
      const node = cardNodesRef.current.get(idx);
      if (!node) return;

      const offsetFromCenter = idx - currentOffset;
      // Cards beyond viewable range (e.g. |offset| > 4.5) are hidden to release GPU layers
      if (Math.abs(offsetFromCenter) > 4.5) {
        node.style.display = "none";
        node.style.willChange = "auto";
        return;
      }
      node.style.display = "block";
      node.style.willChange = "transform, opacity";

      const t = calculateArcTransform(offsetFromCenter, width);
      node.style.transform = t.transform;
      node.style.opacity = String(t.opacity);
      node.style.zIndex = String(t.zIndex);

      const isCurrentCenter = idx === currentCenter && !isSkeletonCard(item);
      node.setAttribute("data-card-center", isCurrentCenter ? "true" : "false");
    });
  }, [items, stageWidth, maxSelectableIndex]);

  // 60 FPS physics loop with idle dormancy (drops to 0% CPU when at rest)
  const isLoopActiveRef = useRef<boolean>(false);

  const wakeLoop = useCallback(() => {
    if (isLoopActiveRef.current) return;
    isLoopActiveRef.current = true;

    const tick = () => {
      let keepRunning = false;

      if (draggingRef.current) {
        keepRunning = true;
      } else {
        // Spring easing toward target offset
        const diff = targetOffsetRef.current - offsetRef.current;
        if (Math.abs(velocityRef.current) > MOMENTUM_STOP) {
          offsetRef.current += velocityRef.current;
          velocityRef.current *= MOMENTUM_DAMPING;

          // Clamp to bounds with spring back (hard stop past maxSelectableIndex)
          if (offsetRef.current < 0) {
            velocityRef.current = 0;
            targetOffsetRef.current = 0;
          } else if (offsetRef.current > maxSelectableIndex) {
            velocityRef.current = 0;
            targetOffsetRef.current = maxSelectableIndex;
            offsetRef.current = maxSelectableIndex;
          } else {
            targetOffsetRef.current = Math.round(offsetRef.current);
          }
          keepRunning = true;
        } else if (Math.abs(diff) > 0.001) {
          offsetRef.current += diff * 0.18;
          keepRunning = true;
        } else {
          offsetRef.current = targetOffsetRef.current;
        }
      }

      // Synchronize live active card index continuously with drag
      const rounded = Math.max(0, Math.min(maxSelectableIndex, Math.round(offsetRef.current)));
      setLiveActiveIndex((prev) => (prev !== rounded ? rounded : prev));

      applyTransforms();

      if (keepRunning) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        isLoopActiveRef.current = false;
        rafRef.current = null;
      }
    };

    rafRef.current = requestAnimationFrame(tick);
  }, [maxSelectableIndex, applyTransforms]);

  // Sync target when activeIndex prop changes externally (e.g. from scrubber or search)
  useEffect(() => {
    targetOffsetRef.current = activeIndex;
    setLiveActiveIndex(activeIndex);
    wakeLoop();
  }, [activeIndex, wakeLoop]);

  // Re-trigger loop on resize or items update
  useEffect(() => {
    wakeLoop();
  }, [stageWidth, items, wakeLoop]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
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

    // Convert pixel drag to offset units (~180px per card slot)
    const slotPixels = 180;
    const offsetDelta = -deltaX / slotPixels;
    const rawOffset = dragStartOffsetRef.current + offsetDelta;

    // Hard clamp: never scroll past latest session (maxSelectableIndex) into skeleton territory
    if (rawOffset < 0) {
      offsetRef.current = rawOffset * 0.2; // soft resistance at start
    } else if (rawOffset > maxSelectableIndex) {
      offsetRef.current = maxSelectableIndex; // HARD CLAMP: cannot drag into skeleton territory
    } else {
      offsetRef.current = rawOffset;
    }

    const now = performance.now();
    const dt = Math.max(1, now - lastPointerTimeRef.current);
    const dx = e.clientX - lastPointerXRef.current;
    velocityRef.current = (-dx / slotPixels) * (16 / dt); // normalized to 60fps frame
    lastPointerXRef.current = e.clientX;
    lastPointerTimeRef.current = now;
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
      // Determine nearest card index and notify parent
      const predictedOffset = offsetRef.current + velocityRef.current * 6;
      const clampedIndex = Math.max(0, Math.min(maxSelectableIndex, Math.round(predictedOffset)));
      targetOffsetRef.current = clampedIndex;
      wakeLoop();
      onSelectIndex(clampedIndex);
    }
  };

  // Wheel horizontal navigation
  const handleWheel = (e: React.WheelEvent) => {
    if (Math.abs(e.deltaX) > 15 || Math.abs(e.deltaY) > 15) {
      const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      if (Math.abs(delta) > 20) {
        if (delta > 0) {
          onSelectIndex(Math.min(activeIndex + 1, maxSelectableIndex));
        } else {
          onSelectIndex(Math.max(activeIndex - 1, 0));
        }
      }
    }
  };

  // Card click: center it if side card; open transcript drawer if already center
  const handleCardClick = (idx: number, item: CarouselItemData) => {
    if (dragMovedRef.current || isSkeletonCard(item)) return;

    if (idx === liveActiveIndex) {
      if (!isBoundaryCard(item)) {
        onOpenSession(item);
      }
    } else {
      const clamped = Math.max(0, Math.min(maxSelectableIndex, idx));
      targetOffsetRef.current = clamped;
      wakeLoop();
      onSelectIndex(clamped);
    }
  };

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
            }}
          >
            <RecallSessionCard
              item={item}
              isActive={isCenter}
              projectName={projectName}
              onSelect={() => handleCardClick(idx, item)}
              onDelete={onDeleteSession}
            />
          </div>
        );
      })}
    </div>
  );
});

RecallArcCarousel.displayName = "RecallArcCarousel";
