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
  models_verified: boolean;
}

interface Props {
  onNext: () => void;
}

export const SystemCheckStep: React.FC<Props> = ({ onNext }) => {
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
    <div className="flex flex-col gap-10">
      <header>
        <h2 className="text-3xl font-black text-white mb-2 tracking-tighter uppercase">Environment Audit</h2>
        <p className="text-white/80 text-sm font-light">Analyzing system environment for local neural execution.</p>
      </header>

      <div className="grid grid-cols-2 gap-4">
        <StatusCard 
          icon={<HardDrive className="w-4 h-4" />}
          label="STORAGE"
          value={report ? `${report.available_space_gb.toFixed(1)} GB Free` : "Scanning..."}
          subValue={report ? `Req: ${report.required_space_gb.toFixed(1)} GB (of ${report.total_space_gb.toFixed(0)} GB total)` : "Calculating required space..."}
          ok={report?.disk_space_ok}
          loading={!report}
        />
        <StatusCard 
          icon={<Mic className="w-4 h-4" />}
          label="AUDIO"
          value={report ? (report.mic_access ? "CONNECTED" : "NOT FOUND") : "Scanning..."}
          ok={report?.mic_access}
          loading={!report}
        />
        <StatusCard 
          icon={<Database className="w-4 h-4" />}
          label="PERMISSIONS"
          value={report ? (report.write_access ? "GRANTED" : "DENIED") : "Checking..."}
          ok={report?.write_access}
          loading={!report}
        />
        <StatusCard 
          icon={<Cpu className="w-4 h-4" />}
          label="HARDWARE"
          value={report ? `${report.cpu_cores} THREADS` : "Scanning..."}
          subValue={report ? `${report.ram_gb.toFixed(0)} GB MEMORY` : "Detecting memory..."}
          ok={true}
          loading={!report}
        />
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
      >
        {report && (
          allOk ? (
            <button
              onClick={onNext}
              className="group relative w-full py-5 bg-[#0a0a0a] border border-white/10 text-white font-bold rounded-2xl overflow-hidden transition-all hover:bg-zinc-900 active:scale-[0.98] shadow-2xl hover:shadow-[#00dbe9]/10 flex items-center justify-center gap-4"
            >
              <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
              <span className="relative z-10 uppercase tracking-[0.3em] text-[11px]">
                Proceed to Model Sync
              </span>
              <ArrowRight className="w-4 h-4 relative z-10 transition-transform group-hover:translate-x-1" />
            </button>
          ) : (
            <div className="p-5 bg-red-500/5 border border-red-500/20 rounded-2xl text-red-400 text-center text-[11px] font-bold tracking-widest uppercase">
              Constraints Detected — Resolution Required
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
