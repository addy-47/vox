import React, { useRef, useEffect, useState } from "react";
import { Brain, Mic, Volume2 } from "lucide-react";
import { type RuntimeSnapshot } from "@/services/pipelineService";
import { type DynamicColors } from "./colorUtils";
import { cn } from "@/shared/lib/utils";

interface LiquidChamberProps {
  latest: RuntimeSnapshot | null;
  colors: DynamicColors;
  isEngineLoaded: boolean;
  activeModelsCount: number;
  cpuPct: number;
  ramMb?: number;
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
  ramMb = 0,
  ramGb,
  ramPct,
  variants,
  popover = false,
  open = true,
}) => {
  const chamberContainerRef = useRef<HTMLDivElement>(null);
  const chamberCanvasRef = useRef<HTMLCanvasElement>(null);
  const [isLightMode, setIsLightMode] = useState(false);

  useEffect(() => {
    const checkTheme = () => {
      const theme = document.documentElement.getAttribute("data-theme");
      setIsLightMode(theme === "light");
    };
    checkTheme();

    const observer = new MutationObserver(checkTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "class"],
    });
    return () => observer.disconnect();
  }, []);

  const isLightModeRef = useRef(isLightMode);
  useEffect(() => {
    isLightModeRef.current = isLightMode;
  }, [isLightMode]);

  // Persistent mutable refs for fluid physics simulation (prevents re-seeding on polling updates)
  const metricsRef = useRef({ ramPct, cpuPct, ramMb });
  useEffect(() => {
    metricsRef.current = { ramPct, cpuPct, ramMb };
  }, [ramPct, cpuPct, ramMb]);

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

    const ctx = canvas.getContext("2d");
    let rafId = 0;
    let running = true;
    let time = 0;
    let lastFrameTime = 0;
    const targetInterval = 1000 / 30; // 30 FPS for fluid sinusoidal waves

    const initialRamMb = metricsRef.current.ramMb > 0 ? metricsRef.current.ramMb : metricsRef.current.ramPct * 81.92;
    let curRamFill = 0.12 + Math.min(1, Math.max(0, initialRamMb / 3500)) * 0.72;
    let curCpuFill = 0.08 + Math.min(1, Math.max(0, metricsRef.current.cpuPct / 100)) * 0.70;

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

    const render = (now: number) => {
      if (!running || !ctx) return;
      if (document.hidden) {
        rafId = 0;
        return;
      }

      rafId = requestAnimationFrame(render);

      const elapsed = now - lastFrameTime;
      if (elapsed < targetInterval) {
        return;
      }
      lastFrameTime = now - (elapsed % targetInterval);

      time += 0.035;

      const dpr = window.devicePixelRatio || 1;
      const width = logicalWidth;
      const height = logicalHeight;

      if (width <= 0 || height <= 0) {
        syncCanvasSize();
        return;
      }

      ctx.save();
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const curColors = colorsRef.current;

      // Real dynamic water level scaling based on Vox RAM
      // ~150MB baseline idle -> ~0.15 fill
      // ~1.5GB models loaded -> ~0.45 fill
      // ~3.5GB heavy pipeline -> ~0.84 fill
      const effectiveRamMb = metricsRef.current.ramMb > 0
        ? metricsRef.current.ramMb
        : metricsRef.current.ramPct * 81.92;
      const normalizedRamRatio = Math.min(1, Math.max(0, effectiveRamMb / 3500));
      const targetRamFill = 0.12 + normalizedRamRatio * 0.72;

      const normalizedCpuRatio = Math.min(1, Math.max(0, metricsRef.current.cpuPct / 100));
      const targetCpuFill = 0.08 + normalizedCpuRatio * 0.70;

      curRamFill += (targetRamFill - curRamFill) * 0.05;
      curCpuFill += (targetCpuFill - curCpuFill) * 0.05;

      ctx.clearRect(0, 0, width, height);

      // Glass Chamber Outline
      const radius = 24;
      ctx.save();
      ctx.beginPath();
      ctx.roundRect(1.5, 1.5, width - 3, height - 3, radius);
      ctx.clip();

      const light = isLightModeRef.current;

      // Chamber interior background (transparent frosted glass in light mode)
      const bgGrad = ctx.createLinearGradient(0, 0, 0, height);
      if (light) {
        bgGrad.addColorStop(0, "rgba(255, 255, 255, 0.35)");
        bgGrad.addColorStop(1, "rgba(255, 255, 255, 0.08)");
      } else {
        bgGrad.addColorStop(0, "rgba(8, 12, 22, 0.55)");
        bgGrad.addColorStop(1, "rgba(4, 7, 14, 0.90)");
      }
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
      if (light) {
        cpuGrad.addColorStop(0, `rgba(${curColors.complementary}, 0.35)`);
        cpuGrad.addColorStop(0.3, `rgba(${curColors.complementary}, 0.18)`);
        cpuGrad.addColorStop(1, `rgba(${curColors.complementary}, 0.04)`);
      } else {
        cpuGrad.addColorStop(0, `rgba(${curColors.complementary}, 0.45)`);
        cpuGrad.addColorStop(0.3, `rgba(${curColors.complementary}, 0.25)`);
        cpuGrad.addColorStop(1, `rgba(${curColors.complementary}, 0.05)`);
      }
      ctx.fillStyle = cpuGrad;
      ctx.fill();

      ctx.strokeStyle = light
        ? `rgba(${curColors.complementary}, 0.70)`
        : `rgba(${curColors.complementary}, 0.85)`;
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
      if (light) {
        ramGrad.addColorStop(0, `rgba(${curColors.primary}, 0.45)`);
        ramGrad.addColorStop(0.4, `rgba(${curColors.primary}, 0.22)`);
        ramGrad.addColorStop(1, `rgba(${curColors.primary}, 0.05)`);
      } else {
        ramGrad.addColorStop(0, `rgba(${curColors.primary}, 0.65)`);
        ramGrad.addColorStop(0.4, `rgba(${curColors.primary}, 0.35)`);
        ramGrad.addColorStop(1, `rgba(${curColors.primary}, 0.10)`);
      }
      ctx.fillStyle = ramGrad;
      ctx.fill();

      ctx.strokeStyle = `rgba(${curColors.primary}, 0.95)`;
      ctx.lineWidth = 2.2;
      ctx.shadowColor = `rgb(${curColors.primary})`;
      ctx.shadowBlur = light ? 4 : 10;
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
        ctx.fillStyle = light
          ? `rgba(${curColors.primary}, ${b.opacity * 0.5})`
          : `rgba(255, 255, 255, ${b.opacity})`;
        ctx.fill();
      });

      // Glass Reflections
      const innerSpec = ctx.createLinearGradient(0, 0, width, 0);
      if (light) {
        innerSpec.addColorStop(0, "rgba(255, 255, 255, 0.45)");
        innerSpec.addColorStop(0.08, "rgba(255, 255, 255, 0.05)");
        innerSpec.addColorStop(0.92, "rgba(255, 255, 255, 0.05)");
        innerSpec.addColorStop(1, "rgba(255, 255, 255, 0.45)");
      } else {
        innerSpec.addColorStop(0, "rgba(255, 255, 255, 0.18)");
        innerSpec.addColorStop(0.08, "rgba(255, 255, 255, 0.03)");
        innerSpec.addColorStop(0.92, "rgba(255, 255, 255, 0.03)");
        innerSpec.addColorStop(1, "rgba(255, 255, 255, 0.18)");
      }
      ctx.fillStyle = innerSpec;
      ctx.fillRect(0, 0, width, height);

      // Top Glass Rim Curve
      ctx.beginPath();
      ctx.ellipse(width / 2, 20, width * 0.44, 10, 0, 0, Math.PI * 2);
      ctx.strokeStyle = light
        ? "rgba(15, 23, 42, 0.08)"
        : "rgba(255, 255, 255, 0.22)";
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
      baseGlow.addColorStop(0, light ? `rgba(${curColors.primary}, 0.25)` : `rgba(${curColors.primary}, 0.50)`);
      baseGlow.addColorStop(1, "rgba(0, 0, 0, 0)");
      ctx.fillStyle = baseGlow;
      ctx.fillRect(0, height - 36, width, 36);

      ctx.restore(); // restore clip

      ctx.beginPath();
      ctx.roundRect(1, 1, width - 2, height - 2, radius);
      ctx.strokeStyle = light
        ? "rgba(15, 23, 42, 0.10)"
        : "rgba(255, 255, 255, 0.15)";
      ctx.lineWidth = 1.2;
      ctx.stroke();

      ctx.restore(); // restore transform
    };

    syncCanvasSize();

    // Use ResizeObserver to continuously track layout changes and animation opens
    const resizeObserver = new ResizeObserver(() => {
      syncCanvasSize();
    });
    resizeObserver.observe(container);

    const onVisibility = () => {
      if (document.hidden) {
        running = false;
        if (rafId) cancelAnimationFrame(rafId);
        rafId = 0;
      } else if (!running) {
        running = true;
        rafId = requestAnimationFrame(render);
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    rafId = requestAnimationFrame(render);

    return () => {
      running = false;
      document.removeEventListener("visibilitychange", onVisibility);
      resizeObserver.disconnect();
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, [open, popover]);

  return (
    <div
      ref={chamberContainerRef}
      className={cn(
        "flex-1 relative rounded-3xl overflow-hidden min-h-[320px] my-1 flex flex-col items-center justify-between p-5 transition-shadow",
        isLightMode
          ? "bg-[rgba(var(--card),0.25)] backdrop-blur-xl shadow-xl shadow-slate-200/40 border border-[rgba(var(--border),0.12)]"
          : "shadow-2xl border border-white/5"
      )}
    >
      {/* Background Liquid Canvas */}
      <canvas
        ref={chamberCanvasRef}
        className="absolute inset-0 w-full h-full block pointer-events-none z-0"
      />

      {/* Top Header Layer Inside Container: CPU % on Top-Left & RAM GB on Top-Right */}
      <div className="relative z-10 w-full flex items-center justify-between px-2">
        {/* Top-Left CPU Indicator */}
        <div
          className="flex items-center gap-2 text-[11px] font-mono font-bold select-none drop-shadow-xs"
          style={{ color: `rgb(${colors.complementary})` }}
        >
          <span
            style={{ backgroundColor: `rgb(${colors.complementary})` }}
            className="w-2 h-2 rounded-full inline-block shadow-[0_0_8px_currentColor]"
          />
          <span>CPU {cpuPct.toFixed(1)}%</span>
        </div>

        {/* Top-Right RAM Indicator */}
        <div
          className="flex items-center gap-2 text-[11px] font-mono font-bold select-none drop-shadow-xs"
          style={{ color: `rgb(${colors.primary})` }}
        >
          <span
            style={{ backgroundColor: `rgb(${colors.primary})` }}
            className="w-2 h-2 rounded-full inline-block shadow-[0_0_8px_currentColor]"
          />
          <span>RAM {ramGb} GB</span>
        </div>
      </div>

      {/* Center: Futuristic Model Resident Counter */}
      <div className="relative z-10 flex flex-col items-center justify-center text-center my-auto pointer-events-none">
        <div className="flex items-baseline gap-1.5">
          <span
            style={{
              color: isEngineLoaded ? `rgb(${colors.primary})` : "rgb(var(--foreground))",
              textShadow: isEngineLoaded
                ? `0 0 35px rgba(${colors.primary}, 0.6)`
                : "none",
            }}
            className="text-7xl font-display font-black tracking-tighter leading-none"
          >
            {activeModelsCount}
          </span>
          <span className="text-2xl font-mono font-bold text-[rgb(var(--foreground-muted))] tracking-tight">
            / 8
          </span>
        </div>

        <div className="mt-2 flex items-center gap-2">
          <span className="text-[12px] font-bold tracking-[0.25em] uppercase text-[rgb(var(--foreground))] drop-shadow-sm">
            {activeModelsCount === 1 ? "MODEL IN MEMORY" : "MODELS IN MEMORY"}
          </span>
        </div>
        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] tracking-wider mt-0.5 max-w-[240px]">
          Your computer's activity, visualized
        </span>
      </div>

      {/* Bottom: 3 Core Model Variant HUD Indicators (LLM, STT, TTS) - Lighter Glass Pills */}
      <div className="relative z-10 grid grid-cols-3 gap-2.5 w-full max-w-md">
        {/* LLM Variant */}
        <div
          style={{
            borderColor: latest?.is_llm_loaded
              ? `rgba(${colors.primary}, 0.65)`
              : "rgba(var(--border), 0.12)",
            boxShadow: latest?.is_llm_loaded
              ? `0 0 16px rgba(${colors.primary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
              : "none",
          }}
          className={cn(
            "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-all duration-300",
            isLightMode
              ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
              : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
          )}
        >
          <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
            <Brain size={11} style={{ color: `rgb(${colors.primary})` }} />
            <span>Thinking</span>
          </div>
          <span
            style={{
              color: latest?.is_llm_loaded ? `rgb(${colors.primary})` : "rgb(var(--foreground))",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
          >
            {variants.llm}
          </span>
        </div>

        {/* STT Variant */}
        <div
          style={{
            borderColor: latest?.is_stt_loaded
              ? `rgba(${colors.complementary}, 0.65)`
              : "rgba(var(--border), 0.12)",
            boxShadow: latest?.is_stt_loaded
              ? `0 0 16px rgba(${colors.complementary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
              : "none",
          }}
          className={cn(
            "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-all duration-300",
            isLightMode
              ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
              : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
          )}
        >
          <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
            <Mic size={11} style={{ color: `rgb(${colors.complementary})` }} />
            <span>Hearing</span>
          </div>
          <span
            style={{
              color: latest?.is_stt_loaded
                ? `rgb(${colors.complementary})`
                : "rgb(var(--foreground))",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
          >
            {variants.stt}
          </span>
        </div>

        {/* TTS Variant */}
        <div
          style={{
            borderColor: latest?.is_tts_loaded
              ? `rgba(${colors.primary}, 0.65)`
              : "rgba(var(--border), 0.12)",
            boxShadow: latest?.is_tts_loaded
              ? `0 0 16px rgba(${colors.primary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
              : "none",
          }}
          className={cn(
            "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-all duration-300",
            isLightMode
              ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
              : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
          )}
        >
          <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
            <Volume2 size={11} style={{ color: `rgb(${colors.primary})` }} />
            <span>Speaking</span>
          </div>
          <span
            style={{
              color: latest?.is_tts_loaded ? `rgb(${colors.primary})` : "rgb(var(--foreground))",
            }}
            className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
          >
            {variants.tts}
          </span>
        </div>
      </div>
    </div>
  );
};
