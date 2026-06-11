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
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useInterpolatedMetric } from "@/shared/hooks/useInterpolatedMetric";
import {
  AreaChart,
  Area,
  ResponsiveContainer,
  Tooltip,
} from "recharts";

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

interface ChartPoint {
  t: number;
  cpu: number;
  ram: number;
  vad: number;
}

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

// Thin progress bar
const ResourceBar = memo(
  ({
    value,
    max,
    label,
    display,
  }: {
    value: number;
    max: number;
    label: string;
    display: string;
  }) => {
    const pct = Math.min(100, (value / max) * 100);
    return (
      <div className="space-y-1.5">
        <div className="flex justify-between items-baseline">
          <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span className="text-[13px] font-mono font-bold text-[rgb(var(--foreground))]">
            {display}
          </span>
        </div>
        <div className="h-[3px] w-full rounded-full bg-[rgba(var(--foreground),0.06)] overflow-hidden">
          <div
            className="h-full rounded-full bg-[rgb(var(--accent))] transition-all duration-700 ease-out"
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>
    );
  }
);
ResourceBar.displayName = "ResourceBar";

// Micro sparkline chart
const Sparkline = memo(
  ({ data, dataKey }: { data: ChartPoint[]; dataKey: keyof ChartPoint }) => (
    <ResponsiveContainer width="100%" height={48}>
      <AreaChart data={data} margin={{ top: 2, right: 0, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id={`sg-${dataKey}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="rgb(var(--accent))" stopOpacity={0.3} />
            <stop offset="95%" stopColor="rgb(var(--accent))" stopOpacity={0} />
          </linearGradient>
        </defs>
        <Tooltip
          contentStyle={{
            background: "rgba(var(--background), 0.95)",
            border: "1px solid rgba(var(--accent), 0.15)",
            borderRadius: "8px",
            fontSize: "10px",
            padding: "4px 8px",
          }}
          itemStyle={{ color: "rgb(var(--accent))" }}
          labelStyle={{ display: "none" }}
        />
        <Area
          type="monotone"
          dataKey={dataKey}
          stroke="rgb(var(--accent))"
          strokeWidth={1.5}
          fill={`url(#sg-${dataKey})`}
          isAnimationActive={false}
          dot={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  )
);
Sparkline.displayName = "Sparkline";

// ─── Main Component ───────────────────────────────────────────────────────────

export const MonitoringPopover: React.FC<MonitoringPopoverProps> = ({
  open,
  onClose,
  anchorRef,
}) => {
  const [history, setHistory] = useState<RuntimeSnapshot[]>([]);
  const latest = useMemo(() => history[history.length - 1] ?? null, [history]);
  const popoverRef = useRef<HTMLDivElement>(null);

  // 1Hz polling — only runs when popover is open
  useEffect(() => {
    if (!open) return;

    const poll = async () => {
      try {
        const snap = await invoke<RuntimeSnapshot>("get_runtime_snapshot");
        if (snap) {
          setHistory((prev) => {
            const next = [...prev, snap];
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

  // Smooth interpolated values for display
  const smoothCpu = useInterpolatedMetric(latest?.vox_cpu_usage ?? 0);
  const smoothRam = useInterpolatedMetric(latest?.vox_ram_mb ?? 0);

  const chartData: ChartPoint[] = useMemo(
    () =>
      history.map((s) => ({
        t: s.timestamp_ms,
        cpu: s.vox_cpu_usage,
        ram: s.vox_ram_mb,
        vad: s.vad_probability,
      })),
    [history]
  );

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
          initial={{ opacity: 0, y: 8, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 8, scale: 0.97 }}
          transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
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
            <button
              onClick={onClose}
              className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
              aria-label="Close monitor"
            >
              <X size={14} />
            </button>
          </div>

          <div className="px-4 pb-4 pt-3 space-y-4">
            {/* Engine status badges */}
            <div className="flex flex-wrap gap-1.5">
              <EngineBadge label="VAD" active={latest?.is_vad_loaded ?? false} icon={<ShieldCheck size={10} />} />
              <EngineBadge label="STT" active={latest?.is_stt_loaded ?? false} icon={<Activity size={10} />} />
              <EngineBadge label="LLM" active={latest?.is_llm_loaded ?? false} icon={<Cpu size={10} />} />
              <EngineBadge label="TTS" active={latest?.is_tts_loaded ?? false} icon={<Volume2 size={10} />} />
              {latest?.is_sleeping && (
                <EngineBadge label="Sleep" active={true} icon={<Moon size={10} />} />
              )}
            </div>

            {/* Resource bars */}
            {latest && (
              <div className="space-y-3">
                <ResourceBar
                  value={smoothCpu}
                  max={100}
                  label="VOX CPU"
                  display={`${smoothCpu.toFixed(1)}%`}
                />
                <ResourceBar
                  value={smoothRam}
                  max={latest.total_ram_mb}
                  label="VOX RAM"
                  display={`${Math.round(smoothRam)} MB`}
                />
              </div>
            )}

            {/* Latency row */}
            {latest && (
              <div className="flex gap-3 pt-1">
                {[
                  { label: "STT", val: formatLatency(latest.stt_latency_ms) },
                  { label: "TTFT", val: formatLatency(latest.ttft_ms) },
                  { label: "RTF", val: latest.tts_rtf != null ? `${latest.tts_rtf.toFixed(2)}×` : "--" },
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
            )}

            {/* Sparklines */}
            {chartData.length > 2 && (
              <div className="space-y-2 pt-1">
                {[
                  { label: "CPU", key: "cpu" as const },
                  { label: "RAM", key: "ram" as const },
                  { label: "VAD", key: "vad" as const },
                ].map(({ label, key }) => (
                  <div key={key} className="space-y-0.5">
                    <div className="flex items-center gap-1.5">
                      {key === "cpu" && <Cpu size={9} className="text-[rgb(var(--accent))]/60" />}
                      {key === "ram" && <MemoryStick size={9} className="text-[rgb(var(--accent))]/60" />}
                      {key === "vad" && <Zap size={9} className="text-[rgb(var(--accent))]/60" />}
                      <span className="text-[9px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))]/60">
                        {label}
                      </span>
                    </div>
                    <Sparkline data={chartData} dataKey={key} />
                  </div>
                ))}
              </div>
            )}

            {/* Awaiting data state */}
            {!latest && (
              <div className="flex flex-col items-center justify-center py-6 gap-2 opacity-60">
                <Activity size={20} className="text-[rgb(var(--accent))] animate-pulse" />
                <span className="text-[10px] font-bold uppercase tracking-widest">
                  Awaiting snapshot...
                </span>
              </div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
