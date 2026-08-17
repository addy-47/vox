import React, { useCallback, useEffect, useRef } from "react";
import {
  depthFromAngle,
  distributeAngles,
  ellipsePoint,
  ORBIT_CARD_OPACITY_MIN,
  ORBIT_CARD_SCALE_MAX,
  ORBIT_CARD_SCALE_MIN,
  ORBIT_CARD_SELECTED_BOOST,
  zIndexForAngle,
} from "./orbitMath";
import { ChamberOrbitRings } from "./ChamberOrbitRings";

export interface OrbitCarouselProps {
  /** Ordered newest-first — the first id always rests at the front. */
  nodeIds: string[];
  /** Ring radius in px, computed from the viewport by the parent. */
  radius: number;
  selectedId?: string | null;
  /** Freeze interaction (e.g. detail panel open). */
  paused?: boolean;
  /** Reports whether the most recent pointer gesture was a drag (click suppression). */
  onDragStateChange?: (moved: boolean) => void;
  /** Renders the card body for a node id; the carousel owns positioning/depth styling. */
  renderNode?: (id: string) => React.ReactNode;
}

const MOMENTUM_DAMPING = 0.94;
const MOMENTUM_STOP = 0.00025;
const MOMENTUM_MAX_MS = 1600;
const DRAG_THRESHOLD_PX = 3;

export const OrbitCarousel: React.FC<OrbitCarouselProps> = ({
  nodeIds,
  radius,
  selectedId = null,
  paused = false,
  onDragStateChange,
  renderNode,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);

  // Continuous ring rotation state (radians)
  const angleRef = useRef<number>(0);
  const velocityRef = useRef<number>(0);
  const draggingRef = useRef<boolean>(false);
  const lastPointerXRef = useRef<number>(0);
  const lastPointerTimeRef = useRef<number>(0);
  const decayStartRef = useRef<number>(0);
  const reducedMotionRef = useRef<boolean>(false);
  const rafRef = useRef<number | null>(null);

  // Projection state — imperative DOM writes, zero React re-renders per frame
  const nodeElsRef = useRef(new Map<string, HTMLDivElement>());
  const styleCacheRef = useRef(new Map<string, string>());
  const baseAnglesRef = useRef(new Map<string, number>());

  // Latest props mirrored into refs so the loop never stalls
  const nodeIdsRef = useRef(nodeIds);
  nodeIdsRef.current = nodeIds;
  const radiusRef = useRef(radius);
  radiusRef.current = radius;
  const selectedIdRef = useRef(selectedId);
  selectedIdRef.current = selectedId;
  const pausedRef = useRef(paused);
  pausedRef.current = paused;
  const onDragStateChangeRef = useRef(onDragStateChange);
  onDragStateChangeRef.current = onDragStateChange;

  // ── Deterministic base angles — newest-first, front slot = newest ──────────
  const rebuildBaseAngles = useCallback((ids: string[]) => {
    baseAnglesRef.current.clear();
    styleCacheRef.current.clear();
    const angles = distributeAngles(ids.length);
    for (let i = 0; i < ids.length; i++) {
      baseAnglesRef.current.set(ids[i], angles[i]);
    }
    angleRef.current = 0;
  }, []);

  // ── 3D Perspective Projection ──────────────────────────────────────────────
  const projectFrame = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;
    const ids = nodeIdsRef.current;
    const radiusNow = radiusRef.current;
    const halfW = container.clientWidth / 2;
    const halfH = container.clientHeight / 2;

    for (const id of ids) {
      const el = nodeElsRef.current.get(id);
      if (!el) continue;

      const base = baseAnglesRef.current.get(id);
      if (base === undefined) continue;

      const angle = base + angleRef.current;
      const p = ellipsePoint(angle, radiusNow);
      const x = halfW + p.x;
      const y = halfH + p.y;

      const depth = depthFromAngle(angle);
      const isSelected = id === selectedIdRef.current;
      let scale = ORBIT_CARD_SCALE_MIN + depth * (ORBIT_CARD_SCALE_MAX - ORBIT_CARD_SCALE_MIN);
      if (isSelected) scale *= ORBIT_CARD_SELECTED_BOOST;
      const opacity = ORBIT_CARD_OPACITY_MIN + depth * (1 - ORBIT_CARD_OPACITY_MIN);
      const zIndex = zIndexForAngle(angle, isSelected);
      
      // Depth blur for cards traversing the deep back half of the 3D chamber
      const blurPx = depth < 0.45 ? ((0.45 - depth) * 2.5).toFixed(1) : "0";

      // Dirty-checked imperative style write — sub-pixel unchanged frames are skipped
      const cacheKey = `${x.toFixed(1)},${y.toFixed(1)},${scale.toFixed(3)},${opacity.toFixed(2)},${zIndex},${blurPx}`;
      if (styleCacheRef.current.get(id) === cacheKey) continue;
      styleCacheRef.current.set(id, cacheKey);
      el.style.transform = `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0) translate(-50%, -50%) scale(${scale.toFixed(3)})`;
      el.style.opacity = opacity.toFixed(2);
      el.style.zIndex = String(zIndex);
      el.style.filter = blurPx === "0" ? "none" : `blur(${blurPx}px)`;
    }
  }, []);

  // Rebuild base angles and synchronously project when nodeIds change
  useEffect(() => {
    rebuildBaseAngles(nodeIds);
    projectFrame();
  }, [nodeIds, rebuildBaseAngles, projectFrame]);

  // Project once when the ring is resized or content changes
  useEffect(() => {
    projectFrame();
  }, [projectFrame, radius, nodeIds]);

  // ── Self-stopping loop — runs only while dragging or decaying momentum ─────
  const stopLoop = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  const loop = useCallback(() => {
    rafRef.current = null;

    if (pausedRef.current) return;

    if (!draggingRef.current) {
      velocityRef.current *= MOMENTUM_DAMPING;
      if (
        Math.abs(velocityRef.current) < MOMENTUM_STOP ||
        performance.now() - decayStartRef.current > MOMENTUM_MAX_MS
      ) {
        velocityRef.current = 0;
        projectFrame();
        return;
      }
      angleRef.current += velocityRef.current;
    }

    projectFrame();
    rafRef.current = requestAnimationFrame(loop);
  }, [projectFrame]);

  const startLoop = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = requestAnimationFrame(loop);
  }, [loop]);

  // Project once when the ring is resized or content changes (resting state)
  useEffect(() => {
    projectFrame();
  }, [projectFrame, radius, nodeIds]);

  useEffect(() => stopLoop, [stopLoop]);

  // ── Reduced-motion awareness ───────────────────────────────────────────────
  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => {
      reducedMotionRef.current = media.matches;
      if (media.matches) velocityRef.current = 0;
    };
    updateMotion();
    media.addEventListener("change", updateMotion);
    return () => media.removeEventListener("change", updateMotion);
  }, []);

  // ── Pointer interaction (direct drag + momentum inertia) ───────────────────
  const handlePointerDown = (e: React.PointerEvent) => {
    if (pausedRef.current) return;
    draggingRef.current = true;
    lastPointerXRef.current = e.clientX;
    lastPointerTimeRef.current = performance.now();
    velocityRef.current = 0;
    decayStartRef.current = performance.now();
    onDragStateChangeRef.current?.(false);
    startLoop();
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    const now = performance.now();
    const deltaX = e.clientX - lastPointerXRef.current;
    const deltaTime = Math.max(now - lastPointerTimeRef.current, 1);

    if (Math.abs(deltaX) > DRAG_THRESHOLD_PX) {
      onDragStateChangeRef.current?.(true);
      try {
        if (!e.currentTarget.hasPointerCapture(e.pointerId)) {
          (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        }
      } catch {
        // ignore
      }
    }

    const radiusNow = Math.max(radiusRef.current, 1);
    const angularDelta = -deltaX / radiusNow;
    angleRef.current += angularDelta;
    velocityRef.current = (angularDelta / deltaTime) * 16.0;
    decayStartRef.current = now;

    lastPointerXRef.current = e.clientX;
    lastPointerTimeRef.current = now;
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    if (reducedMotionRef.current) velocityRef.current = 0;
    decayStartRef.current = performance.now();
    try {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) {
        (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      }
    } catch {
      // ignore
    }
  };

  const handleRegisterNode = useCallback(
    (id: string, el: HTMLDivElement | null) => {
      if (el) {
        nodeElsRef.current.set(id, el);
        // Immediately project initial 3D position to prevent any top-left flash before rAF
        const container = containerRef.current;
        const base = baseAnglesRef.current.get(id);
        if (container && base !== undefined) {
          const halfW = container.clientWidth / 2;
          const halfH = container.clientHeight / 2;
          const angle = base + angleRef.current;
          const p = ellipsePoint(angle, radiusRef.current);
          const x = halfW + p.x;
          const y = halfH + p.y;
          const depth = depthFromAngle(angle);
          const isSelected = id === selectedIdRef.current;
          let scale =
            ORBIT_CARD_SCALE_MIN +
            depth * (ORBIT_CARD_SCALE_MAX - ORBIT_CARD_SCALE_MIN);
          if (isSelected) scale *= ORBIT_CARD_SELECTED_BOOST;
          const opacity =
            ORBIT_CARD_OPACITY_MIN + depth * (1 - ORBIT_CARD_OPACITY_MIN);
          const zIndex = zIndexForAngle(angle, isSelected);
          const blurPx =
            depth < 0.45 ? ((0.45 - depth) * 2.5).toFixed(1) : "0";

          el.style.transform = `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0) translate(-50%, -50%) scale(${scale.toFixed(3)})`;
          el.style.opacity = opacity.toFixed(2);
          el.style.zIndex = String(zIndex);
          el.style.filter = blurPx === "0" ? "none" : `blur(${blurPx}px)`;
        }
      } else {
        nodeElsRef.current.delete(id);
        styleCacheRef.current.delete(id);
      }
    },
    []
  );

  const registerRefCb = useCallback(
    (id: string) => (el: HTMLDivElement | null) => handleRegisterNode(id, el),
    [handleRegisterNode]
  );

  return (
    <div
      ref={containerRef}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      className="absolute inset-0 w-full h-full cursor-grab active:cursor-grabbing select-none overflow-hidden"
      style={{ touchAction: "none" }}
    >
      {/* 3D Luminous Concentric Chamber Rings & Front-Arc Spotlight */}
      <ChamberOrbitRings radius={radius} />

      {/* Imperatively-positioned 3D orbit card layer (stable DOM nodes, zero re-render per frame) */}
      <div className="absolute inset-0 pointer-events-none">
        {renderNode &&
          nodeIds.map((id) => (
            <div
              key={id}
              ref={registerRefCb(id)}
              className="absolute left-0 top-0 pointer-events-auto will-change-transform opacity-0"
              style={{ transform: "translate3d(-9999px, -9999px, 0)" }}
            >
              {renderNode(id)}
            </div>
          ))}
      </div>
    </div>
  );
};

OrbitCarousel.displayName = "OrbitCarousel";
