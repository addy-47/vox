import React from "react";

type AmbientMood = "calm" | "active" | "thinking" | "speaking";

interface AmbientBackgroundProps {
  mood?: AmbientMood;
  /** X origin of the orb — ripples expand from this point */
  originX?: string;
  /** Y origin of the orb — ripples expand from this point */
  originY?: string;
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
}: AmbientBackgroundProps) => {
  const cfg = MOOD_CONFIG[mood];

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

  const blobOpacityMultiplier = isLight ? 4.5 : 1.0;
  const glowOpacityMultiplier = isLight ? 3.0 : 1.0;
  const rippleOpacityMultiplier = isLight ? 2.5 : 1.0;

  return (
    <div
      className="amb-background-container"
      style={{
        "--origin-x": originX,
        "--origin-y": originY,
        "--rp-opacity": cfg.rippleOpacity * rippleOpacityMultiplier,
        "--rp-dur": `${cfg.rippleDuration}s`,
        "--glow-opacity": cfg.glowOpacity * glowOpacityMultiplier,
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
      <div className="amb-glow" />

      {/* Ripple rings — expand from orb origin to screen edges */}
      <div>
        {Array.from({ length: RIPPLE_COUNT }, (_, i) => (
          <div
            key={i}
            className="rp-ring"
            style={{
              animationDelay: `${(i * cfg.rippleDuration) / RIPPLE_COUNT}s`,
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
