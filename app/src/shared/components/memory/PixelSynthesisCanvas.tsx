import React, { useRef, useEffect } from "react";

interface PixelSynthesisCanvasProps {
  className?: string;
  active?: boolean;
}

/**
 * PixelSynthesisCanvas renders an organic computational dot/pixel matrix.
 * Rather than a generic linear loading bar, dots appear, brighten, fade,
 * and regenerate in dynamic clusters via multi-frequency spatial wave interference.
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
          canvas.width = width * dpr;
          canvas.height = height * dpr;
          ctx.resetTransform();
          ctx.scale(dpr, dpr);
        }
      }
    });
    resizeObserver.observe(canvas);

    // Dynamic extraction of CSS variable --accent (fallback to vox electric cyan 0, 229, 255)
    const getAccentRgb = (): string => {
      const val = getComputedStyle(document.documentElement)
        .getPropertyValue("--accent")
        .trim();
      return val || "0, 229, 255";
    };

    let accentRgb = getAccentRgb();
    let startTime = performance.now();

    // Dot grid configuration
    const DOT_SPACING = 12; // Distance between points in pixels
    const DOT_BASE_RADIUS = 1.25;

    const render = (now: number) => {
      const t = (now - startTime) * 0.00085; // Slower, calmer wave rhythm

      ctx.clearRect(0, 0, width, height);

      // Re-query accent occasionally in case theme toggles
      if (Math.floor(t * 10) % 20 === 0) {
        accentRgb = getAccentRgb();
      }

      const cols = Math.floor(width / DOT_SPACING);
      const rows = Math.floor(height / DOT_SPACING);
      const offsetX = (width - cols * DOT_SPACING) / 2;
      const offsetY = (height - rows * DOT_SPACING) / 2;

      for (let r = 0; r < rows; r++) {
        const y = offsetY + r * DOT_SPACING;
        for (let c = 0; c < cols; c++) {
          const x = offsetX + c * DOT_SPACING;

          // Normalized spatial coordinates
          const nx = x / width;
          const ny = y / height;

          // Multi-frequency organic wave interference
          const w1 = Math.sin(nx * 6.0 + t * 1.5);
          const w2 = Math.cos(ny * 5.0 - t * 1.2);
          const w3 = Math.sin((nx + ny) * 5.5 + t * 1.8);
          const distFromCenter = Math.sqrt((nx - 0.5) ** 2 + (ny - 0.5) ** 2);
          const radialRipple = Math.sin(distFromCenter * 10.0 - t * 1.6);

          // Smooth composite intensity without random jitter/flicker
          let intensity = (w1 * 0.35 + w2 * 0.3 + w3 * 0.2 + radialRipple * 0.15 + 1) / 2;
          intensity = Math.max(0, Math.min(1, intensity));

          // Soft organic curve
          const alpha = Math.pow(intensity, 2.2);

          if (alpha > 0.04) {
            const radius = DOT_BASE_RADIUS + alpha * 1.1;

            ctx.beginPath();
            ctx.arc(x, y, radius, 0, Math.PI * 2);
            ctx.fillStyle = `rgba(${accentRgb}, ${alpha * 0.8})`;
            ctx.fill();
          }
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
