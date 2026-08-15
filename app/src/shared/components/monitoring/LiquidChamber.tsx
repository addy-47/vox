import React, { useRef, useEffect } from "react";
import { Brain, Mic, Volume2 } from "lucide-react";
import { type RuntimeSnapshot } from "@/services/pipelineService";
import { type DynamicColors } from "./colorUtils";

interface LiquidChamberProps {
  latest: RuntimeSnapshot | null;
  colors: DynamicColors;
  isEngineLoaded: boolean;
  activeModelsCount: number;
  cpuPct: number;
  ramGb: string;
  ramPct: number;
  variants: {
    llm: string;
    tts: string;
    stt: string;
  };
  popover?: boolean;
  open?: boolean;
}

export const LiquidChamber: React.FC<LiquidChamberProps> = ({
  latest,
  colors,
  isEngineLoaded,
  activeModelsCount,
  cpuPct,
  ramGb,
  ramPct,
  variants,
  popover = false,
  open = true,
}) => {
  const chamberContainerRef = useRef<HTMLDivElement>(null);
  const chamberCanvasRef = useRef<HTMLCanvasElement>(null);

  // Persistent mutable refs for fluid physics simulation (prevents re-seeding on polling updates)
  const metricsRef = useRef({ ramPct, cpuPct });
  useEffect(() => {
    metricsRef.current = { ramPct, cpuPct };
  }, [ramPct, cpuPct]);

  const colorsRef = useRef(colors);
  useEffect(() => {
    colorsRef.current = colors;
  }, [colors]);

  // Liquid Chamber Canvas Animation Loop (Fluid Continuous Wave Physics)
  useEffect(() => {
    if (popover && !open) return;

    const canvas = chamberCanvasRef.current;
    const container = chamberContainerRef.current;
    if (!canvas || !container) return;

    let rafId: number;
    let time = 0;

    let curRamFill = Math.max(0.18, Math.min(0.85, metricsRef.current.ramPct / 100));
    let curCpuFill = Math.max(0.12, Math.min(0.75, metricsRef.current.cpuPct / 100));

    const bubbles = Array.from({ length: 22 }, () => ({
      x: Math.random(),
      y: Math.random(),
      r: Math.random() * 2.2 + 1.0,
      speed: Math.random() * 0.0025 + 0.0012,
      drift: (Math.random() - 0.5) * 0.0015,
      opacity: Math.random() * 0.45 + 0.15,
    }));

    let logicalWidth = container.clientWidth || 380;
    let logicalHeight = container.clientHeight || 280;

    const syncCanvasSize = () => {
      const rect = container.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return false;
      const dpr = window.devicePixelRatio || 1;
      logicalWidth = rect.width;
      logicalHeight = rect.height;
      if (
        canvas.width !== Math.floor(rect.width * dpr) ||
        canvas.height !== Math.floor(rect.height * dpr)
      ) {
        canvas.width = Math.floor(rect.width * dpr);
        canvas.height = Math.floor(rect.height * dpr);
      }
      return true;
    };

    const render = () => {
      time += 0.02;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const dpr = window.devicePixelRatio || 1;
      const width = logicalWidth;
      const height = logicalHeight;

      if (width <= 0 || height <= 0) {
        syncCanvasSize();
        rafId = requestAnimationFrame(render);
        return;
      }

      ctx.save();
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const curColors = colorsRef.current;
      const targetRamFill = Math.max(
        0.18,
        Math.min(0.85, metricsRef.current.ramPct / 100)
      );
      const targetCpuFill = Math.max(
        0.12,
        Math.min(0.75, metricsRef.current.cpuPct / 100)
      );

      curRamFill += (targetRamFill - curRamFill) * 0.05;
      curCpuFill += (targetCpuFill - curCpuFill) * 0.05;

      ctx.clearRect(0, 0, width, height);

      // Glass Chamber Outline
      const radius = 24;
      ctx.save();
      ctx.beginPath();
      ctx.roundRect(1.5, 1.5, width - 3, height - 3, radius);
      ctx.clip();

      // Deep space interior
      const bgGrad = ctx.createLinearGradient(0, 0, 0, height);
      bgGrad.addColorStop(0, "rgba(8, 12, 22, 0.55)");
      bgGrad.addColorStop(1, "rgba(4, 7, 14, 0.90)");
      ctx.fillStyle = bgGrad;
      ctx.fillRect(0, 0, width, height);

      // CPU Liquid Wave Fill (Harmonic Complementary)
      const cpuLevelY = height * (1 - curCpuFill * 0.65);
      ctx.beginPath();
      ctx.moveTo(0, height);
      for (let x = 0; x <= width; x += 4) {
        const wave1 = Math.sin(x * 0.015 + time * 1.2) * 8;
        const wave2 = Math.cos(x * 0.025 - time * 0.8) * 5;
        ctx.lineTo(x, cpuLevelY + wave1 + wave2);
      }
      ctx.lineTo(width, height);
      ctx.closePath();

      const cpuGrad = ctx.createLinearGradient(0, cpuLevelY - 20, 0, height);
      cpuGrad.addColorStop(0, `rgba(${curColors.complementary}, 0.45)`);
      cpuGrad.addColorStop(0.3, `rgba(${curColors.complementary}, 0.25)`);
      cpuGrad.addColorStop(1, `rgba(${curColors.complementary}, 0.05)`);
      ctx.fillStyle = cpuGrad;
      ctx.fill();

      ctx.strokeStyle = `rgba(${curColors.complementary}, 0.85)`;
      ctx.lineWidth = 1.8;
      ctx.stroke();

      // RAM Liquid Wave Fill (Primary Accent)
      const ramLevelY = height * (1 - curRamFill * 0.75);
      ctx.beginPath();
      ctx.moveTo(0, height);
      for (let x = 0; x <= width; x += 4) {
        const wave1 = Math.sin(x * 0.018 - time * 1.5) * 10;
        const wave2 = Math.cos(x * 0.032 + time * 1.1) * 6;
        ctx.lineTo(x, ramLevelY + wave1 + wave2);
      }
      ctx.lineTo(width, height);
      ctx.closePath();

      const ramGrad = ctx.createLinearGradient(0, ramLevelY - 20, 0, height);
      ramGrad.addColorStop(0, `rgba(${curColors.primary}, 0.65)`);
      ramGrad.addColorStop(0.4, `rgba(${curColors.primary}, 0.35)`);
      ramGrad.addColorStop(1, `rgba(${curColors.primary}, 0.10)`);
      ctx.fillStyle = ramGrad;
      ctx.fill();

      ctx.strokeStyle = `rgba(${curColors.primary}, 0.95)`;
      ctx.lineWidth = 2.2;
      ctx.shadowColor = `rgb(${curColors.primary})`;
      ctx.shadowBlur = 10;
      ctx.stroke();
      ctx.shadowBlur = 0;

      // Floating Bubbles
      bubbles.forEach((b) => {
        b.y -= b.speed;
        b.x += b.drift;
        if (b.y < 0) {
          b.y = 1.05;
          b.x = Math.random();
        }
        const px = b.x * width;
        const py = b.y * height;

        ctx.beginPath();
        ctx.arc(px, py, b.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(255, 255, 255, ${b.opacity})`;
        ctx.fill();
      });

      // Glass Reflections
      const innerSpec = ctx.createLinearGradient(0, 0, width, 0);
      innerSpec.addColorStop(0, "rgba(255, 255, 255, 0.18)");
      innerSpec.addColorStop(0.08, "rgba(255, 255, 255, 0.03)");
      innerSpec.addColorStop(0.92, "rgba(255, 255, 255, 0.03)");
      innerSpec.addColorStop(1, "rgba(255, 255, 255, 0.18)");
      ctx.fillStyle = innerSpec;
      ctx.fillRect(0, 0, width, height);

      // Top Glass Rim Curve
      ctx.beginPath();
      ctx.ellipse(width / 2, 20, width * 0.44, 10, 0, 0, Math.PI * 2);
      ctx.strokeStyle = "rgba(255, 255, 255, 0.22)";
      ctx.lineWidth = 1.2;
      ctx.stroke();

      // Bottom Base Glow
      const baseGlow = ctx.createRadialGradient(
        width / 2,
        height - 10,
        10,
        width / 2,
        height,
        width * 0.48
      );
      baseGlow.addColorStop(0, `rgba(${curColors.primary}, 0.50)`);
      baseGlow.addColorStop(1, "rgba(0, 0, 0, 0)");
      ctx.fillStyle = baseGlow;
      ctx.fillRect(0, height - 36, width, 36);

      ctx.restore(); // restore clip

      ctx.beginPath();
      ctx.roundRect(1, 1, width - 2, height - 2, radius);
      ctx.strokeStyle = "rgba(255, 255, 255, 0.15)";
      ctx.lineWidth = 1.2;
      ctx.stroke();

      ctx.restore(); // restore transform

      rafId = requestAnimationFrame(render);
    };

    syncCanvasSize();

    // Use ResizeObserver to continuously track layout changes and animation opens
    const resizeObserver = new ResizeObserver(() => {
      syncCanvasSize();
    });
    resizeObserver.observe(container);

    rafId = requestAnimationFrame(render);

    return () => {
      resizeObserver.disconnect();
      cancelAnimationFrame(rafId);
    };
  }, [open, popover]);

  return (
    <div
      ref={chamberContainerRef}
      className="flex-1 relative rounded-3xl overflow-hidden min-h-[320px] my-1 shadow-2xl flex flex-col items-center justify-between p-5"
    >
      {/* Background Liquid Canvas */}
      <canvas
        ref={chamberCanvasRef}
        className="absolute inset-0 w-full h-full block pointer-events-none z-0"
      />

      {/* Top Header Layer Inside Container: CPU % on Top-Left & RAM GB on Top-Right */}
      <div className="relative z-10 w-full flex items-center justify-between">
        {/* Top-Left CPU Pill */}
        <div
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-2xl bg-[rgba(var(--card),0.35)] border border-white/10 backdrop-blur-md shadow-sm text-[11px] font-mono font-bold"
          style={{ color: `rgb(${colors.complementary})` }}
        >
          <span
            style={{ backgroundColor: `rgb(${colors.complementary})` }}
            className="w-2 h-2 rounded-full inline-block"
          />
          <span>CPU {cpuPct.toFixed(1)}%</span>
        </div>

        {/* Top-Right RAM Pill */}
        <div
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-2xl bg-[rgba(var(--card),0.35)] border border-white/10 backdrop-blur-md shadow-sm text-[11px] font-mono font-bold"
          style={{ color: `rgb(${colors.primary})` }}
        >
          <span
            style={{ backgroundColor: `rgb(${colors.primary})` }}
            className="w-2 h-2 rounded-full inline-block"
          />
          <span>RAM {ramGb} GB</span>
        </div>
      </div>

      {/* Center: Futuristic Model Resident Counter */}
      <div className="relative z-10 flex flex-col items-center justify-center text-center my-auto pointer-events-none">
        <div className="flex items-baseline gap-1.5 drop-shadow-[0_0_24px_rgba(255,255,255,0.2)]">
          <span
            style={{
              color: isEngineLoaded ? `rgb(${colors.primary})` : "white",
              textShadow: isEngineLoaded
                ? `0 0 35px rgba(${colors.primary}, 0.7)`
                : "none",
            }}
            className="text-7xl font-sans font-black tracking-tighter leading-none"
          >
            {activeModelsCount}
          </span>
          <span className="text-2xl font-mono font-bold text-white/40 tracking-tight">
            / 8
          </span>
        </div>

        <div className="mt-2 flex items-center gap-2">
          <span className="text-[12.5px] font-mono font-bold tracking-[0.25em] uppercase text-white/90 drop-shadow-md">
            {activeModelsCount === 1 ? "MODEL RESIDENT" : "MODELS RESIDENT"}
          </span>
        </div>
        <span className="text-[10.5px] font-sans text-white/60 tracking-wider mt-0.5 max-w-[240px]">
          Realtime dual-frequency wave containment
        </span>
      </div>

      {/* Bottom: 3 Core Model Variant HUD Indicators (LLM, STT, TTS) - Lighter Glass Pills */}
      <div className="relative z-10 grid grid-cols-3 gap-2.5 w-full max-w-md">
        {/* LLM Variant */}
        <div
          style={{
            backgroundColor: "rgba(255, 255, 255, 0.08)",
            borderColor: latest?.is_llm_loaded
              ? `rgba(${colors.primary}, 0.65)`
              : "rgba(255, 255, 255, 0.15)",
            boxShadow: latest?.is_llm_loaded
              ? `0 0 16px rgba(${colors.primary}, 0.25), inset 0 1px 1px rgba(255, 255, 255, 0.25)`
              : "none",
          }}
          className="px-3 py-2 rounded-2xl border backdrop-blur-xl flex flex-col items-center text-center shadow-lg transition-all duration-300 hover:bg-white/[0.12]"
        >
          <div className="flex items-center gap-1.5 text-[9.5px] font-mono font-bold text-white/80 uppercase">
            <Brain size={11} style={{ color: `rgb(${colors.primary})` }} />
            <span>LLM</span>
          </div>
          <span
            style={{
              color: latest?.is_llm_loaded ? `rgb(${colors.primary})` : "white",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 drop-shadow-sm"
          >
            {variants.llm}
          </span>
        </div>

        {/* STT Variant */}
        <div
          style={{
            backgroundColor: "rgba(255, 255, 255, 0.08)",
            borderColor: latest?.is_stt_loaded
              ? `rgba(${colors.complementary}, 0.65)`
              : "rgba(255, 255, 255, 0.15)",
            boxShadow: latest?.is_stt_loaded
              ? `0 0 16px rgba(${colors.complementary}, 0.25), inset 0 1px 1px rgba(255, 255, 255, 0.25)`
              : "none",
          }}
          className="px-3 py-2 rounded-2xl border backdrop-blur-xl flex flex-col items-center text-center shadow-lg transition-all duration-300 hover:bg-white/[0.12]"
        >
          <div className="flex items-center gap-1.5 text-[9.5px] font-mono font-bold text-white/80 uppercase">
            <Mic size={11} style={{ color: `rgb(${colors.complementary})` }} />
            <span>STT</span>
          </div>
          <span
            style={{
              color: latest?.is_stt_loaded
                ? `rgb(${colors.complementary})`
                : "white",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 drop-shadow-sm"
          >
            {variants.stt}
          </span>
        </div>

        {/* TTS Variant */}
        <div
          style={{
            backgroundColor: "rgba(255, 255, 255, 0.08)",
            borderColor: latest?.is_tts_loaded
              ? `rgba(${colors.primary}, 0.65)`
              : "rgba(255, 255, 255, 0.15)",
            boxShadow: latest?.is_tts_loaded
              ? `0 0 16px rgba(${colors.primary}, 0.25), inset 0 1px 1px rgba(255, 255, 255, 0.25)`
              : "none",
          }}
          className="px-3 py-2 rounded-2xl border backdrop-blur-xl flex flex-col items-center text-center shadow-lg transition-all duration-300 hover:bg-white/[0.12]"
        >
          <div className="flex items-center gap-1.5 text-[9.5px] font-mono font-bold text-white/80 uppercase">
            <Volume2 size={11} style={{ color: `rgb(${colors.primary})` }} />
            <span>TTS</span>
          </div>
          <span
            style={{
              color: latest?.is_tts_loaded ? `rgb(${colors.primary})` : "white",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 drop-shadow-sm"
          >
            {variants.tts}
          </span>
        </div>
      </div>
    </div>
  );
};
