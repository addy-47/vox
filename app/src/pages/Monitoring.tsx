import React, {
  useState,
  useEffect,
  useRef,
  useMemo,
  useCallback,
  memo,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  Cpu,
  Volume2,
  ShieldCheck,
  Moon,
  Zap,
  MemoryStick,
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
        "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[10px] font-bold tracking-widest uppercase transition-all duration-500",
        active
          ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] shadow-[0_0_12px_rgba(var(--accent),0.1)]"
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
      <div className="space-y-2">
        <div className="flex justify-between items-baseline">
          <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span
            ref={textRef}
            className="text-[14px] font-mono font-bold text-[rgb(var(--foreground))]"
          >
            0.0%
          </span>
        </div>
        <div className="h-[4px] w-full rounded-full bg-[rgba(var(--foreground),0.06)] overflow-hidden">
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
  }
);
Sparkline.displayName = "Sparkline";

// ─── Main Page Component ──────────────────────────────────────────────────────

export const Monitoring: React.FC = () => {
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



  const cpuTextRef = useRef<HTMLSpanElement>(null);
  const cpuBarRef = useRef<HTMLDivElement>(null);
  const ramTextRef = useRef<HTMLSpanElement>(null);
  const ramBarRef = useRef<HTMLDivElement>(null);

  const latestRef = useRef<LocalSnapshot | null>(null);
  latestRef.current = latest;

  // Background Polling Loop
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

    poll();
    const id = setInterval(poll, POLL_MS);
    return () => clearInterval(id);
  }, []);

  // Direct DOM Interpolation Loop (EMA) at 60fps
  useEffect(() => {
    let curCpu = 0;
    let curRam = 0;
    let rafId = 0;

    if (latestRef.current) {
      curCpu = latestRef.current.vox_cpu_usage;
      curRam = latestRef.current.vox_ram_mb;
    }

    const tick = () => {
      const snap = latestRef.current;
      if (snap) {
        const targetCpu = snap.vox_cpu_usage;
        const targetRam = snap.vox_ram_mb;

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
      }
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, []);

  const formatLatency = useCallback((ms: number | null) => {
    if (ms === null) return "--";
    if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
    return `${ms}ms`;
  }, []);

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-transparent px-8 pt-6 z-10 select-none">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 shrink-0 border-b border-[rgba(var(--accent),0.06)]">
        <div>
          <span className="signal-text text-[13px]">Monitoring</span>
          <p className="text-[10px] text-[rgb(var(--foreground-muted))]/40 font-mono  tracking-[0.2em] mt-1">
            System Metrics
          </p>
        </div>
        <div className="flex items-center gap-3">
          {/* Force Offload / Reload button */}
          {isEngineLoaded ? (
            <button
              onClick={async () => {
                if (togglingEngine) return;
                setTogglingEngine(true);
                try {
                  await invoke("stop_engine");
                } catch (e) {
                  console.error("Failed to offload engine:", e);
                } finally {
                  setTogglingEngine(false);
                }
              }}
              disabled={togglingEngine}
              title="Force offload all models immediately from RAM"
              className={cn(
                "p-2 rounded-full border transition-all duration-300 flex items-center justify-center cursor-pointer",
                togglingEngine
                  ? "opacity-50 cursor-wait border-white/5 text-white/10 bg-white/2"
                  : "border-[rgba(239,68,68,0.35)] text-red-500 bg-red-500/10 hover:bg-red-500/20 shadow-[0_0_12px_rgba(239,68,68,0.25)]"
              )}
            >
              <Skull size={16} />
            </button>
          ) : (
            <button
              onClick={async () => {
                if (togglingEngine) return;
                setTogglingEngine(true);
                try {
                  await invoke("launch_engine");
                } catch (e) {
                  console.error("Failed to reload engine:", e);
                } finally {
                  setTogglingEngine(false);
                }
              }}
              disabled={togglingEngine}
              title="Reload default models"
              className={cn(
                "p-2 rounded-full border transition-all duration-300 flex items-center justify-center cursor-pointer",
                togglingEngine
                  ? "opacity-50 cursor-wait border-white/5 text-white/10 bg-white/2"
                  : "border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.05)] hover:bg-[rgba(var(--accent),0.15)]"
              )}
            >
              <RefreshCw size={16} className={cn(togglingEngine && "animate-spin")} />
            </button>
          )}

          <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-full border border-[rgba(var(--accent),0.12)] glass">
            <Activity size={16} className="text-[rgb(var(--accent))] animate-pulse" />
            <span className="text-[9px] font-mono tracking-widest text-[rgb(var(--accent))] uppercase">
              LIVE MONITOR
            </span>
          </div>
        </div>
      </div>

      {/* Main Content Pane */}
      <div className="flex-1 overflow-y-auto custom-scrollbar pt-6 pb-10 space-y-6 min-h-0">
        {/* Engine badges */}
        <div className="flex flex-wrap gap-1">
          <EngineBadge
            label="VAD"
            active={latest?.is_vad_loaded ?? false}
            icon={<ShieldCheck size={16} />}
          />
          <EngineBadge
            label="STT"
            active={latest?.is_stt_loaded ?? false}
            icon={<Activity size={16} />}
          />
          <EngineBadge
            label="LLM"
            active={latest?.is_llm_loaded ?? false}
            icon={<Cpu size={16} />}
          />
          <EngineBadge
            label="TTS"
            active={latest?.is_tts_loaded ?? false}
            icon={<Volume2 size={16} />}
          />
          {latest?.is_sleeping && (
            <EngineBadge label="Sleep" active={true} icon={<Moon size={16} />} />
          )}
        </div>

        {/* Resource bars */}
        <div className="space-y-4 max-w-lg pt-5 ">
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

        {/* Latency metrics */}
        <div className="grid grid-cols-3 gap-3 max-w-lg">
          {[
            { label: "STT", val: formatLatency(latest?.stt_latency_ms ?? null) },
            { label: "TTFT", val: formatLatency(latest?.ttft_ms ?? null) },
            {
              label: "RTF",
              val: latest?.tts_rtf != null ? `${latest.tts_rtf.toFixed(2)}×` : "--",
            },
          ].map((m) => (
            <div
              key={m.label}
              className="glass px-2 py-3 flex flex-col items-center gap-1"
            >
              <span className="text-[9px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]/60">
                {m.label}
              </span>
              <span className="text-[14px] font-mono font-bold text-[rgb(var(--accent))]">
                {m.val}
              </span>
            </div>
          ))}
        </div>

        {/* Live Sparkline Graphs */}
        <div className="space-y-4 max-w-xl">
          {[
            { label: "CPU History", key: "vox_cpu_usage" as const, icon: Cpu },
            { label: "RAM History", key: "vox_ram_mb" as const, icon: MemoryStick },
            { label: "VAD Probability", key: "vad_probability" as const, icon: Zap },
          ].map(({ label, key, icon: Icon }) => (
            <div key={key} className="space-y-2">
              <div className="flex items-center gap-2">
                <Icon size={16} className="text-[rgb(var(--accent))]/70" />
                <span className="text-[10px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--foreground-muted))]/70">
                  {label}
                </span>
              </div>
              <Sparkline history={history} dataKey={key} />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
