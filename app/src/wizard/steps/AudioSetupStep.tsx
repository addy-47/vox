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
}

export const AudioSetupStep: React.FC<Props> = ({ onNext }) => {
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
    <div className="flex flex-col gap-8 h-full">
      <header>
        <h2 className="text-3xl font-black text-white mb-2 tracking-tighter uppercase">AUDIO CONFIGURATION</h2>
        <p className="text-white/80 text-sm font-light">Calibrating audio input for system interaction.</p>
      </header>

      {/* Live Analysis Card */}
      <div className="p-6 bg-white/[0.02] border border-white/10 rounded-2xl relative overflow-hidden group">
        <div className="absolute inset-0 bg-gradient-to-br from-[#00dbe9]/5 to-transparent opacity-50" />
        
        <div className="relative z-10 flex items-center gap-6">
          <div className="relative">
            <motion.div 
              animate={energy > 5 ? { scale: [1, 1.15, 1], opacity: [0.3, 0.6, 0.3] } : { opacity: 0.2 }}
              className="absolute inset-0 bg-[#00dbe9] rounded-full blur-xl"
            />
            <div className={cn(
              "w-16 h-16 rounded-2xl flex items-center justify-center transition-all duration-300 relative z-10",
              energy > 5 ? "bg-[#00dbe9] text-black shadow-[0_0_30px_#00dbe9]" : "bg-white/5 text-white/20"
            )}>
              <Mic className="w-8 h-8" />
            </div>
          </div>

          <div className="flex-1">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[11px] font-black text-white/80 uppercase tracking-widest flex items-center gap-2">
                <Activity className="w-3 h-3" /> Input Signal
              </span>
              <span className="text-[11px] font-bold text-[#00dbe9]">{Math.round(energy)}%</span>
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

      <div className="flex flex-col gap-3 flex-1 overflow-hidden">
        <span className="text-[11px] font-black text-white/80 uppercase tracking-widest px-1">Source Selection</span>
        <div className="space-y-2 overflow-y-auto pr-1 custom-scrollbar">
          {devices.map(device => (
            <button
              key={device.name}
              onClick={() => setSelected(device.name)}
              className={cn(
                "w-full p-4 rounded-xl border transition-all text-left flex items-center justify-between group",
                selected === device.name 
                  ? "bg-[#00dbe9]/10 border-[#00dbe9]/50 text-white shadow-xl" 
                  : "bg-white/[0.02] border-white/5 text-white/80 hover:bg-white/5 hover:border-white/10"
              )}
            >
              <div className="flex items-center gap-3">
                <Volume2 className={cn("w-4 h-4", selected === device.name ? "text-[#00dbe9]" : "text-white/40")} />
                <span className="text-[11px] font-bold truncate max-w-[300px] uppercase tracking-tight">{device.name}</span>
              </div>
              {selected === device.name && <Check className="w-4 h-4 text-[#00dbe9]" />}
            </button>
          ))}
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <button
          onClick={onNext}
          disabled={!selected}
          className={cn(
            "group relative w-full py-5 font-bold rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4 shadow-2xl",
            selected 
              ? "bg-[#0a0a0a] border border-white/10 text-white hover:bg-zinc-900 active:scale-[0.98]" 
              : "bg-white/5 text-white/40 border border-white/5 cursor-not-allowed"
          )}
        >
          {selected && <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />}
          <span className="relative z-10 uppercase tracking-[0.3em] text-[11px]">Finalize Initialization</span>
          <ArrowRight className="w-4 h-4 relative z-10 group-hover:translate-x-1 transition-transform" />
        </button>
      </motion.div>
    </div>
  );
};
