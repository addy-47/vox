import React, {
  useMemo,
  memo,
} from "react";
import { stopEngine, launchEngine } from "@/services/pipelineService";
import { useMonitoringMetrics } from "@/shared/hooks/useMonitoringMetrics";
import { Sparkline } from "@/shared/components/monitoring/Sparkline";
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
import { EngineBadge } from "@/shared/components/monitoring/EngineBadge";

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
          <span className="text-[11px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            {label}
          </span>
          <span
            ref={textRef}
            className="text-[15px] font-mono font-bold text-[rgb(var(--foreground))]"
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

// ─── Main Page Component ──────────────────────────────────────────────────────

export const Monitoring = memo(() => {
  const {
    history,
    latest,
    engineToggling: togglingEngine,
    setEngineToggling: setTogglingEngine,
    cpuTextRef,
    cpuBarRef,
    ramTextRef,
    ramBarRef,
    formatLatency,
  } = useMonitoringMetrics();

  const isEngineLoaded = useMemo(() => {
    return !!(
      latest?.is_vad_loaded ||
      latest?.is_stt_loaded ||
      latest?.is_llm_loaded ||
      latest?.is_tts_loaded
    );
  }, [latest]);

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-transparent px-8 pt-6 z-10 select-none">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 shrink-0 border-b border-[rgba(var(--accent),0.06)]">
        <div>
          <span className="signal-text text-[14px]">Monitoring</span>
          <p className="text-[11px] text-[rgb(var(--foreground-muted))]/40 font-mono  tracking-[0.2em] mt-1">
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
                  await stopEngine();
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
                  await launchEngine();
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
            <span className="text-[10px] font-mono tracking-widest text-[rgb(var(--accent))] uppercase">
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
              <span className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]/60">
                {m.label}
              </span>
              <span className="text-[15px] font-mono font-bold text-[rgb(var(--accent))]">
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
                <span className="text-[11px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--foreground-muted))]/70">
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
});
