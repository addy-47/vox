import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, X, Loader2, HardDrive, Cpu, Database, Mic } from 'lucide-react';

interface RuntimeReport {
  write_access: boolean;
  disk_space_gb: number;
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
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const check = async () => {
      try {
        await invoke('fetch_manifest');
        const r = await invoke<RuntimeReport>('get_runtime_report');
        setReport(r);
      } catch (e) {
        console.error('System check failed', e);
      } finally {
        setLoading(false);
      }
    };
    check();
  }, []);

  const allOk = report && 
    report.write_access && 
    report.disk_space_ok && 
    report.mic_access;

  if (loading) {
    return (
      <div className="text-center py-20">
        <Loader2 className="h-12 w-12 text-indigo-500 animate-spin mx-auto mb-4" />
        <p className="text-neutral-400">Scanning system configuration...</p>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-3xl font-bold text-white mb-2">System Check</h2>
        <p className="text-neutral-400">We need to ensure your device can run the Vox engine.</p>
      </div>

      <div className="grid gap-3">
        <StatusRow 
          icon={<HardDrive className="h-5 w-5" />}
          label="Disk Space"
          value={`${report?.disk_space_gb.toFixed(1)} GB Available`}
          subValue={`Requires ${report?.required_space_gb.toFixed(1)} GB`}
          ok={report?.disk_space_ok}
        />
        <StatusRow 
          icon={<Mic className="h-5 w-5" />}
          label="Microphone"
          value={report?.mic_access ? "Ready" : "Not Found"}
          ok={report?.mic_access}
        />
        <StatusRow 
          icon={<Database className="h-5 w-5" />}
          label="Permissions"
          value={report?.write_access ? "Write Access Granted" : "Access Denied"}
          ok={report?.write_access}
        />
        <StatusRow 
          icon={<Cpu className="h-5 w-5" />}
          label="Processing Power"
          value={`${report?.cpu_cores} Cores / ${report?.ram_gb.toFixed(0)} GB RAM`}
          ok={true}
        />
      </div>

      <div className="pt-4">
        {allOk ? (
          <button
            onClick={onNext}
            className="w-full py-4 bg-indigo-600 hover:bg-indigo-500 text-white font-semibold rounded-2xl transition-all active:scale-[0.98]"
          >
            All Systems Go — Continue
          </button>
        ) : (
          <div className="p-4 bg-red-500/10 border border-red-500/20 rounded-2xl text-red-400 text-center text-sm">
            Please resolve the issues above to continue.
          </div>
        )}
      </div>
    </div>
  );
};

const StatusRow = ({ icon, label, value, subValue, ok }: any) => (
  <div className="flex items-center justify-between p-4 bg-neutral-900/50 border border-neutral-800 rounded-2xl backdrop-blur-md">
    <div className="flex items-center gap-4">
      <div className={`p-2 rounded-xl ${ok ? 'bg-indigo-500/10 text-indigo-400' : 'bg-neutral-800 text-neutral-500'}`}>
        {icon}
      </div>
      <div>
        <p className="text-sm font-medium text-neutral-500">{label}</p>
        <p className="text-white">{value}</p>
        {subValue && <p className="text-xs text-neutral-600">{subValue}</p>}
      </div>
    </div>
    {ok !== undefined && (
      <div className={`h-8 w-8 rounded-full flex items-center justify-center ${ok ? 'bg-emerald-500/10 text-emerald-500' : 'bg-red-500/10 text-red-500'}`}>
        {ok ? <Check className="h-5 w-5" /> : <X className="h-5 w-5" />}
      </div>
    )}
  </div>
);
