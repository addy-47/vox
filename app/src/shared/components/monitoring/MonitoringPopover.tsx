import React, {
  useState,
  useEffect,
  useRef,
  useMemo,
  useCallback,
  memo,
} from "react";
import {
  stopEngine,
  launchEngine,
  getRuntimeSnapshot,
  type LocalSnapshot,
} from "@/services/pipelineService";
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
import { EngineBadge } from "@/shared/components/monitoring/EngineBadge";
import { Sparkline } from "@/shared/components/monitoring/Sparkline";
import { MONITORING_COPY } from "@/data/monitoringData";

interface MonitoringPopoverProps {
  open: boolean;
  onClose: () => void;
  anchorRef: React.RefObject<HTMLButtonElement | null>;
}

const MAX_SAMPLES = 60;
const POLL_MS = 1000;

export const ResourceBar = memo(
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
          <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span
            ref={textRef}
            className="text-[14px] font-mono font-bold text-[rgb(var(--foreground))]"
          >
            0.0%
          </span>
        </div>
        <div className="h-[3px] w-full rounded-full bg-[rgba(var(--foreground),0.08)] overflow-hidden">
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
  const popoverRef = useRef<HTMLDivElement>(null);

  // DOM Refs for high-performance direct DOM updates
  const cpuTextRef = useRef<HTMLSpanElement>(null);
  const cpuBarRef = useRef<HTMLDivElement>(null);
  const ramTextRef = useRef<HTMLSpanElement>(null);
  const ramBarRef = useRef<HTMLDivElement>(null);

  const latestRef = useRef<LocalSnapshot | null>(null);
  latestRef.current = latest;

  const inFlightRef = useRef(false);

  // 1Hz Background Polling Loop
  useEffect(() => {
    if (!open) return;

    const poll = async () => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      try {
        const snap = await getRuntimeSnapshot();
        if (snap) {
          setHistory((prev) => {
            const next = [...prev, { ...snap, localTime: performance.now() }];
            return next.length > MAX_SAMPLES ? next.slice(next.length - MAX_SAMPLES) : next;
          });
        }
      } catch {
        // silent
      } finally {
        inFlightRef.current = false;
      }
    };

    poll();
    const id = setInterval(poll, POLL_MS);

    return () => {
      clearInterval(id);
    };
  }, [open]);

  // Direct DOM Interpolation Loop (EMA)
  useEffect(() => {
    if (!open) return;

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
          const totalRam = snap.total_ram_mb > 0 ? snap.total_ram_mb : 8192;
          const pct = Math.min(100, Math.max(0, (curRam / totalRam) * 100));
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
          initial={{ opacity: 0, y: 12, scale: 0.98 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 12, scale: 0.98 }}
          transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1], opacity: { duration: 0.1 } }}
          className="fixed z-[200] bottom-[72px] left-4 w-[340px] max-h-[calc(100vh-96px)] overflow-y-auto custom-scrollbar glass-card no-blur"
          role="dialog"
          aria-label="System Monitoring"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 pt-4 pb-3 border-b border-[rgba(var(--accent),0.08)]">
            <div className="flex items-center gap-2">
              <Activity size={16} className="text-[rgb(var(--accent))]" />
              <span className="text-[12px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
                {MONITORING_COPY.engineMonitor}
              </span>
            </div>
            <div className="flex items-center gap-2">
              {/* Force Offload / Reload button */}
              {isEngineLoaded ? (
                <button
                  onClick={async () => {
                    if (togglingEngine) return;
                    setTogglingEngine(true);
                    try {
                      await stopEngine();
                    } catch (e) {
                      console.error("Failed to offload engine:", e);
                    } finally {
                      setTogglingEngine(false);
                    }
                  }}
                  disabled={togglingEngine}
                  title={MONITORING_COPY.forceOffloadDesc}
                  className={cn(
                    "p-1.5 rounded-lg transition-all duration-300 flex items-center justify-center cursor-pointer",
                    togglingEngine
                      ? "opacity-50 cursor-wait text-[rgb(var(--foreground-muted))]"
                      : "text-[rgb(var(--destructive))] hover:bg-[rgb(var(--destructive))]/10"
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
                      await launchEngine();
                    } catch (e) {
                      console.error("Failed to reload engine:", e);
                    } finally {
                      setTogglingEngine(false);
                    }
                  }}
                  disabled={togglingEngine}
                  title={MONITORING_COPY.reloadModelsDesc}
                  className={cn(
                    "p-1.5 rounded-lg transition-all duration-300 flex items-center justify-center cursor-pointer",
                    togglingEngine
                      ? "opacity-50 cursor-wait text-[rgb(var(--foreground-muted))]"
                      : "text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
                  )}
                >
                  <RefreshCw size={16} className={cn(togglingEngine && "animate-spin")} />
                </button>
              )}
              <button
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                aria-label="Close monitor"
              >
                <X size={16} />
              </button>
            </div>
          </div>

          <div className="px-4 pb-4 pt-3 space-y-4">
            {/* Engine status badges */}
            <div className="flex flex-wrap gap-1.5">
              <EngineBadge
                label="VAD"
                active={latest?.is_vad_loaded ?? false}
                icon={<ShieldCheck size={14} />}
              />
              <EngineBadge
                label="STT"
                active={latest?.is_stt_loaded ?? false}
                icon={<Activity size={14} />}
              />
              <EngineBadge
                label="LLM"
                active={latest?.is_llm_loaded ?? false}
                icon={<Cpu size={14} />}
              />
              <EngineBadge
                label="TTS"
                active={latest?.is_tts_loaded ?? false}
                icon={<Volume2 size={14} />}
              />
              {latest?.is_sleeping && (
                <EngineBadge label="Sleep" active={true} icon={<Moon size={14} />} />
              )}
            </div>

            {/* Resource bars */}
            <div className="space-y-3 pt-2">
              <ResourceBar
                label={MONITORING_COPY.voxCpu}
                textRef={cpuTextRef}
                barRef={cpuBarRef}
              />
              <ResourceBar
                label={MONITORING_COPY.voxRam}
                textRef={ramTextRef}
                barRef={ramBarRef}
              />
            </div>

            {/* Latency row */}
            <div className="flex gap-3 pt-1">
              {[
                { label: "STT", title: MONITORING_COPY.sttTooltip, val: formatLatency(latest?.stt_latency_ms ?? null) },
                { label: "TTFT", title: MONITORING_COPY.ttftTooltip, val: formatLatency(latest?.ttft_ms ?? null) },
                {
                  label: "RTF",
                  title: MONITORING_COPY.rtfTooltip,
                  val: latest?.tts_rtf != null ? `${latest.tts_rtf.toFixed(2)}×` : "--",
                },
              ].map((m) => (
                <div
                  key={m.label}
                  title={m.title}
                  className="flex-1 bg-[rgba(var(--foreground),0.03)] rounded-xl px-2 py-2 flex flex-col items-center gap-0.5 border border-[rgba(var(--border),0.1)] hover:border-[rgba(var(--accent),0.2)] transition-colors cursor-help"
                >
                  <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
                    {m.label}
                  </span>
                  <span className="text-[14px] font-mono font-bold text-[rgb(var(--accent))]">
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
                      <Cpu size={12} className="text-[rgb(var(--accent))]" />
                    )}
                    {key === "vox_ram_mb" && (
                      <MemoryStick size={12} className="text-[rgb(var(--accent))]" />
                    )}
                    {key === "vad_probability" && (
                      <Zap size={12} className="text-[rgb(var(--accent))]" />
                    )}
                    <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))]">
                      {label}
                    </span>
                  </div>
                  <Sparkline history={history} dataKey={key} heightPx={48} />
                </div>
              ))}
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
