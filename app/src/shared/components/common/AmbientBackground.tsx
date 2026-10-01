import React from "react";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
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
const BLOB_SPEED = 40; // seconds for blob morph cycle
const BLOB_OPACITY = 0.032; // max blob opacity
const GLOW_OPACITY = 0.05; // core glow under the orb

interface BlobDef {
  x: string;
  y: string;
  size: string;
  animName: string;
  delay: number;
  borderRadius: string;
}

const BLOBS: BlobDef[] = [
  { x: "20%",  y: "30%", size: "42vmax", animName: "blob-rotate-a", delay: 0, borderRadius: "42% 58% 60% 40% / 48% 42% 58% 52%" },
  { x: "75%",  y: "65%", size: "36vmax", animName: "blob-rotate-b", delay: -15, borderRadius: "58% 42% 45% 55% / 62% 38% 62% 38%" },
];

const RIPPLE_COUNT = 5;

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

  const [isLight, setIsLight] = React.useState(false);
  React.useEffect(() => {
    const checkTheme = () => {
      setIsLight(document.documentElement.getAttribute('data-theme') === 'light');
    };
    checkTheme();
    const observer = new MutationObserver(checkTheme);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);

  const blobOpacityMultiplier = isLight ? 2.5 : 1.5;
  const glowOpacityMultiplier = isLight ? 1.8 : 1.2;
  const rippleOpacityMultiplier = isLight ? 1.8 : 1.2;

  const blobRefs = React.useRef<(HTMLDivElement | null)[]>([]);

  React.useEffect(() => {
    if (paused) {
      return;
    }

    let animId: number | null = null;
    let smoothedEnergy = 0;
    let isRunning = false;
    let isSettled = false;

    const startLoop = () => {
      if (isRunning || document.hidden) return;
      isRunning = true;
      isSettled = false;
      // Re-enable willChange on active wake
      blobRefs.current.forEach((el) => {
        if (el) el.style.willChange = "transform";
      });
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
      if (document.hidden) {
        stopLoop();
        return;
      }

      const energy = telemetryRef.current?.energy || 0;
      // organic, fluid interpolation
      smoothedEnergy += (energy - smoothedEnergy) * 0.15;

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
          // Demote compositor layer on idle
          blobRefs.current.forEach((el) => {
            if (el) el.style.willChange = "auto";
          });
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

    startLoop();
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      stopLoop();
      clearInterval(checkInterval);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [glowOpacityMultiplier, rippleOpacityMultiplier, telemetryRef, paused]);

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

      {/* Organic fog blobs */}
      {BLOBS.map((blob, i) => (
        <div
          key={i}
          ref={(el) => {
            blobRefs.current[i] = el;
          }}
          className="amb-blob"
          style={{
            left: blob.x,
            top: blob.y,
            width: blob.size,
            height: blob.size,
            background: `radial-gradient(circle, rgba(var(--accent), ${BLOB_OPACITY * blobOpacityMultiplier}) 0%, transparent 68%)`,
            animation: `${blob.animName} ${BLOB_SPEED}s ease-in-out infinite`,
            animationPlayState: paused ? "paused" : "running",
            animationDelay: `${blob.delay}s`,
            borderRadius: blob.borderRadius,
            willChange: paused ? "auto" : "transform",
          }}
        />
      ))}

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
