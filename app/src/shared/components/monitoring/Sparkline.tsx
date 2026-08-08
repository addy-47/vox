import React, { memo, useRef, useEffect } from "react";
import { type RuntimeSnapshot } from "@/services/pipelineService";

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

interface SparklineProps {
  history: (RuntimeSnapshot & { localTime: number })[];
  dataKey: keyof RuntimeSnapshot;
}

export const Sparkline: React.FC<SparklineProps> = memo(({ history, dataKey }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationRef = useRef<number>(0);
  const historyRef = useRef<(RuntimeSnapshot & { localTime: number })[]>(history);
  const dimensionsRef = useRef<{ width: number; height: number }>({ width: 0, height: 0 });

  useEffect(() => {
    historyRef.current = history;
  }, [history]);

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
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const render = () => {
      const ctx = canvas.getContext("2d", { alpha: true });
      if (!ctx) {
        animationRef.current = requestAnimationFrame(render);
        return;
      }

      const { width, height } = dimensionsRef.current;
      if (width <= 0 || height <= 0) {
        animationRef.current = requestAnimationFrame(render);
        return;
      }

      ctx.clearRect(0, 0, width, height);

      const currentHistory = historyRef.current;
      if (currentHistory.length < 2) {
        animationRef.current = requestAnimationFrame(render);
        return;
      }

      const now = performance.now();
      const maxAge = MAX_SAMPLES * POLL_MS;

      const points: { x: number; y: number }[] = [];
      const values = currentHistory.map((h) => h[dataKey] as number);
      const minVal = 0;
      const maxVal =
        dataKey === "vox_ram_mb"
          ? Math.max(...values, 100)
          : dataKey === "vox_cpu_usage"
          ? 100
          : 1.0;

      for (let i = 0; i < currentHistory.length; i++) {
        const pt = currentHistory[i];
        const age = now - pt.localTime;
        if (age > maxAge) continue;

        const x = width - (age / maxAge) * width;
        const val = pt[dataKey] as number;
        const y = height - ((val - minVal) / (maxVal - minVal)) * (height - 6) - 3;
        points.push({ x, y });
      }

      if (points.length < 2) {
        animationRef.current = requestAnimationFrame(render);
        return;
      }

      const accentVal =
        getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() ||
        "0, 219, 233";
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

      animationRef.current = requestAnimationFrame(render);
    };

    animationRef.current = requestAnimationFrame(render);
    return () => {
      cancelAnimationFrame(animationRef.current);
    };
  }, [dataKey]);

  return (
    <div className="w-full h-[64px] relative rounded-xl overflow-hidden glass">
      <canvas ref={canvasRef} className="block w-full h-full" />
    </div>
  );
});
Sparkline.displayName = "Sparkline";
