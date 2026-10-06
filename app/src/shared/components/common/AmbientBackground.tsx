import React from "react";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { getThemeTransitioning, subscribeThemeTransition } from "@/shared/theme";
import { cn } from "@/shared/lib/utils";

type RippleShape = "circle" | "orbit";

interface AmbientBackgroundProps {
  /** X origin of the orb — ripples expand from this point */
  originX?: string;
  /** Y origin of the orb — ripples expand from this point */
  originY?: string;
  /** Speed multiplier for ripple ring expansion (e.g. 1.5 = 1.5x slower / longer interval) */
  rippleSpeedMultiplier?: number;
  /** Shape geometry of ripples — 'circle' for orb views, 'orbit' for 3D tilted chamber */
  rippleShape?: RippleShape;
  /** When true, freezes the rAF loop and CSS animations to preserve GPU budget */
  paused?: boolean;
  /**
   * Distinguishes concurrent mounts in the memory profiler. Two instances
   * (e.g. the layout-level field plus a page-level one) previously shared one
   * trace key, so mount/unmount counts were wrong for the most-suspected
   * component on the page.
   */
  instanceId?: string;
}

/** Fixed ambient tuning. The mood prop never had a caller, so the four mood
 * presets collapsed to the one reachable set of values. */
const RIPPLE_DURATION = 28; // seconds per ripple cycle
const RIPPLE_OPACITY = 0.10; // max opacity at ring origin
const GLOW_OPACITY = 0.05; // core glow under the orb

const RIPPLE_COUNT = 3;

export const AmbientBackground = React.memo(({
  originX = "50%",
  originY = "50%",
  rippleSpeedMultiplier = 1.0,
  rippleShape = "circle",
  paused = false,
  instanceId = "default",
}: AmbientBackgroundProps) => {
  useMemoryTrace(`AmbientBackground (${instanceId})`);

  const effectiveRippleDuration = RIPPLE_DURATION * rippleSpeedMultiplier;
  const telemetryRef = useTelemetry();
  const glowRef = React.useRef<HTMLDivElement>(null);
  const rippleRef = React.useRef<HTMLDivElement>(null);

  /**
   * Park the loop while a theme flip animates. The glow and ripple layers are
   * full-viewport (60vmax / inset:0) and sit behind every glass surface, so
   * writing inline opacity on them for the length of the fade forces a full
   * ambient repaint plus a re-blur of every `backdrop-filter` region above it —
   * the single largest source of the laggy, desynchronized flip.
   *
   * Reading `isFrozen` through a ref (not an effect dependency) keeps the
   * 39-62s ripple animation clock and the smoothed-energy integrator intact
   * across the freeze, so nothing snaps when the loop resumes.
   */
  const frozenRef = React.useRef(getThemeTransitioning());

  React.useEffect(() => {
    const isFrozen = () => frozenRef.current || paused;
    let animId: number | null = null;
    let smoothedEnergy = 0;
    let isRunning = false;
    let isSettled = false;

    const startLoop = () => {
      if (isRunning || document.hidden || isFrozen()) return;
      isRunning = true;
      isSettled = false;
      if (rippleRef.current) {
        rippleRef.current.style.animationPlayState = "running";
      }
      animId = requestAnimationFrame(update);
    };

    const stopLoop = () => {
      if (animId !== null) {
        cancelAnimationFrame(animId);
        animId = null;
      }
      isRunning = false;
    };

    const update = () => {
      if (document.hidden || isFrozen()) {
        stopLoop();
        return;
      }

      const energy = telemetryRef.current?.energy || 0;
      // organic, fluid interpolation
      smoothedEnergy += (energy - smoothedEnergy) * 0.15;

      const isLight =
        typeof document !== "undefined" &&
        document.documentElement.getAttribute("data-theme") === "light";
      const glowOpacityMultiplier = isLight ? 1.8 : 1.2;
      const rippleOpacityMultiplier = isLight ? 1.8 : 1.2;

      const baseGlow = GLOW_OPACITY * glowOpacityMultiplier;
      const dynamicGlow = baseGlow + smoothedEnergy * 0.12 * glowOpacityMultiplier;

      const baseRipple = RIPPLE_OPACITY * rippleOpacityMultiplier;
      const dynamicRipple = baseRipple + smoothedEnergy * 0.18 * rippleOpacityMultiplier;

      if (glowRef.current) {
        glowRef.current.style.opacity = dynamicGlow.toFixed(3);
      }
      if (rippleRef.current) {
        rippleRef.current.style.opacity = dynamicRipple.toFixed(3);
      }

      // Self-stop rAF when energy is settled at idle 0
      if (energy < 0.001 && smoothedEnergy < 0.001) {
        if (!isSettled) {
          isSettled = true;
          if (glowRef.current) glowRef.current.style.opacity = baseGlow.toFixed(3);
          if (rippleRef.current) rippleRef.current.style.opacity = baseRipple.toFixed(3);
        }
        stopLoop();
        return;
      } else {
        isSettled = false;
      }

      animId = requestAnimationFrame(update);
    };

    // Telemetry monitor check to wake up self-stopping loop (relaxed interval for low idle overhead)
    const checkInterval = setInterval(() => {
      const currentEnergy = telemetryRef.current?.energy || 0;
      if (currentEnergy > 0.005 && !isRunning && !document.hidden) {
        startLoop();
      }
    }, 600);

    const onVisibilityChange = () => {
      if (document.hidden) {
        stopLoop();
      } else {
        startLoop();
      }
    };

    // Park on theme flip, resume on completion. `smoothedEnergy` and
    // `isSettled` survive, so the loop picks up mid-interpolation.
    const onThemeTransition = () => {
      frozenRef.current = getThemeTransitioning();
      if (frozenRef.current) {
        stopLoop();
      } else {
        startLoop();
      }
    };

    frozenRef.current = getThemeTransitioning();
    startLoop();
    document.addEventListener("visibilitychange", onVisibilityChange);
    const unsubscribe = subscribeThemeTransition(onThemeTransition);

    return () => {
      stopLoop();
      clearInterval(checkInterval);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      unsubscribe();
    };
  }, [telemetryRef, paused]);

  return (
    <div
      className={cn("amb-background-container", paused && "is-paused")}
      style={{
        "--origin-x": originX,
        "--origin-y": originY,
        "--rp-dur": `${effectiveRippleDuration}s`,
      } as React.CSSProperties}
      aria-hidden="true"
    >
      {/* Deep space base gradient */}
      <div className="amb-base" />

      {/* Core glow — centered at orb origin */}
      <div ref={glowRef} className="amb-glow" />

      {/* Ripple rings */}
      <div ref={rippleRef} className="rp-wrapper">
        {/* Outward layer */}
        <div
          key={rippleShape}
          className="rp-layer"
        >
          {Array.from({ length: RIPPLE_COUNT }, (_, i) => (
            <div
              key={`${rippleShape}-out-${i}`}
              className={rippleShape === "orbit" ? "rp-ring rp-ring-out rp-ring-orbit" : "rp-ring rp-ring-out"}
              style={{
                animationDelay: `${(i * effectiveRippleDuration) / RIPPLE_COUNT}s`,
              }}
            />
          ))}
        </div>
      </div>

      {/* Noise grain overlay */}
      <div className="amb-noise" />
    </div>
  );
});

AmbientBackground.displayName = "AmbientBackground";
