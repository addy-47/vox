import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion } from 'framer-motion';
import { Mic, Check, Volume2, ArrowRight, Activity } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface AudioDevice {
  name: string;
  is_default: boolean;
}

interface Props {
  onNext: () => void;
  onBack: () => void;
}

export const AudioSetupStep: React.FC<Props> = ({ onNext, onBack }) => {
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selected, setSelected] = useState<string>('');
  const [energy, setEnergy] = useState(0);

  useEffect(() => {
    const load = async () => {
      try {
        const devList = await invoke<AudioDevice[]>('list_input_devices');
        setDevices(devList);
        const def = devList.find(d => d.is_default);
        if (def) setSelected(def.name);
      } catch (e) {
        console.error('Failed to list devices', e);
      }
    };
    load();

    const unlisten = listen<number>('audio_energy', (event) => {
      setEnergy(event.payload * 100);
    });

    return () => {
      unlisten.then(u => u());
    };
  }, []);

  return (
    <div className="flex flex-col h-full relative">
      <header className="mb-8">
        <div className="flex items-center gap-4 mb-4">
          <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
          <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Step 3.0 • Audio Input</span>
        </div>
        <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Device Selection</h1>
        <p className="text-white/40 text-sm leading-relaxed max-w-md">
            Configuring audio input for real-time interaction. Select your primary microphone to enable voice understanding.
        </p>
      </header>
 
      <div className="flex-1 flex flex-col gap-6 min-h-0 overflow-y-auto pr-2 custom-scrollbar">
        {/* Live Analysis Card */}
        <div className="p-5 bg-white/[0.02] border border-white/10 rounded-2xl relative overflow-hidden group">
            <div className="absolute inset-0 bg-gradient-to-br from-[#00dbe9]/5 to-transparent opacity-50" />
            
            <div className="relative z-10 flex items-center gap-6">
            <div className="relative">
                <motion.div 
                animate={energy > 5 ? { scale: [1, 1.15, 1], opacity: [0.3, 0.6, 0.3] } : { opacity: 0.2 }}
                className="absolute inset-0 bg-[#00dbe9] rounded-full blur-xl"
                />
                <div className={cn(
                "w-14 h-14 rounded-2xl flex items-center justify-center transition-all duration-300 relative z-10",
                energy > 5 ? "bg-[#00dbe9] text-black shadow-[0_0_30px_#00dbe9]" : "bg-white/5 text-white/20"
                )}>
                <Mic className="w-7 h-7" />
                </div>
            </div>
    
            <div className="flex-1">
                <div className="flex items-center justify-between mb-2">
                <span className="text-[11px] font-black text-white/80 uppercase tracking-widest flex items-center gap-2">
                    <Activity className="w-3 h-3" /> Input Signal
                </span>
                <span className="text-[11px] font-bold text-[#00dbe9] font-mono">{Math.round(energy)}%</span>
                </div>
                <div className="h-1.5 bg-white/5 rounded-full overflow-hidden border border-white/5">
                <motion.div 
                    className="h-full bg-gradient-to-r from-[#00dbe9] to-[#d8baff]"
                    animate={{ width: `${Math.min(energy, 100)}%` }}
                    transition={{ type: 'spring', bounce: 0, duration: 0.1 }}
                />
                </div>
            </div>
            </div>
        </div>
    
        <div className="flex flex-col gap-3 min-h-0">
            <span className="text-[10px] font-bold text-white/30 uppercase tracking-widest px-1">Source Selection</span>
            <div className="space-y-2">
            {devices.map(device => (
                <button
                key={device.name}
                onClick={() => setSelected(device.name)}
                className={cn(
                    "w-full p-4 rounded-xl border transition-all text-left flex items-center justify-between group",
                    selected === device.name 
                    ? "bg-white/[0.04] border-white/20 text-white shadow-xl" 
                    : "bg-white/[0.01] border-white/5 text-white/40 hover:bg-white/[0.02] hover:border-white/10"
                )}
                >
                <div className="flex items-center gap-3">
                    <Volume2 className={cn("w-4 h-4 transition-colors", selected === device.name ? "text-[#00dbe9]" : "text-white/20")} />
                    <span className="text-[11px] font-bold truncate max-w-[280px] uppercase tracking-tight">{device.name}</span>
                </div>
                {selected === device.name && <Check className="w-4 h-4 text-[#00dbe9]" />}
                </button>
            ))}
            {devices.length === 0 && (
                <div className="p-8 text-center border border-dashed border-white/10 rounded-xl bg-white/[0.01]">
                <span className="text-[11px] font-bold text-white/40 uppercase tracking-widest">No devices detected</span>
                </div>
            )}
            </div>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="mt-8 pt-8 border-t border-white/5 flex gap-4"
      >
        <button
          onClick={onBack}
          className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors"
        >
          Back
        </button>

        <button
          onClick={onNext}
          disabled={!selected}
          className={cn(
            "group relative flex-1 py-5 font-black rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4 shadow-[0_0_40px_rgba(0,0,0,0.5)]",
            selected 
              ? "bg-zinc-950 text-white border border-white/10 hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98]" 
              : "bg-white/5 text-white/20 border border-white/5 cursor-not-allowed"
          )}
        >
          {selected && <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />}
          <span className="relative z-10 uppercase tracking-[0.4em] text-[11px]">Finalize Initialization</span>
          <ArrowRight className="w-4 h-4 relative z-10 group-hover:translate-x-1 transition-transform text-[#00dbe9]" />
        </button>
      </motion.div>
    </div>
  );
};
