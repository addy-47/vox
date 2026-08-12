import { memo, useRef, useEffect, useCallback } from "react";
import { type RuntimeSnapshot } from "@/services/pipelineService";
import { ErrorBoundary } from "@/shared/components/common/ErrorBoundary";

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

interface SparklineProps {
  history: (RuntimeSnapshot & { localTime: number })[];
  dataKey: keyof RuntimeSnapshot;
  heightPx?: number;
}

export const Sparkline = memo(({ history, dataKey, heightPx = 64 }: SparklineProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const accentColorRef = useRef<string>("0, 219, 233");
  const dimensionsRef = useRef<{ width: number; height: number }>({ width: 0, height: 0 });

  // Cache accent CSS variable value to avoid layout thrashing via getComputedStyle in loops
  useEffect(() => {
    if (typeof window !== "undefined") {
      const val = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
      if (val) accentColorRef.current = val;
    }
  }, []);

  const drawCanvas = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    try {
      const ctx = canvas.getContext("2d", { alpha: true });
      if (!ctx) return;

      const { width, height } = dimensionsRef.current;
      if (width <= 0 || height <= 0) return;

      ctx.clearRect(0, 0, width, height);

      if (history.length < 2) return;

      const now = performance.now();
      const maxAge = MAX_SAMPLES * POLL_MS;

      const points: { x: number; y: number }[] = [];
      const values = history.map((h) => (h[dataKey] as number) || 0);
      const minVal = 0;
      const maxVal =
        dataKey === "vox_ram_mb"
          ? Math.max(...values, 100)
          : dataKey === "vox_cpu_usage"
          ? 100
          : 1.0;

      // Division-by-zero safeguard floor
      const range = (maxVal - minVal) || 1;

      for (let i = 0; i < history.length; i++) {
        const pt = history[i];
        const age = now - pt.localTime;
        if (age > maxAge) continue;

        const x = width - (age / maxAge) * width;
        const val = (pt[dataKey] as number) || 0;
        const y = height - ((val - minVal) / range) * (height - 6) - 3;
        points.push({ x, y });
      }

      if (points.length < 2) return;

      const accentVal = accentColorRef.current;
      const grad = ctx.createLinearGradient(0, 0, 0, height);
      grad.addColorStop(0, `rgba(${accentVal}, 0.22)`);
      grad.addColorStop(1, `rgba(${accentVal}, 0)`);

      ctx.beginPath();
      ctx.moveTo(points[0].x, height);
      for (let i = 0; i < points.length; i++) {
        ctx.lineTo(points[i].x, points[i].y);
      }
      ctx.lineTo(points[points.length - 1].x, height);
      ctx.closePath();
      ctx.fillStyle = grad;
      ctx.fill();

      ctx.beginPath();
      ctx.moveTo(points[0].x, points[0].y);
      for (let i = 1; i < points.length - 1; i++) {
        const xc = (points[i].x + points[i + 1].x) / 2;
        const yc = (points[i].y + points[i + 1].y) / 2;
        ctx.quadraticCurveTo(points[i].x, points[i].y, xc, yc);
      }
      ctx.lineTo(points[points.length - 1].x, points[points.length - 1].y);
      ctx.strokeStyle = `rgb(${accentVal})`;
      ctx.lineWidth = 1.8;
      ctx.stroke();
    } catch (err) {
      console.error("[Sparkline] Canvas render error:", err);
    }
  }, [history, dataKey]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const width = canvas.offsetWidth;
      const height = canvas.offsetHeight;

      if (width !== dimensionsRef.current.width || height !== dimensionsRef.current.height) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
        const ctx = canvas.getContext("2d");
        if (ctx) {
          ctx.resetTransform();
          ctx.scale(dpr, dpr);
        }
        dimensionsRef.current = { width, height };
        drawCanvas();
      }
    };

    resize();
    const observer = new ResizeObserver(() => {
      resize();
    });
    observer.observe(canvas);

    return () => {
      observer.disconnect();
    };
  }, [drawCanvas]);

  // Redraw canvas on history data update
  useEffect(() => {
    drawCanvas();
  }, [drawCanvas]);

  return (
    <ErrorBoundary name="Sparkline">
      <div style={{ height: `${heightPx}px` }} className="w-full relative rounded-xl overflow-hidden glass">
        <canvas ref={canvasRef} className="block w-full h-full" />
      </div>
    </ErrorBoundary>
  );
});

Sparkline.displayName = "Sparkline";
