import React, { useEffect, useState } from 'react';
import { Check, X, HardDrive, Cpu, Database, Mic, ArrowRight } from 'lucide-react';
import { motion } from 'framer-motion';
import { cn } from '@/shared/lib/utils';
import { invoke } from '@tauri-apps/api/core';

interface RuntimeReport {
  write_access: boolean;
  available_space_gb: number;
  total_space_gb: number;
  required_space_gb: number;
  disk_space_ok: boolean;
  mic_access: boolean;
  ram_gb: number;
  cpu_cores: number;
  models_dir_exists: boolean;
  models_dir: string;
  models_missing: string[];
  models_verified: boolean;
}

interface Props {
  onNext: () => void;
  onBack: () => void;
  error?: string;
}

export const SystemCheckStep: React.FC<Props> = ({ onNext, onBack, error }) => {
  const [report, setReport] = useState<RuntimeReport | null>(null);

  useEffect(() => {
    const check = async () => {
      try {
        await invoke('fetch_manifest');
        const r = await invoke<RuntimeReport>('get_runtime_report');
        setReport(r);
      } catch (e) {
        console.error('System check failed', e);
      }
    };
    check();
  }, []);

  const allOk = report && 
    report.write_access && 
    report.disk_space_ok && 
    report.mic_access;

  return (
    <div className="flex flex-col h-full relative">
      <header className="mb-8">
        <div className="flex items-center gap-4 mb-4">
          <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
          <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Step 2.0 • Infrastructure</span>
        </div>
        <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Environment Scan</h1>
        <p className="text-white/40 text-sm leading-relaxed max-w-md">
            Analyzing system environment for local neural execution. We ensure your hardware meets the requirements for a seamless experience.
        </p>
      </header>

      <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
        <div className="grid grid-cols-2 gap-4">
          <StatusCard 
            icon={<HardDrive className="w-4 h-4" />}
            label="STORAGE"
            value={report ? `${report.available_space_gb.toFixed(1)} GB Available` : "Scanning..."}
            subValue={report ? `System Total: ${report.total_space_gb.toFixed(1)} GB | Required: ${report.required_space_gb.toFixed(1)} GB` : "Calculating requirements..."}
            ok={report?.disk_space_ok}
            loading={!report}
          />
          <StatusCard 
            icon={<Mic className="w-4 h-4" />}
            label="AUDIO"
            value={report ? (report.mic_access ? "CONNECTED" : "NOT FOUND") : "Scanning..."}
            subValue={report ? (report.mic_access ? "Input device detected" : "Check permissions") : "Scanning hardware..."}
            ok={report?.mic_access}
            loading={!report}
          />
          <StatusCard 
            icon={<Database className="w-4 h-4" />}
            label="PERMISSIONS"
            value={report ? (report.write_access ? "GRANTED" : "DENIED") : "Checking..."}
            subValue={report ? (report.write_access ? "Sandbox I/O verified" : "Check folder access") : "Verifying access..."}
            ok={report?.write_access}
            loading={!report}
          />
          <StatusCard 
            icon={<Cpu className="w-4 h-4" />}
            label="HARDWARE"
            value={report ? `${report.cpu_cores} THREADS` : "Scanning..."}
            subValue={report ? `${report.ram_gb.toFixed(1)} GB RAM detected` : "Detecting memory..."}
            ok={true}
            loading={!report}
          />
        </div>

        {error && (
          <div className="mt-6 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center gap-3">
            <X className="w-4 h-4 text-red-500" />
            <p className="text-[11px] text-red-400 font-medium uppercase tracking-wider">{error}</p>
          </div>
        )}
      </div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
        className="mt-8 pt-8 border-t border-white/5 flex gap-4"
      >
        <button
          onClick={onBack}
          className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors"
        >
          Back
        </button>

        {report && (
          allOk ? (
            <button
              onClick={onNext}
              className="group relative flex-1 py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
            >
              <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
              <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
                Proceed to Model Sync
                <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />
              </span>
            </button>
          ) : (
            <div className="flex-1 py-5 bg-red-500/5 border border-red-500/20 rounded-2xl text-red-400 text-center text-[11px] font-black tracking-[0.4em] uppercase flex items-center justify-center opacity-50">
              Constraints Detected
            </div>
          )
        )}
      </motion.div>
    </div>
  );
};

const StatusCard = ({ icon, label, value, subValue, ok, loading }: any) => (
  <div className={cn(
    "p-5 bg-white/[0.02] border border-white/5 rounded-2xl group transition-all hover:bg-white/[0.04] hover:border-white/10",
    loading && "animate-pulse"
  )}>
    <div className="flex items-center justify-between mb-4">
      <div className={cn(
        "w-8 h-8 rounded-lg flex items-center justify-center transition-all",
        loading ? 'bg-white/5 text-white/20' : 
        ok ? 'bg-[#00dbe9]/10 text-[#00dbe9]' : 'bg-red-500/10 text-red-400'
      )}>
        {icon}
      </div>
      {!loading && (
        <div className={cn(
          "w-5 h-5 rounded-full flex items-center justify-center border",
          ok ? 'bg-[#00dbe9]/10 border-[#00dbe9]/20 text-[#00dbe9]' : 'bg-red-500/10 border-red-500/20 text-red-400'
        )}>
          {ok ? <Check className="w-2.5 h-2.5" /> : <X className="w-2.5 h-2.5" />}
        </div>
      )}
    </div>
    <div className="text-[11px] font-bold text-white/80 tracking-widest uppercase mb-1">{label}</div>
    <div className={cn(
      "text-sm font-medium leading-none mb-1",
      loading ? "text-white/20" : "text-white"
    )}>{value}</div>
    {subValue && (
      <div className={cn(
        "text-[11px] font-medium",
        loading ? "text-white/10" : "text-white/50"
      )}>{subValue}</div>
    )}
  </div>
);
