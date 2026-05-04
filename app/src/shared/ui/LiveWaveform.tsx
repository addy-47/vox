import React, { useEffect, useRef } from "react";
import { cn } from "../lib/utils";

interface LiveWaveformProps {
  active?: boolean;
  processing?: boolean;
  amplitude?: number; // New: provided from backend telemetry
  height?: string | number;
  width?: string | number;
  className?: string;
}

// Helper for mapping values
const mapRange = (val: number, in_min: number, in_max: number, out_min: number, out_max: number) => {
  return (val - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
};

export const LiveWaveform: React.FC<LiveWaveformProps> = ({
  active = false,
  processing = false,
  amplitude = 0.04,
  height = 60,
  width = "100%",
  className,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const animationFrameRef = useRef<number>(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const render = () => {
      const rect = container.getBoundingClientRect();
      if (canvas.width !== rect.width || canvas.height !== rect.height) {
        canvas.width = rect.width;
        canvas.height = rect.height;
      }

      const width = canvas.width;
      const heightVal = canvas.height;

      ctx.clearRect(0, 0, width, heightVal);

      if (active) {
        ctx.beginPath();
        ctx.lineWidth = 3;
        ctx.strokeStyle = "#00dbe9";
        ctx.lineCap = "round";
        ctx.lineJoin = "round";

        const centerX = width / 2;
        const time = Date.now() / 1000;
        const bars = 40;
        const barWidth = (width / 2) / bars;
        
        for (let i = 0; i < bars; i++) {
          const noise = Math.sin(time * 10 + i * 0.5) * 0.1;
          const v = mapRange(amplitude + noise, 0, 1, 0.1, 1.0);
          const h = (v * heightVal * 0.8);
          const x_right = centerX + (i * barWidth);
          const x_left = centerX - (i * barWidth);
          const y = (heightVal - h) / 2;
          
          if (i === 0) ctx.moveTo(centerX, heightVal / 2);
          ctx.lineTo(x_right, y + h / 2);
          ctx.moveTo(centerX, heightVal / 2);
          ctx.lineTo(x_left, y + h / 2);
        }
        ctx.stroke();

        ctx.shadowBlur = 15;
        ctx.shadowColor = "rgba(0, 219, 233, 0.5)";
        ctx.stroke();
        ctx.shadowBlur = 0;

      } else if (processing) {
        const time = Date.now() / 1000;
        ctx.beginPath();
        ctx.lineWidth = 2;
        ctx.strokeStyle = "rgba(0, 219, 233, 0.25)";
        
        for (let x = 0; x < width; x++) {
          const distanceFromCenter = Math.abs(x - width / 2) / (width / 2);
          const envelope = Math.pow(1 - distanceFromCenter, 2);
          const y = heightVal / 2 + Math.sin(x * 0.05 + time * 5) * 15 * envelope;
          if (x === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }

      animationFrameRef.current = requestAnimationFrame(render);
    };

    render();
    return () => cancelAnimationFrame(animationFrameRef.current);
  }, [active, processing, amplitude]);

  return (
    <div 
      ref={containerRef}
      className={cn("relative overflow-hidden flex items-center justify-center", className)} 
      style={{ height, width }}
    >
      <canvas
        ref={canvasRef}
        className="w-full h-full"
      />
    </div>
  );
};
