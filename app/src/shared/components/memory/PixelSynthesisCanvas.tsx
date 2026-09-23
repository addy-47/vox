import React, { useRef, useEffect } from "react";

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

    const render = (now: number) => {
      const t = (now - startTime) * 0.0011; // Fluid, organic time parameter

      ctx.clearRect(0, 0, width, height);

      // Periodically refresh accent color
      if (Math.floor(t * 10) % 30 === 0) {
        accentRgb = getAccentRgb();
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

      for (let r = 0; r < rows; r++) {
        const y = offsetY + r * DOT_SPACING;
        const dy = y - orbY;

        for (let c = 0; c < cols; c++) {
          const x = offsetX + c * DOT_SPACING;
          const dx = x - orbX;
          const dist = Math.sqrt(dx * dx + dy * dy);

          // Subtle organic blob contour wobble based on angle
          const angle = Math.atan2(dy, dx);
          const contourWobble =
            1 +
            0.12 * Math.sin(angle * 3 + t * 2.2) +
            0.08 * Math.cos(angle * 2 - t * 1.6);
          const effectiveOrbRadius = baseRadius * contourWobble;

          // Proximity factor: 1 at orb center, smoothly decreasing to 0 at edge
          const normDist = Math.min(1, dist / effectiveOrbRadius);
          const proximity = (Math.cos(normDist * Math.PI) + 1) / 2;

          // Non-linear falloff curve
          const influence = Math.pow(proximity, 1.5);

          // Dot size scales smoothly with orb proximity
          const radius = BASE_RADIUS + (PEAK_RADIUS - BASE_RADIUS) * influence;

          // Rich, colored accent values:
          // Resting baseline has rich, visible color (0.22 opacity)
          // Inside the liquid orb, swells to deep saturated accent (up to 0.88 opacity)
          const alpha = 0.22 + 0.66 * influence;

          ctx.beginPath();
          ctx.arc(x, y, radius, 0, Math.PI * 2);
          ctx.fillStyle = `rgba(${accentRgb}, ${alpha.toFixed(3)})`;
          ctx.fill();
        }
      }

      animId = requestAnimationFrame(render);
    };

    animId = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(animId);
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
