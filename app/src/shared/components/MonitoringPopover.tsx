import React, {
  useState,
  useEffect,
  useRef,
  useMemo,
  useCallback,
  memo,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import {
  Activity,
  Cpu,
  Volume2,
  ShieldCheck,
  Moon,
  Zap,
  MemoryStick,
  X,
  Skull,
  RefreshCw,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";

// ─── Types ────────────────────────────────────────────────────────────────────

interface RuntimeSnapshot {
  pipeline_state: string;
  system_cpu_usage: number;
  system_ram_mb: number;
  vox_cpu_usage: number;
  vox_ram_mb: number;
  total_ram_mb: number;
  cpu_cores: number;
  vad_energy: number;
  vad_probability: number;
  stt_latency_ms: number | null;
  ttft_ms: number | null;
  total_voice_latency_ms: number | null;
  tts_rtf: number | null;
  is_db_healthy: boolean;
  is_llm_loaded: boolean;
  is_tts_loaded: boolean;
  is_stt_loaded: boolean;
  is_vad_loaded: boolean;
  is_sleeping: boolean;
  timestamp_ms: number;
}

type LocalSnapshot = RuntimeSnapshot & { localTime: number };

interface MonitoringPopoverProps {
  open: boolean;
  onClose: () => void;
  anchorRef: React.RefObject<HTMLButtonElement | null>;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

// ─── Sub-components ───────────────────────────────────────────────────────────

const EngineBadge = memo(
  ({
    label,
    active,
    icon,
  }: {
    label: string;
    active: boolean;
    icon: React.ReactNode;
  }) => (
    <div
      className={cn(
        "flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-bold tracking-widest uppercase transition-all duration-500",
        active
          ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)]"
          : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.06)]"
      )}
    >
      <span className={cn("transition-transform duration-500", active && "scale-110")}>
        {icon}
      </span>
      {label}
    </div>
  )
);
EngineBadge.displayName = "EngineBadge";

// High-performance progress bar using direct DOM refs to avoid React re-renders
const ResourceBar = memo(
  ({
    label,
    textRef,
    barRef,
  }: {
    label: string;
    textRef: React.RefObject<HTMLSpanElement | null>;
    barRef: React.RefObject<HTMLDivElement | null>;
  }) => {
    return (
      <div className="space-y-1.5">
        <div className="flex justify-between items-baseline">
          <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span
            ref={textRef}
            className="text-[13px] font-mono font-bold text-[rgb(var(--foreground))]"
          >
            0.0%
          </span>
        </div>
        <div className="h-[3px] w-full rounded-full bg-[rgba(var(--foreground),0.06)] overflow-hidden">
          <div
            ref={barRef}
            className="h-full rounded-full bg-[rgb(var(--accent))]"
            style={{ width: "0%" }}
          />
        </div>
      </div>
    );
  }
);
ResourceBar.displayName = "ResourceBar";

// High-performance Canvas Sparkline scrolling smoothly at 60fps
const Sparkline = memo(
  ({
    history,
    dataKey,
  }: {
    history: LocalSnapshot[];
    dataKey: keyof RuntimeSnapshot;
  }) => {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const animationRef = useRef<number>(0);
    const historyRef = useRef<LocalSnapshot[]>(history);
    const dimensionsRef = useRef<{ width: number; height: number }>({ width: 0, height: 0 });

    // Keep history ref updated
    useEffect(() => {
      historyRef.current = history;
    }, [history]);

    // Handle canvas resizing only when size actually changes
    useEffect(() => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const resize = () => {
        const dpr = window.devicePixelRatio || 1;
        const width = canvas.offsetWidth;
        const height = canvas.offsetHeight;
        
        // Only resize canvas buffer if dimensions actually changed
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

      // Run initially
      resize();

      // Set up ResizeObserver
      const observer = new ResizeObserver(() => {
        resize();
      });
      observer.observe(canvas);

      return () => {
        observer.disconnect();
      };
    }, []);

    // 60fps render loop
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
        if (width === 0 || height === 0) {
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
        const maxAge = MAX_SAMPLES * POLL_MS; // 60s history window

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

          // x scales smoothly from left to right based on time age
          const x = width - (age / maxAge) * width;
          const val = pt[dataKey] as number;
          const y = height - ((val - minVal) / (maxVal - minVal)) * (height - 6) - 3;
          points.push({ x, y });
        }

        if (points.length < 2) {
          animationRef.current = requestAnimationFrame(render);
          return;
        }

        // Draw area gradient under the curve
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

        // Draw smooth neon line
        ctx.beginPath();
        ctx.moveTo(points[0].x, points[0].y);
        for (let i = 1; i < points.length - 1; i++) {
          const xc = (points[i].x + points[i + 1].x) / 2;
          const yc = (points[i].y + points[i + 1].y) / 2;
          ctx.quadraticCurveTo(points[i].x, points[i].y, xc, yc);
        }
        ctx.lineTo(points[points.length - 1].x, points[points.length - 1].y);
        ctx.strokeStyle = `rgb(${accentVal})`;
        ctx.lineWidth = 1.5;
        ctx.stroke();

        animationRef.current = requestAnimationFrame(render);
      };

      animationRef.current = requestAnimationFrame(render);

      return () => {
        cancelAnimationFrame(animationRef.current);
      };
    }, [dataKey]);

    return (
      <div className="w-full h-[48px] relative rounded-lg overflow-hidden border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]">
        <canvas ref={canvasRef} className="block w-full h-full" />
      </div>
    );
  }
);
Sparkline.displayName = "Sparkline";

// ─── Main Component ───────────────────────────────────────────────────────────

export const MonitoringPopover: React.FC<MonitoringPopoverProps> = ({
  open,
  onClose,
  anchorRef,
}) => {
  const [history, setHistory] = useState<LocalSnapshot[]>([]);
  const latest = useMemo(() => history[history.length - 1] ?? null, [history]);

  const isEngineLoaded = useMemo(() => {
    return !!(
      latest?.is_vad_loaded ||
      latest?.is_stt_loaded ||
      latest?.is_llm_loaded ||
      latest?.is_tts_loaded
    );
  }, [latest]);

  const [togglingEngine, setTogglingEngine] = useState(false);

  const handleToggleEngine = useCallback(async () => {
    if (togglingEngine) return;
    setTogglingEngine(true);
    try {
      if (isEngineLoaded) {
        await invoke("stop_engine");
      } else {
        await invoke("launch_engine");
      }
    } catch (e) {
      console.error("Failed to toggle engine:", e);
    } finally {
      setTogglingEngine(false);
    }
  }, [isEngineLoaded, togglingEngine]);

  const popoverRef = useRef<HTMLDivElement>(null);

  // DOM Refs for high-performance direct DOM updates (avoiding React re-renders at 60fps)
  const cpuTextRef = useRef<HTMLSpanElement>(null);
  const cpuBarRef = useRef<HTMLDivElement>(null);
  const ramTextRef = useRef<HTMLSpanElement>(null);
  const ramBarRef = useRef<HTMLDivElement>(null);

  const latestRef = useRef<LocalSnapshot | null>(null);
  latestRef.current = latest;

  // 1Hz Background Polling Loop - runs continuously to keep history populated and fresh
  useEffect(() => {
    const poll = async () => {
      try {
        const snap = await invoke<RuntimeSnapshot>("get_runtime_snapshot");
        if (snap) {
          setHistory((prev) => {
            const next = [...prev, { ...snap, localTime: performance.now() }];
            return next.length > MAX_SAMPLES ? next.slice(next.length - MAX_SAMPLES) : next;
          });
        }
      } catch {
        // silent
      }
    };

    poll(); // poll immediately on startup
    const id = setInterval(poll, POLL_MS);

    return () => {
      clearInterval(id);
    };
  }, []);

  // Direct DOM Interpolation Loop (EMA) running at 60fps - only active when popover is open
  useEffect(() => {
    if (!open) return;

    let curCpu = 0;
    let curRam = 0;
    let rafId = 0;

    // Seed initial values if available
    if (latestRef.current) {
      curCpu = latestRef.current.vox_cpu_usage;
      curRam = latestRef.current.vox_ram_mb;
    }

    const tick = () => {
      const snap = latestRef.current;
      if (snap) {
        const targetCpu = snap.vox_cpu_usage;
        const targetRam = snap.vox_ram_mb;

        // Exponential Moving Average
        curCpu += (targetCpu - curCpu) * 0.12;
        curRam += (targetRam - curRam) * 0.12;

        if (cpuTextRef.current) {
          cpuTextRef.current.textContent = `${curCpu.toFixed(1)}%`;
        }
        if (cpuBarRef.current) {
          cpuBarRef.current.style.width = `${Math.min(100, Math.max(0, curCpu))}%`;
        }
        if (ramTextRef.current) {
          const ramGb = curRam / 1024;
          ramTextRef.current.textContent = `${ramGb.toFixed(2)} GB`;
        }
        if (ramBarRef.current) {
          const pct = Math.min(100, Math.max(0, (curRam / snap.total_ram_mb) * 100));
          ramBarRef.current.style.width = `${pct}%`;
        }
      } else {
        if (cpuTextRef.current) cpuTextRef.current.textContent = "0.0%";
        if (cpuBarRef.current) cpuBarRef.current.style.width = "0%";
        if (ramTextRef.current) ramTextRef.current.textContent = "0.00 GB";
        if (ramBarRef.current) ramBarRef.current.style.width = "0%";
      }

      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(rafId);
    };
  }, [open]);

  // Close on click-outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node) &&
        anchorRef.current &&
        !anchorRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open, onClose, anchorRef]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  const formatLatency = useCallback((ms: number | null) => {
    if (ms === null) return "--";
    if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
    return `${ms}ms`;
  }, []);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={popoverRef}
          initial={{ y: 12, scale: 0.98 }}
          animate={{ y: 0, scale: 1 }}
          exit={{ y: 12, scale: 0.98 }}
          transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
          className="fixed z-[200] bottom-[72px] left-4 w-[340px] glass-elevated glass-base rounded-2xl overflow-hidden shadow-[0_24px_64px_rgba(0,0,0,0.5)]"
          role="dialog"
          aria-label="System Monitoring"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 pt-4 pb-3 border-b border-[rgba(var(--accent),0.08)]">
            <div className="flex items-center gap-2">
              <Activity size={14} className="text-[rgb(var(--accent))]" />
              <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
                Engine Monitor
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={handleToggleEngine}
                disabled={togglingEngine}
                title={isEngineLoaded ? "Offload all models immediately from RAM" : "Load default models"}
                className={cn(
                  "p-1 rounded-lg transition-all duration-300 flex items-center justify-center cursor-pointer",
                  togglingEngine && "opacity-50 cursor-wait",
                  isEngineLoaded
                    ? "text-red-500 hover:bg-red-500/10"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-white/5"
                )}
              >
                {isEngineLoaded ? (
                  <Skull size={13} className={cn(togglingEngine && "animate-spin")} />
                ) : (
                  <RefreshCw size={13} className={cn(togglingEngine && "animate-spin")} />
                )}
              </button>
              <button
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
                aria-label="Close monitor"
              >
                <X size={14} />
              </button>
            </div>
          </div>

          <div className="px-4 pb-4 pt-3 space-y-4">
            {/* Engine status badges */}
            <div className="flex flex-wrap ">
              <EngineBadge
                label="VAD"
                active={latest?.is_vad_loaded ?? false}
                icon={<ShieldCheck size={10} />}
              />
              <EngineBadge
                label="STT"
                active={latest?.is_stt_loaded ?? false}
                icon={<Activity size={10} />}
              />
              <EngineBadge
                label="LLM"
                active={latest?.is_llm_loaded ?? false}
                icon={<Cpu size={10} />}
              />
              <EngineBadge
                label="TTS"
                active={latest?.is_tts_loaded ?? false}
                icon={<Volume2 size={10} />}
              />
              {latest?.is_sleeping && (
                <EngineBadge label="Sleep" active={true} icon={<Moon size={10} />} />
              )}
            </div>

            {/* Resource bars */}
            <div className="space-y-3 pt-5">
              <ResourceBar
                label="VOX CPU"
                textRef={cpuTextRef}
                barRef={cpuBarRef}
              />
              <ResourceBar
                label="VOX RAM"
                textRef={ramTextRef}
                barRef={ramBarRef}
              />
            </div>

            {/* Latency row */}
            <div className="flex gap-3 pt-1">
              {[
                { label: "STT", val: formatLatency(latest?.stt_latency_ms ?? null) },
                { label: "TTFT", val: formatLatency(latest?.ttft_ms ?? null) },
                {
                  label: "RTF",
                  val:
                    latest?.tts_rtf != null ? `${latest.tts_rtf.toFixed(2)}×` : "--",
                },
              ].map((m) => (
                <div
                  key={m.label}
                  className="flex-1 bg-[rgba(var(--foreground),0.03)] rounded-xl px-2 py-2 flex flex-col items-center gap-0.5"
                >
                  <span className="text-[9px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
                    {m.label}
                  </span>
                  <span className="text-[13px] font-mono font-bold text-[rgb(var(--accent))]">
                    {m.val}
                  </span>
                </div>
              ))}
            </div>

            {/* Sparklines */}
            <div className="space-y-2 pt-1">
              {[
                { label: "CPU", key: "vox_cpu_usage" as const },
                { label: "RAM", key: "vox_ram_mb" as const },
                { label: "VAD", key: "vad_probability" as const },
              ].map(({ label, key }) => (
                <div key={key} className="space-y-1">
                  <div className="flex items-center gap-1.5">
                    {key === "vox_cpu_usage" && (
                      <Cpu size={9} className="text-[rgb(var(--accent))]/60" />
                    )}
                    {key === "vox_ram_mb" && (
                      <MemoryStick size={9} className="text-[rgb(var(--accent))]/60" />
                    )}
                    {key === "vad_probability" && (
                      <Zap size={9} className="text-[rgb(var(--accent))]/60" />
                    )}
                    <span className="text-[9px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))]/60">
                      {label}
                    </span>
                  </div>
                  <Sparkline history={history} dataKey={key} />
                </div>
              ))}
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
