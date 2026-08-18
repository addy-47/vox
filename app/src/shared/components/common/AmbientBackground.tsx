import React from "react";
import { useTelemetry } from "@/shared/hooks/useTelemetry";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";

type AmbientMood = "calm" | "active" | "thinking" | "speaking";
type RippleShape = "circle" | "orbit";

interface AmbientBackgroundProps {
  mood?: AmbientMood;
  /** X origin of the orb — ripples expand from this point */
  originX?: string;
  /** Y origin of the orb — ripples expand from this point */
  originY?: string;
  /** Speed multiplier for ripple ring expansion (e.g. 1.5 = 1.5x slower / longer interval) */
  rippleSpeedMultiplier?: number;
  /** Shape geometry of ripples — 'circle' for orb views, 'orbit' for 3D tilted chamber */
  rippleShape?: RippleShape;
}

interface MoodConfig {
  rippleDuration: number;   // seconds per ripple cycle
  rippleOpacity: number;    // max opacity at ring origin
  blobSpeed: number;        // seconds for blob morph cycle
  blobOpacity: number;      // max blob opacity
  glowOpacity: number;      // core glow under the orb
}

const MOOD_CONFIG: Record<AmbientMood, MoodConfig> = {
  calm: {
    rippleDuration: 16,
    rippleOpacity: 0.10,
    blobSpeed: 40,
    blobOpacity: 0.032,
    glowOpacity: 0.05,
  },
  thinking: {
    rippleDuration: 10,
    rippleOpacity: 0.14,
    blobSpeed: 25,
    blobOpacity: 0.045,
    glowOpacity: 0.08,
  },
  active: {
    rippleDuration: 15,
    rippleOpacity: 0.18,
    blobSpeed: 18,
    blobOpacity: 0.055,
    glowOpacity: 0.1,
  },
  speaking: {
    rippleDuration: 14,
    rippleOpacity: 0.16,
    blobSpeed: 22,
    blobOpacity: 0.05,
    glowOpacity: 0.09,
  },
};

interface BlobDef {
  x: string;
  y: string;
  size: string;
  animName: string;
  delay: number;
  borderRadius: string;
}

const BLOBS: BlobDef[] = [
  { x: "20%",  y: "30%", size: "55vmax", animName: "blob-rotate-a", delay: 0, borderRadius: "42% 58% 60% 40% / 48% 42% 58% 52%" },
  { x: "75%",  y: "65%", size: "50vmax", animName: "blob-rotate-b", delay: -15, borderRadius: "58% 42% 45% 55% / 62% 38% 62% 38%" },
  { x: "50%",  y: "15%", size: "40vmax", animName: "blob-rotate-c", delay: -28, borderRadius: "50% 50% 38% 62% / 42% 58% 42% 58%" },
];

const RIPPLE_COUNT = 5;

export const AmbientBackground = React.memo(({
  mood = "calm",
  originX = "50%",
  originY = "50%",
  rippleSpeedMultiplier = 1.0,
  rippleShape = "circle",
}: AmbientBackgroundProps) => {
  useMemoryTrace("AmbientBackground (rAF Dynamic Glow)");

  const cfg = MOOD_CONFIG[mood];
  const effectiveRippleDuration = cfg.rippleDuration * rippleSpeedMultiplier;
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

  const blobOpacityMultiplier = isLight ? 4.5 : 2.5;
  const glowOpacityMultiplier = isLight ? 3.0 : 2.0;
  const rippleOpacityMultiplier = isLight ? 2.5 : 1.8;

  React.useEffect(() => {
    let animId: number | null = null;
    let smoothedEnergy = 0;
    let isRunning = false;
    let isSettled = false;

    const startLoop = () => {
      if (isRunning || document.hidden) return;
      isRunning = true;
      isSettled = false;
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

      const baseGlow = cfg.glowOpacity * glowOpacityMultiplier;
      const dynamicGlow = baseGlow + smoothedEnergy * 0.12 * glowOpacityMultiplier;

      const baseRipple = cfg.rippleOpacity * rippleOpacityMultiplier;
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
        // Poll every 250ms when idle to check for incoming audio energy instead of continuous 60fps rAF
        stopLoop();
        return;
      } else {
        isSettled = false;
      }

      animId = requestAnimationFrame(update);
    };

    // Telemetry monitor check to wake up self-stopping loop
    const checkInterval = setInterval(() => {
      const currentEnergy = telemetryRef.current?.energy || 0;
      if (currentEnergy > 0.005 && !isRunning && !document.hidden) {
        startLoop();
      }
    }, 200);

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
  }, [cfg, glowOpacityMultiplier, rippleOpacityMultiplier, telemetryRef]);

  const rpAnimName = (mood === "active" || (mood as string) === "listening") ? "ripple-in" : "ripple-out";

  return (
    <div
      className="amb-background-container"
      style={{
        "--origin-x": originX,
        "--origin-y": originY,
        "--rp-dur": `${effectiveRippleDuration}s`,
        "--rp-anim-name": rpAnimName,
      } as React.CSSProperties}
      aria-hidden="true"
    >
      {/* Deep space base gradient */}
      <div className="amb-base" />

      {/* Organic fog blobs */}
      {BLOBS.map((blob, i) => (
        <div
          key={i}
          className="amb-blob"
          style={{
            left: blob.x,
            top: blob.y,
            width: blob.size,
            height: blob.size,
            background: `radial-gradient(circle, rgba(var(--accent), ${cfg.blobOpacity * blobOpacityMultiplier}) 0%, transparent 68%)`,
            animation: `${blob.animName} ${cfg.blobSpeed}s ease-in-out infinite`,
            animationDelay: `${blob.delay}s`,
            borderRadius: blob.borderRadius,
          }}
        />
      ))}

      {/* Core glow — centered at orb origin */}
      <div ref={glowRef} className="amb-glow" />

      {/* Ripple rings — expand from origin, circular or 3D orbit shape */}
      <div ref={rippleRef} className="rp-wrapper">
        {Array.from({ length: RIPPLE_COUNT }, (_, i) => (
          <div
            key={i}
            className={rippleShape === "orbit" ? "rp-ring rp-ring-orbit" : "rp-ring"}
            style={{
              animationDelay: `${(i * effectiveRippleDuration) / RIPPLE_COUNT}s`,
            }}
          />
        ))}
      </div>

      {/* Noise grain overlay */}
      <div className="amb-noise" />
    </div>
  );
});

AmbientBackground.displayName = "AmbientBackground";
