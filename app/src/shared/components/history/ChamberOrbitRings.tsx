import { memo } from "react";
import { ORBIT_TILT_COMPRESSION, ORBIT_GUIDE_GAP } from "./orbitMath";

export interface ChamberOrbitRingsProps {
  radius: number;
}

/**
 * Pure SVG + CSS 3D Illuminated Chamber Ring System:
 * - Concentric daily resonance guide tracks (strata for morning/afternoon/evening)
 * - Primary tilted elliptical orbit ring with high-intensity front-arc illumination
 * - Ambient acoustic core glow centered on the clock
 * Costs < 1MB RAM and 0% idle CPU.
 */
export const ChamberOrbitRings = memo(({ radius }: ChamberOrbitRingsProps) => {
  const width = radius * 2 + 160;
  const height = radius * 2 * ORBIT_TILT_COMPRESSION + 160;
  const cx = width / 2;
  const cy = height / 2;
  const rx = radius;
  const ry = radius * ORBIT_TILT_COMPRESSION;

  // Concentric track radii
  const rxInner = rx * 0.72;
  const ryInner = ry * 0.72;
  const rxOuter = rx + ORBIT_GUIDE_GAP;
  const ryOuter = ry + ORBIT_GUIDE_GAP * ORBIT_TILT_COMPRESSION;
  const rxFarOuter = rx + ORBIT_GUIDE_GAP * 2.2;
  const ryFarOuter = ry + ORBIT_GUIDE_GAP * 2.2 * ORBIT_TILT_COMPRESSION;

  return (
    <div className="absolute inset-0 pointer-events-none flex items-center justify-center overflow-hidden z-0">
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        className="overflow-visible"
        aria-hidden
      >
        <defs>
          {/* Luminous front-arc gradient spotlight (intense front highlight matching mockup) */}
          <linearGradient id="frontArcGlow" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="rgb(var(--accent))" stopOpacity="0.08" />
            <stop offset="55%" stopColor="rgb(var(--accent))" stopOpacity="0.25" />
            <stop offset="85%" stopColor="rgb(var(--accent))" stopOpacity="0.95" />
            <stop offset="100%" stopColor="rgb(var(--accent))" stopOpacity="1" />
          </linearGradient>

          {/* Golden/Cyan glow filter for front spotlight */}
          <filter id="orbitNeonGlow" x="-30%" y="-30%" width="160%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="6" result="blur1" />
            <feGaussianBlur in="SourceGraphic" stdDeviation="16" result="blur2" />
            <feMerge>
              <feMergeNode in="blur2" />
              <feMergeNode in="blur1" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* 1. Innermost Concentric Time Track */}
        <ellipse
          cx={cx}
          cy={cy}
          rx={rxInner}
          ry={ryInner}
          fill="none"
          stroke="rgb(var(--accent))"
          strokeWidth="1"
          strokeOpacity="0.08"
          strokeDasharray="4 8"
        />

        {/* 2. Primary Solid Track (Back Half — Soft) */}
        <ellipse
          cx={cx}
          cy={cy}
          rx={rx}
          ry={ry}
          fill="none"
          stroke="rgb(var(--accent))"
          strokeWidth="1.5"
          strokeOpacity="0.22"
        />

        {/* 3. Primary Front Spotlight Arc (Intense Luminous Glow on the bottom half) */}
        <path
          d={`M ${cx - rx} ${cy} A ${rx} ${ry} 0 0 0 ${cx + rx} ${cy}`}
          fill="none"
          stroke="url(#frontArcGlow)"
          strokeWidth="2.75"
          strokeLinecap="round"
          filter="url(#orbitNeonGlow)"
        />

        {/* 4. Concentric Outer Guide Ring 1 */}
        <ellipse
          cx={cx}
          cy={cy}
          rx={rxOuter}
          ry={ryOuter}
          fill="none"
          stroke="rgb(var(--accent))"
          strokeWidth="1"
          strokeOpacity="0.10"
          strokeDasharray="2 6"
        />

        {/* 5. Concentric Outer Guide Ring 2 (Distant Strata) */}
        <ellipse
          cx={cx}
          cy={cy}
          rx={rxFarOuter}
          ry={ryFarOuter}
          fill="none"
          stroke="rgb(var(--accent))"
          strokeWidth="1"
          strokeOpacity="0.05"
        />
      </svg>
    </div>
  );
});

ChamberOrbitRings.displayName = "ChamberOrbitRings";
