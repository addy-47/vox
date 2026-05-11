import React, { useState, useEffect, useMemo } from "react";
import { 
  Activity, 
  Cpu, 
  Clock, 
  Zap, 
  ShieldCheck,
  Volume2,
  ListRestart,
  Moon,
  Hash
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/shared/lib/utils";
import {
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  CartesianGrid
} from "recharts";

// ─── Types ───────────────────────────────────────────────────────────────────

interface RuntimeSnapshot {
  pipeline_state: string;
  current_turn_id: number;
  conversation_id: number;
  playback_active: boolean;
  tts_generating: boolean;
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
  persistence_queue_depth: number;
  dropped_persistence_events: number;
  playback_buffer_samples: number;
  playback_underruns: number;
  active_owner: string;
  active_threads: number;
  tts_rtf: number | null;
  playback_start_ms: number | null;
  persistence_writes_per_sec: number;
  is_db_healthy: boolean;
  is_llm_loaded: boolean;
  is_tts_loaded: boolean;
  is_stt_loaded: boolean;
  is_vad_loaded: boolean;
  is_sleeping: boolean;
  timestamp_ms: number;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const MAX_HISTORY_SAMPLES = 60; // ~6 seconds at 10Hz
const POLL_INTERVAL_MS = 1000;   // 1Hz

// ─── Sub-Components ───────────────────────────────────────────────────────────

const StatusBadge: React.FC<{ label: string; active: boolean; icon: React.ReactNode }> = ({ label, active, icon }) => (
  <div className={cn(
    "flex items-center gap-2 px-3 py-1.5 rounded-lg border transition-all duration-300",
    active 
      ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]" 
      : "bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.05)] text-[rgb(var(--foreground-muted))]"
  )}>
    <div className={cn("transition-transform duration-500", active && "animate-pulse")}>
      {icon}
    </div>
    <span className="text-[10px] font-bold uppercase tracking-wider">{label}</span>
  </div>
);

const MetricCard: React.FC<{ 
  title: string; 
  value: string | number; 
  unit?: string; 
  trend?: "up" | "down" | "neutral";
  icon: React.ReactNode;
}> = React.memo(({ title, value, unit, icon }) => (
  <div className="premium-card p-5 flex flex-col gap-4">
    <div className="flex items-center justify-between">
      <div className="p-2 rounded-lg bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--accent))]">
        {icon}
      </div>
      <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest">{title}</span>
    </div>
    <div className="flex items-baseline gap-1">
      <span className="text-2xl font-mono font-bold text-[rgb(var(--foreground))]">{value}</span>
      {unit && <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase">{unit}</span>}
    </div>
  </div>
));

// ─── Main Component ──────────────────────────────────────────────────────────

export const Monitoring: React.FC = () => {
  const [history, setHistory] = useState<RuntimeSnapshot[]>([]);
  const latest = useMemo(() => history[history.length - 1] || null, [history]);

  // IPC Subscription (Pull-based throttled updates)
  useEffect(() => {
    const fetchSnapshot = async () => {
      try {
        const snapshot = await invoke<RuntimeSnapshot>("get_runtime_snapshot");
        if (snapshot) {
          setHistory(prev => {
            const next = [...prev, snapshot];
            if (next.length > MAX_HISTORY_SAMPLES) {
              return next.slice(next.length - MAX_HISTORY_SAMPLES);
            }
            return next;
          });
        }
      } catch (e) {
        console.error("Failed to fetch runtime snapshot:", e);
      }
    };

    const interval = setInterval(fetchSnapshot, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, []);

  // Performance Optimization: Memoized chart data
  const cpuData = useMemo(() => history.map(s => ({ 
    time: s.timestamp_ms, 
    system: s.system_cpu_usage,
    vox: s.vox_cpu_usage
  })), [history]);
  
  const ramData = useMemo(() => history.map(s => ({ 
    time: s.timestamp_ms, 
    system: s.system_ram_mb,
    vox: s.vox_ram_mb
  })), [history]);
  const vadData = useMemo(() => history.map(s => ({ time: s.timestamp_ms, prob: s.vad_probability, energy: s.vad_energy })), [history]);

  if (!latest) {
    return (
      <div className="flex h-full w-full items-center justify-center">
        <div className="flex flex-col items-center gap-4 opacity-40">
          <Activity size={32} className="animate-pulse text-[rgb(var(--accent))]" />
          <span className="text-[11px] font-bold uppercase tracking-widest">Awaiting Runtime Snapshot...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen overflow-hidden bg-[rgb(var(--background))]">
      {/* ── Header ──────────────────────────────────────────────────────────── */}
      <header className="px-6 md:px-10 py-6 md:py-10 shrink-0">
        <div className="max-w-[1600px] mx-auto flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-xl bg-[rgb(var(--accent))]/10">
                <Activity className="text-[rgb(var(--accent))]" size={24} />
              </div>
              <h1 className="text-2xl md:text-3xl font-bold tracking-tight text-[rgb(var(--foreground))]">System Monitoring</h1>
            </div>
            <p className="text-sm text-[rgb(var(--foreground-muted))] max-w-md">Realtime runtime observability and pipeline health.</p>
          </div>

          <div className="flex flex-wrap gap-3 items-center">
            <StatusBadge label="VAD" active={latest.is_vad_loaded} icon={<ShieldCheck size={14} />} />
            <StatusBadge label="STT" active={latest.is_stt_loaded} icon={<Activity size={14} />} />
            <StatusBadge label="LLM" active={latest.is_llm_loaded} icon={<Cpu size={14} />} />
            <StatusBadge label="TTS" active={latest.is_tts_loaded} icon={<Volume2 size={14} />} />
            {latest.is_sleeping && (
              <StatusBadge label="Sleep" active={true} icon={<Moon size={14} />} />
            )}
          </div>
        </div>
      </header>

      {/* ── Main Grid ───────────────────────────────────────────────────────── */}
      <main className="flex-1 overflow-y-auto custom-scrollbar px-6 md:px-10 py-6 md:py-8">
        <div className="max-w-[1600px] mx-auto grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 pb-8">
          
          {/* Section 1: Interaction Metadata */}
          <div className="lg:col-span-4 grid grid-cols-1 md:grid-cols-3 gap-6">
             <MetricCard title="Current Turn" value={latest.current_turn_id} icon={<Hash size={18} />} />
             <MetricCard title="Session ID" value={latest.conversation_id === 0 ? "Inactive" : `#${latest.conversation_id.toString().slice(-6)}`} icon={<Clock size={18} />} />
             <MetricCard title="Threads" value={latest.active_threads} unit="Active" icon={<ListRestart size={18} />} />
          </div>

          {/* Section 2: Latency Metrics */}
          <div className="lg:col-span-2 premium-card p-6 flex flex-col gap-6">
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest">Pipeline Latency</h3>
              <Zap size={16} className="text-[rgb(var(--accent))]" />
            </div>
            <div className="grid grid-cols-3 gap-4">
              <div className="space-y-1">
                <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase">STT</span>
                <div className="text-xl font-mono font-bold">{latest.stt_latency_ms ?? "--"} <span className="text-[10px]">ms</span></div>
              </div>
              <div className="space-y-1">
                <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase">TTFT</span>
                <div className="text-xl font-mono font-bold text-[rgb(var(--accent))]">{latest.ttft_ms ?? "--"} <span className="text-[10px]">ms</span></div>
              </div>
              <div className="space-y-1">
                <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase">TTS RTF</span>
                <div className="text-xl font-mono font-bold">{latest.tts_rtf?.toFixed(2) ?? "--"} <span className="text-[10px]">x</span></div>
              </div>
            </div>
            <div className="h-32 w-full mt-4">
               {/* Latency History could go here */}
            </div>
          </div>

          {/* Section 3: System Resources */}
          <div className="lg:col-span-2 premium-card p-6 flex flex-col gap-6">
            <div className="flex items-center justify-between mb-4">
              <div className="flex flex-col gap-1">
                <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest">System Health</h3>
                <div className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))] opacity-60">
                   {latest.cpu_cores} Cores &bull; {(latest.total_ram_mb / 1024).toFixed(1)} GB Physical
                </div>
              </div>
              <div className="flex gap-4">
                <div className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
                  <span className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-tighter">Vox Engine</span>
                </div>
                <div className="flex items-center gap-1.5">
                  <div className="w-2 h-2 rounded-full bg-[rgba(var(--foreground),0.1)] border border-[rgba(var(--foreground),0.2)]" />
                  <span className="text-[9px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-tighter">Host System</span>
                </div>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-8">
              <div className="space-y-3">
                <div className="flex justify-between items-end">
                  <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-tight">CPU Load</span>
                  <div className="flex flex-col items-end">
                    <span className="text-xl font-mono font-bold leading-none">{latest.vox_cpu_usage.toFixed(1)}%</span>
                    <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))] mt-1">Total: {latest.system_cpu_usage.toFixed(1)}%</span>
                  </div>
                </div>
                <div className="h-1 w-full bg-[rgb(var(--foreground))]/[0.05] rounded-full overflow-hidden">
                   <div 
                    className="h-full bg-[rgb(var(--accent))] transition-all duration-500" 
                    style={{ width: `${latest.vox_cpu_usage}%` }} 
                  />
                </div>
              </div>
              <div className="space-y-3">
                <div className="flex justify-between items-end">
                  <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-tight">Memory</span>
                  <div className="flex flex-col items-end">
                    <span className="text-xl font-mono font-bold leading-none">{latest.vox_ram_mb} <span className="text-[10px]">MB</span></span>
                    <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))] mt-1">Total: {latest.system_ram_mb} MB</span>
                  </div>
                </div>
                <div className="h-1 w-full bg-[rgb(var(--foreground))]/[0.05] rounded-full overflow-hidden">
                   <div 
                    className="h-full bg-[rgb(var(--accent))] transition-all duration-500" 
                    style={{ width: `${(latest.vox_ram_mb / latest.total_ram_mb) * 100}%` }} 
                  />
                </div>
              </div>
            </div>
            <div className="h-40 w-full mt-6 flex gap-8">
              <div className="flex-1 h-full relative group">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={cpuData} margin={{ top: 5, right: 5, left: -25, bottom: 0 }}>
                    <defs>
                      <linearGradient id="cpuGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="rgb(var(--accent))" stopOpacity={0.2}/>
                        <stop offset="95%" stopColor="rgb(var(--accent))" stopOpacity={0}/>
                      </linearGradient>
                    </defs>
                    <CartesianGrid vertical={false} stroke="rgba(var(--foreground), 0.05)" strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis 
                      domain={[0, (dataMax: number) => Math.max(20, Math.ceil(dataMax / 10) * 10 + 10)]} 
                      tick={{fontSize: 8, fill: 'rgb(var(--foreground-muted))'}} 
                      axisLine={false} 
                      tickLine={false} 
                      tickCount={4}
                    />
                    <Tooltip 
                      contentStyle={{ 
                        backgroundColor: 'rgb(var(--background))', 
                        borderColor: 'rgba(var(--border), 0.1)',
                        borderRadius: '12px',
                        fontSize: '10px'
                      }}
                      labelStyle={{ display: 'none' }}
                    />
                    <Area 
                      type="monotone" 
                      dataKey="system" 
                      stroke="rgba(var(--foreground), 0.1)" 
                      strokeWidth={1}
                      fill="rgba(var(--foreground), 0.02)"
                      isAnimationActive={false}
                    />
                    <Area 
                      type="monotone" 
                      dataKey="vox" 
                      stroke="rgb(var(--accent))" 
                      strokeWidth={2}
                      fillOpacity={1} 
                      fill="url(#cpuGradient)" 
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
              <div className="flex-1 h-full relative group">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={ramData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                    <defs>
                      <linearGradient id="ramGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="rgb(var(--accent))" stopOpacity={0.15}/>
                        <stop offset="95%" stopColor="rgb(var(--accent))" stopOpacity={0}/>
                      </linearGradient>
                    </defs>
                    <CartesianGrid vertical={false} stroke="rgba(var(--foreground), 0.05)" strokeDasharray="3 3" />
                    <XAxis dataKey="time" hide />
                    <YAxis 
                      domain={['dataMin - 100', 'dataMax + 100']} 
                      tick={{fontSize: 8, fill: 'rgb(var(--foreground-muted))'}} 
                      axisLine={false} 
                      tickLine={false}
                      tickCount={4}
                    />
                    <Tooltip 
                      contentStyle={{ 
                        backgroundColor: 'rgb(var(--background))', 
                        borderColor: 'rgba(var(--border), 0.1)',
                        borderRadius: '12px',
                        fontSize: '10px'
                      }}
                      labelStyle={{ display: 'none' }}
                    />
                    <Area 
                      type="monotone" 
                      dataKey="system" 
                      stroke="rgba(var(--foreground), 0.1)" 
                      strokeWidth={1}
                      fill="rgba(var(--foreground), 0.02)"
                      isAnimationActive={false}
                    />
                    <Area 
                      type="monotone" 
                      dataKey="vox" 
                      stroke="rgb(var(--accent))" 
                      strokeWidth={2}
                      fillOpacity={1} 
                      fill="url(#ramGradient)" 
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>

          {/* Section 6: VAD / Audio Energy */}
          <div className="lg:col-span-4 premium-card p-6 flex flex-col gap-6">
            <div className="flex items-center justify-between">
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest">VAD / Audio Activity</h3>
              <Volume2 size={16} className="text-[rgb(var(--accent))]" />
            </div>
            <div className="h-48 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={vadData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                  <CartesianGrid vertical={false} stroke="rgba(var(--foreground), 0.05)" strokeDasharray="3 3" />
                  <XAxis dataKey="time" hide />
                  <YAxis tick={{fontSize: 8, fill: 'rgb(var(--foreground-muted))'}} axisLine={false} tickLine={false} domain={[0, 1]} tickCount={3} />
                  <Tooltip 
                    contentStyle={{ 
                      backgroundColor: 'rgb(var(--background))', 
                      borderColor: 'rgba(var(--border), 0.1)',
                      borderRadius: '12px',
                      fontSize: '10px'
                    }}
                    labelStyle={{ display: 'none' }}
                  />
                  <Area 
                    type="monotone" 
                    dataKey="prob" 
                    stroke="rgb(var(--accent))" 
                    strokeWidth={2}
                    fill="rgb(var(--accent))" 
                    fillOpacity={0.15} 
                    isAnimationActive={false}
                  />
                  <Area 
                    type="monotone" 
                    dataKey="energy" 
                    stroke="rgba(var(--foreground), 0.3)" 
                    strokeWidth={1}
                    fill="transparent" 
                    strokeDasharray="3 3"
                    isAnimationActive={false}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          </div>

        </div>
      </main>
    </div>
  );
};
