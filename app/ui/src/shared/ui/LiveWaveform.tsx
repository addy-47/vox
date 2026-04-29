import React, { useEffect, useRef } from "react";
import { cn } from "../lib/utils";

interface LiveWaveformProps {
  active?: boolean;
  processing?: boolean;
  height?: string | number;
  width?: string | number;
  className?: string;
}

export const LiveWaveform: React.FC<LiveWaveformProps> = ({
  active = false,
  processing = false,
  height = 60,
  width = "100%",
  className,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const dataArrayRef = useRef<Uint8Array | null>(null);
  const animationFrameRef = useRef<number>(0);

  useEffect(() => {
    if (active && !audioContextRef.current) {
      const initAudio = async () => {
        try {
          const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
          const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
          const context = new AudioContextClass();
          const source = context.createMediaStreamSource(stream);
          const analyser = context.createAnalyser();
          analyser.fftSize = 256;
          analyser.smoothingTimeConstant = 0.8;
          source.connect(analyser);

          audioContextRef.current = context;
          analyserRef.current = analyser;
          dataArrayRef.current = new Uint8Array(analyser.frequencyBinCount);
        } catch (err) {
          console.error("Error accessing microphone:", err);
        }
      };
      initAudio();
    }

    return () => {
      if (audioContextRef.current) {
        audioContextRef.current.close();
        audioContextRef.current = null;
      }
    };
  }, [active]);

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

      if (active && analyserRef.current && dataArrayRef.current) {
        analyserRef.current.getByteFrequencyData(dataArrayRef.current as any);
        const data = dataArrayRef.current;
        
        ctx.beginPath();
        ctx.lineWidth = 3;
        ctx.strokeStyle = "#00dbe9";
        ctx.lineCap = "round";
        ctx.lineJoin = "round";

        const sliceWidth = width / data.length;
        let x = 0;

        for (let i = 0; i < data.length; i++) {
          const v = data[i] / 128.0;
          const y = (v * heightVal) / 2;

          if (i === 0) {
            ctx.moveTo(x, y);
          } else {
            ctx.lineTo(x, y);
          }

          x += sliceWidth;
        }

        ctx.stroke();

        // Add a secondary glow wave
        ctx.beginPath();
        ctx.lineWidth = 1;
        ctx.strokeStyle = "rgba(0, 219, 233, 0.3)";
        x = 0;
        for (let i = 0; i < data.length; i++) {
          const v = data[i] / 128.0;
          const y = (v * heightVal) / 2 + Math.sin(i * 0.1) * 5;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
          x += sliceWidth;
        }
        ctx.stroke();
      } else if (processing) {
        // Idle breathing wave
        const time = Date.now() / 1000;
        ctx.beginPath();
        ctx.lineWidth = 2;
        ctx.strokeStyle = "rgba(0, 219, 233, 0.2)";
        
        for (let x = 0; x < width; x++) {
          const y = heightVal / 2 + Math.sin(x * 0.05 + time * 5) * 10;
          if (x === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }

      animationFrameRef.current = requestAnimationFrame(render);
    };

    render();
    return () => cancelAnimationFrame(animationFrameRef.current);
  }, [active, processing]);

  return (
    <div 
      ref={containerRef}
      className={cn("relative overflow-hidden flex items-center justify-center", className)} 
      style={{ height, width }}
    >
      <canvas
        ref={canvasRef}
        className="w-full h-full opacity-80"
      />
    </div>
  );
};
