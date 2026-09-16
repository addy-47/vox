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
      const t = (now - startTime) * 0.0015; // Animation time in seconds

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
          // Simulates cellular automata / quantum dot excitation
          const w1 = Math.sin(nx * 8.5 + t * 2.2);
          const w2 = Math.cos(ny * 7.0 - t * 1.7);
          const w3 = Math.sin((nx + ny) * 9.0 + t * 3.1);
          const distFromCenter = Math.sqrt((nx - 0.5) ** 2 + (ny - 0.5) ** 2);
          const radialRipple = Math.sin(distFromCenter * 14.0 - t * 2.5);

          // Discrete pseudo-random hash for localized pixel flickering
          const cellSeed = Math.sin(c * 12.9898 + r * 78.233) * 43758.5453;
          const flicker = Math.sin(cellSeed + t * 4.0) * 0.2;

          // Composite intensity factor [-1..1] mapped to [0..1]
          let intensity = (w1 * 0.35 + w2 * 0.25 + w3 * 0.2 + radialRipple * 0.2 + flicker + 1) / 2;
          intensity = Math.max(0, Math.min(1, intensity));

          // Sharpen active clusters so dots pop distinctly rather than a uniform haze
          const alpha = Math.pow(intensity, 2.5);

          if (alpha > 0.03) {
            const radius = DOT_BASE_RADIUS + alpha * 1.25;

            ctx.beginPath();
            ctx.arc(x, y, radius, 0, Math.PI * 2);
            ctx.fillStyle = `rgba(${accentRgb}, ${alpha * 0.85})`;
            ctx.fill();

            // High-intensity core spark
            if (alpha > 0.65) {
              ctx.beginPath();
              ctx.arc(x, y, radius * 0.5, 0, Math.PI * 2);
              ctx.fillStyle = `rgba(255, 255, 255, ${(alpha - 0.65) * 2.0})`;
              ctx.fill();
            }
          }
        }
      }

      animId = requestAnimationFrame(render);
    };

    animId = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(animId);
      resizeObserver.disconnect();
    };
  }, [active]);

  return (
    <canvas
      ref={canvasRef}
      className={`w-full h-full block pointer-events-none ${className}`}
    />
  );
};
