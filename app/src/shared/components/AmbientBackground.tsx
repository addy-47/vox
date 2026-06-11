import React, { useId } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────

type AmbientMood = "calm" | "active" | "thinking" | "speaking";

interface AmbientBackgroundProps {
  mood?: AmbientMood;
  /** X origin of the orb — ripples expand from this point */
  originX?: string;
  /** Y origin of the orb — ripples expand from this point */
  originY?: string;
}

// ─── Mood configuration ───────────────────────────────────────────────────────

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
    rippleDuration: 8,
    rippleOpacity: 0.18,
    blobSpeed: 18,
    blobOpacity: 0.055,
    glowOpacity: 0.1,
  },
  speaking: {
    rippleDuration: 9,
    rippleOpacity: 0.16,
    blobSpeed: 22,
    blobOpacity: 0.05,
    glowOpacity: 0.09,
  },
};

// ─── Blob definitions ─────────────────────────────────────────────────────────
// Each blob has a starting position and its own morph animation name.
// Positions are staggered so they never perfectly overlap.
// Opacity is kept extremely low — the goal is texture, not visibility.

interface BlobDef {
  x: string;
  y: string;
  size: string;
  animName: string;
  delay: number;
}

const BLOBS: BlobDef[] = [
  { x: "20%",  y: "30%", size: "55vmax", animName: "blob-a", delay: 0 },
  { x: "75%",  y: "65%", size: "50vmax", animName: "blob-b", delay: -15 },
  { x: "50%",  y: "15%", size: "40vmax", animName: "blob-c", delay: -28 },
];

// ─── Ripple ring count ────────────────────────────────────────────────────────
const RIPPLE_COUNT = 5;

// ─── Component ────────────────────────────────────────────────────────────────

export const AmbientBackground: React.FC<AmbientBackgroundProps> = ({
  mood = "calm",
  originX = "50%",
  originY = "50%",
}) => {
  const uid = useId().replace(/:/g, "-");
  const cfg = MOOD_CONFIG[mood];

  return (
    <div
      className="fixed inset-0 pointer-events-none overflow-hidden select-none"
      style={{ zIndex: 0, transform: "translateZ(0)", willChange: "transform" }}
      aria-hidden="true"
    >
      <style>{`
        /* ── Deep space base ── */
        .amb-base-${uid} {
          position: absolute;
          inset: 0;
          background:
            radial-gradient(
              ellipse 80% 70% at ${originX} ${originY},
              hsl(230 30% 6%) 0%,
              hsl(230 20% 3%) 55%,
              hsl(0 0% 1%) 100%
            );
        }
        [data-theme='light'] .amb-base-${uid} {
          background:
            radial-gradient(
              ellipse 80% 70% at ${originX} ${originY},
              hsl(220 60% 97%) 0%,
              hsl(220 40% 94%) 55%,
              hsl(220 20% 92%) 100%
            );
        }

        /* ── Organic blob morph keyframes ──
         * Uses border-radius morphing only — fully GPU composited.
         * Three independent animations so blobs never sync. */
        @keyframes blob-a {
          0%,100% { border-radius: 42% 58% 60% 40% / 48% 42% 58% 52%; }
          20%      { border-radius: 60% 40% 42% 58% / 38% 62% 38% 62%; }
          40%      { border-radius: 38% 62% 55% 45% / 58% 38% 62% 42%; }
          60%      { border-radius: 55% 45% 38% 62% / 42% 58% 42% 58%; }
          80%      { border-radius: 48% 52% 62% 38% / 55% 45% 55% 45%; }
        }
        @keyframes blob-b {
          0%,100% { border-radius: 58% 42% 45% 55% / 62% 38% 62% 38%; }
          25%      { border-radius: 42% 58% 62% 38% / 45% 55% 38% 62%; }
          50%      { border-radius: 62% 38% 48% 52% / 38% 62% 55% 45%; }
          75%      { border-radius: 38% 62% 38% 62% / 62% 38% 42% 58%; }
        }
        @keyframes blob-c {
          0%,100% { border-radius: 50% 50% 38% 62% / 42% 58% 42% 58%; }
          33%      { border-radius: 62% 38% 55% 45% / 58% 42% 62% 38%; }
          66%      { border-radius: 38% 62% 42% 58% / 50% 50% 38% 62%; }
        }

        /* ── Ripple rings ──
         * Ring starts at ≈ orb diameter (280px square), ends at ~8x.
         * This matches the orb's visual bounds and expands past all edges.
         * Only transform + opacity animated → GPU composited. */
        @keyframes ripple-out-${uid} {
          0%   { transform: translate(-50%, -50%) scale(1); opacity: var(--rp-opacity); }
          100% { transform: translate(-50%, -50%) scale(8); opacity: 0; }
        }

        .rp-ring-${uid} {
          position: absolute;
          left: ${originX};
          top: ${originY};
          width: 280px;
          height: 280px;
          border-radius: 50%;
          border: 1px solid rgba(var(--accent), 0.5);
          transform: translate(-50%, -50%) scale(1);
          will-change: transform, opacity;
          animation: ripple-out-${uid} var(--rp-dur) cubic-bezier(0.1, 0.4, 0.2, 1) infinite;
        }

        /* ── Core glow under orb ── */
        .amb-glow-${uid} {
          position: absolute;
          left: ${originX};
          top: ${originY};
          transform: translate(-50%, -50%);
          width: 60vmax;
          height: 60vmax;
          border-radius: 50%;
          background: radial-gradient(
            circle,
            rgba(var(--accent), var(--glow-opacity)) 0%,
            transparent 65%
          );
          transition: opacity 1.2s ease;
          pointer-events: none;
        }

        /* ── Noise grain ── */
        .amb-noise-${uid} {
          position: absolute;
          inset: 0;
          background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.75' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.45'/%3E%3C/svg%3E");
          background-size: 200px 200px;
          opacity: 0.08;
          mix-blend-mode: overlay;
          pointer-events: none;
        }
        [data-theme='light'] .amb-noise-${uid} {
          mix-blend-mode: multiply;
          opacity: 0.05;
        }

        /* ── Reduced motion: disable all animations ── */
        @media (prefers-reduced-motion: reduce) {
          .rp-ring-${uid},
          .amb-blob-${uid} {
            animation: none !important;
            opacity: 0 !important;
          }
        }
      `}</style>

      {/* Deep space base gradient */}
      <div className={`amb-base-${uid}`} />

      {/* Organic fog blobs — very low opacity, always accent color */}
      {BLOBS.map((blob, i) => (
        <div
          key={i}
          className={`amb-blob-${uid}`}
          style={{
            position: "absolute",
            left: blob.x,
            top: blob.y,
            width: blob.size,
            height: blob.size,
            transform: "translate(-50%, -50%)",
            background: `radial-gradient(circle, rgba(var(--accent), ${cfg.blobOpacity}) 0%, transparent 68%)`,
            animation: `${blob.animName} ${cfg.blobSpeed}s ease-in-out infinite`,
            animationDelay: `${blob.delay}s`,
            willChange: "border-radius",
            borderRadius: "50%",
          }}
        />
      ))}

      {/* Core glow — centered at orb origin */}
      <div
        className={`amb-glow-${uid}`}
        style={{
          ["--glow-opacity" as string]: cfg.glowOpacity,
          transition: "opacity 1.5s ease",
        }}
      />

      {/* Ripple rings — expand from orb origin to screen edges */}
      <div
        style={{
          ["--rp-opacity" as string]: cfg.rippleOpacity,
          ["--rp-dur" as string]: `${cfg.rippleDuration}s`,
        }}
      >
        {Array.from({ length: RIPPLE_COUNT }, (_, i) => (
          <div
            key={i}
            className={`rp-ring-${uid}`}
            style={{
              animationDelay: `${(i * cfg.rippleDuration) / RIPPLE_COUNT}s`,
            }}
          />
        ))}
      </div>

      {/* Noise grain overlay */}
      <div className={`amb-noise-${uid}`} />
    </div>
  );
};
