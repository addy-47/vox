import React, { memo, useEffect, useRef } from 'react';
import type { InteractionState } from '@/services/eventsService';
import { isViewportResizing } from '@/layout/viewportResize';
import { OrbitFieldEngine, resolveFieldKey } from './orbitFieldEngine';

export interface OrbitFieldProps {
  interactionState?: InteractionState;
  /** Resolved activity key (activity.name, else orb preset such as 'web'). */
  activity?: string;
  paused?: boolean;
  children: React.ReactNode;
}

const FRAME_MS = 1000 / 30; // living background doesn't need 60fps

function readAccent(): [number, number, number] {
  const v = getComputedStyle(document.documentElement).getPropertyValue('--accent').split(',').map((s) => parseInt(s, 10));
  return v.length === 3 && !v.some(Number.isNaN) ? [v[0], v[1], v[2]] : [0, 219, 233];
}

/**
 * Wraps the orb. Draws the orbit field on a viewport-sized canvas BEHIND the orb (so it is not
 * clipped by the stage) and a small canvas IN FRONT of the orb for the near halves of the orbits.
 * AdvancedOrb is untouched. Replaces the earlier OrbHalo.
 */
export const OrbitField = memo(({ interactionState, activity, paused, children }: OrbitFieldProps) => {
  const wrapRef = useRef<HTMLDivElement>(null);
  const backRef = useRef<HTMLCanvasElement>(null);
  const frontRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<OrbitFieldEngine | null>(null);
  const loopRef = useRef<{ start: () => void } | null>(null);
  const pausedRef = useRef(false);
  pausedRef.current = !!paused;

  useEffect(() => {
    const engine = new OrbitFieldEngine(backRef.current!, frontRef.current!, wrapRef.current!);
    engineRef.current = engine;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    const syncTheme = () => engine.setColors(readAccent(), document.documentElement.dataset.theme === 'light');
    const relayout = () => engine.layout(window.devicePixelRatio || 1);
    syncTheme(); relayout();

    const ro = new ResizeObserver(relayout);
    ro.observe(wrapRef.current!);
    window.addEventListener('resize', relayout);
    const mo = new MutationObserver(syncTheme);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] });

    let raf = 0, last = 0, running = false;
    const tick = (now: number) => {
      if (document.hidden) { running = false; return; }
      raf = requestAnimationFrame(tick);
      if (now - last < FRAME_MS) return;
      const dt = Math.min((now - last) / 1000, 0.1); last = now;
      if (pausedRef.current || isViewportResizing()) return;
      engine.frame(reduced ? 0 : dt);
      if (engine.isIdle()) { cancelAnimationFrame(raf); running = false; }
    };
    const start = () => { if (running || document.hidden) return; running = true; last = performance.now(); raf = requestAnimationFrame(tick); };
    loopRef.current = { start };
    const onVis = () => { if (!document.hidden) start(); };
    document.addEventListener('visibilitychange', onVis);

    return () => {
      cancelAnimationFrame(raf); ro.disconnect(); mo.disconnect();
      window.removeEventListener('resize', relayout); document.removeEventListener('visibilitychange', onVis);
      engineRef.current = null; loopRef.current = null;
    };
  }, []);

  const key = resolveFieldKey(interactionState, activity);
  useEffect(() => {
    engineRef.current?.setKey(key);
    if (key) { engineRef.current?.layout(window.devicePixelRatio || 1); loopRef.current?.start(); }
  }, [key]);

  return (
    <div ref={wrapRef} className="relative w-full h-full" style={{ overflow: 'visible' }}>
      <canvas ref={backRef} aria-hidden="true" style={{ position: 'absolute', zIndex: 0, pointerEvents: 'none' }} />
      <div className="relative w-full h-full" style={{ zIndex: 1 }}>{children}</div>
      <canvas ref={frontRef} aria-hidden="true" style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', zIndex: 2, pointerEvents: 'none' }} />
    </div>
  );
});
OrbitField.displayName = 'OrbitField';
