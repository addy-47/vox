import React, { useRef, useEffect } from "react";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";

interface PixelSynthesisCanvasProps {
  className?: string;
  active?: boolean;
}

/**
 * PixelSynthesisCanvas renders an organic liquid blob / gradient orb dot matrix.
 * An organic fluid orb drifts continuously across the card, causing dots directly
 * in its vicinity to swell slightly larger and illuminate with rich accent color,
 * while dots outside remain smaller in a crisp, clean baseline accent.
 */
export const PixelSynthesisCanvas: React.FC<PixelSynthesisCanvasProps> = ({
  className = "",
  active = true,
}) => {
  useMemoryTrace("PixelSynthesisCanvas");
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    if (!active) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animId: number;
    let width = (canvas.width = canvas.offsetWidth);
    let height = (canvas.height = canvas.offsetHeight);

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.width > 0 && entry.contentRect.height > 0) {
          const dpr = window.devicePixelRatio || 1;
          width = entry.contentRect.width;
          height = entry.contentRect.height;
          canvas.width = Math.round(width * dpr);
          canvas.height = Math.round(height * dpr);
          ctx.resetTransform();
          ctx.scale(dpr, dpr);
        }
      }
    });
    resizeObserver.observe(canvas);

    const getAccentRgb = (): string => {
      const val = getComputedStyle(document.documentElement)
        .getPropertyValue("--accent")
        .trim();
      return val || "124, 58, 237";
    };

    let accentRgb = getAccentRgb();
    const startTime = performance.now();

    // Dot grid configuration
    const DOT_SPACING = 15;
    const BASE_RADIUS = 1.15; // Clean resting dot size
    const PEAK_RADIUS = 3.15; // Swelled dot size inside the liquid blob

    // ── PERF: batched dot-matrix rendering ─────────────────────────────
    // Was, per frame: 1,548 beginPath+arc+fill + 1,548 template-string
    // fillStyle writes (~92,880 fill()/s at 60Hz), plus atan2+2x sin/cos
    // per dot. Now: dots are bucketed into 6 alpha bands (one beginPath +
    // one fill per band = 6 fills/frame), fillStyle strings are precomputed
    // when the accent refreshes (zero per-frame allocation), the contour
    // wobble is evaluated with multiply-add Chebyshev identities instead of
    // atan2/sin/cos per dot, and the loop is capped at 30 FPS.
    // REVERT: set APPLY_BATCHED_MATRIX = false to restore the legacy
    // per-dot loop below (kept intact for exactly this purpose).
    // ─────────────────────────────────────────────────────────────────
    const APPLY_BATCHED_MATRIX = true;
    const ALPHA_BANDS = 6;
    const MATRIX_FRAME_INTERVAL = APPLY_BATCHED_MATRIX ? 1000 / 30 : 0;

    const bucketStyles: string[] = new Array(ALPHA_BANDS).fill("");
    const refreshBucketStyles = () => {
      for (let b = 0; b < ALPHA_BANDS; b++) {
        const mid = (b + 0.5) / ALPHA_BANDS;
        bucketStyles[b] = `rgba(${accentRgb}, ${(0.22 + 0.66 * mid).toFixed(3)})`;
      }
    };
    refreshBucketStyles();

    // Reusable per-dot stores (grown only when the grid grows).
    let dotCap = 0;
    let dotX = new Float32Array(0);
    let dotY = new Float32Array(0);
    let dotR = new Float32Array(0);
    let dotBand = new Uint8Array(0);

    let lastMatrixFrame = 0;

    // Legacy per-dot renderer (kept for REVERT above).
    const renderLegacy = (
      t: number,
      orbX: number,
      orbY: number,
      baseRadius: number,
      cols: number,
      rows: number,
      offsetX: number,
      offsetY: number
    ) => {
      for (let r = 0; r < rows; r++) {
        const y = offsetY + r * DOT_SPACING;
        const dy = y - orbY;
        for (let c = 0; c < cols; c++) {
          const x = offsetX + c * DOT_SPACING;
          const dx = x - orbX;
          const dist = Math.sqrt(dx * dx + dy * dy);
          const angle = Math.atan2(dy, dx);
          const contourWobble =
            1 +
            0.12 * Math.sin(angle * 3 + t * 2.2) +
            0.08 * Math.cos(angle * 2 - t * 1.6);
          const effectiveOrbRadius = baseRadius * contourWobble;
          const normDist = Math.min(1, dist / effectiveOrbRadius);
          const proximity = (Math.cos(normDist * Math.PI) + 1) / 2;
          const influence = Math.pow(proximity, 1.5);
          const radius = BASE_RADIUS + (PEAK_RADIUS - BASE_RADIUS) * influence;
          const alpha = 0.22 + 0.66 * influence;
          ctx.beginPath();
          ctx.arc(x, y, radius, 0, Math.PI * 2);
          ctx.fillStyle = `rgba(${accentRgb}, ${alpha.toFixed(3)})`;
          ctx.fill();
        }
      }
    };

    const render = (now: number) => {
      if (document.hidden) {
        animId = 0;
        return;
      }
      animId = requestAnimationFrame(render);
      if (APPLY_BATCHED_MATRIX && now - lastMatrixFrame < MATRIX_FRAME_INTERVAL) return;
      lastMatrixFrame = now;

      const t = (now - startTime) * 0.0011; // Fluid, organic time parameter

      ctx.clearRect(0, 0, width, height);

      // Periodically refresh accent color (+ its precomputed band styles)
      if (Math.floor(t * 10) % 30 === 0) {
        accentRgb = getAccentRgb();
        if (APPLY_BATCHED_MATRIX) refreshBucketStyles();
      }

      // Dynamic wandering liquid orb position (smooth multi-harmonic continuous path)
      const orbX =
        width * (0.5 + 0.34 * Math.sin(t * 0.95) + 0.12 * Math.sin(t * 1.8 + 1.2));
      const orbY =
        height * (0.5 + 0.34 * Math.cos(t * 0.75) + 0.12 * Math.cos(t * 1.4 + 0.8));

      // Organic fluid orb influence radius with gentle breathing/wobble
      const baseRadius = Math.min(width, height) * 0.42;

      const cols = Math.ceil(width / DOT_SPACING) + 1;
      const rows = Math.ceil(height / DOT_SPACING) + 1;
      const offsetX = (width - (cols - 1) * DOT_SPACING) / 2;
      const offsetY = (height - (rows - 1) * DOT_SPACING) / 2;

      if (!APPLY_BATCHED_MATRIX) {
        renderLegacy(t, orbX, orbY, baseRadius, cols, rows, offsetX, offsetY);
        return;
      }

      // Per-frame trig factors for the Chebyshev wobble identities below.
      const sinPhi = Math.sin(t * 2.2);
      const cosPhi = Math.cos(t * 2.2);
      const sinPsi = Math.sin(t * 1.6);
      const cosPsi = Math.cos(t * 1.6);
      const radiusSpan = PEAK_RADIUS - BASE_RADIUS;

      const need = cols * rows;
      if (need > dotCap) {
        dotCap = need;
        dotX = new Float32Array(dotCap);
        dotY = new Float32Array(dotCap);
        dotR = new Float32Array(dotCap);
        dotBand = new Uint8Array(dotCap);
      }

      // Single pass: influence via multiply-add identities (no atan2/sin/cos
      // per dot), bucketed for the 6 batched fills below.
      let n = 0;
      for (let r = 0; r < rows; r++) {
        const y = offsetY + r * DOT_SPACING;
        const dy0 = y - orbY;
        for (let c = 0; c < cols; c++) {
          const x = offsetX + c * DOT_SPACING;
          const dx = x - orbX;
          const dist = Math.sqrt(dx * dx + dy0 * dy0);
          // Unit direction (cos θ, sin θ) without atan2.
          const inv = dist > 0.0001 ? 1 / dist : 0;
          const co = dx * inv;
          const si = dy0 * inv;
          // sin(3θ+φ) and cos(2θ−ψ) via Chebyshev: sin3θ = s(3−4s²),
          // cos3θ = c(4c²−3), cos2θ = 1−2s², sin2θ = 2sc.
          const sin3 = si * (3 - 4 * si * si);
          const cos3 = co * (4 * co * co - 3);
          const sin3p = sin3 * cosPhi + cos3 * sinPhi;
          const cos2 = 1 - 2 * si * si;
          const sin2 = 2 * si * co;
          const cos2m = cos2 * cosPsi + sin2 * sinPsi;
          const contourWobble = 1 + 0.12 * sin3p + 0.08 * cos2m;
          const effectiveOrbRadius = baseRadius * contourWobble;

          // Proximity factor: 1 at orb center, smoothly decreasing to 0 at edge
          const normDist = Math.min(1, dist / effectiveOrbRadius);
          const proximity = (Math.cos(normDist * Math.PI) + 1) / 2;

          // Non-linear falloff curve (x^1.5 as x*sqrt(x) — no Math.pow)
          const influence = proximity * Math.sqrt(proximity);

          dotX[n] = x;
          dotY[n] = y;
          dotR[n] = BASE_RADIUS + radiusSpan * influence;
          const band = (influence * ALPHA_BANDS) | 0;
          dotBand[n] = band > ALPHA_BANDS - 1 ? ALPHA_BANDS - 1 : band;
          n++;
        }
      }

      // 6 batched fills (one beginPath + one fill per alpha band).
      for (let b = 0; b < ALPHA_BANDS; b++) {
        ctx.beginPath();
        ctx.fillStyle = bucketStyles[b];
        for (let i = 0; i < n; i++) {
          if (dotBand[i] !== b) continue;
          ctx.moveTo(dotX[i] + dotR[i], dotY[i]);
          ctx.arc(dotX[i], dotY[i], dotR[i], 0, Math.PI * 2);
        }
        ctx.fill();
      }
    };

    animId = requestAnimationFrame(render);

    const onVisibility = () => {
      if (!document.hidden && animId === 0) {
        animId = requestAnimationFrame(render);
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      if (animId) cancelAnimationFrame(animId);
      document.removeEventListener("visibilitychange", onVisibility);
      resizeObserver.disconnect();
      if (canvas) {
        canvas.width = 1;
        canvas.height = 1;
      }
    };
  }, [active]);

  return (
    <canvas
      ref={canvasRef}
      className={`w-full h-full block pointer-events-none ${className}`}
    />
  );
};
