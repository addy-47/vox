import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion } from 'framer-motion';
import { Mic, Check, Volume2, Activity } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

// --- Modular Components ---
import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';

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
    const init = async () => {
      try {
        // Ensure engine is running so we get energy events
        await invoke('launch_engine');
        
        const devList = await invoke<AudioDevice[]>('list_input_devices');
        setDevices(devList);
        
        // Try to get current device from settings first
        try {
          const settings = await invoke<any>('get_settings');
          if (settings.audio.input_device) {
            setSelected(settings.audio.input_device);
          } else {
            const def = devList.find(d => d.is_default);
            if (def) setSelected(def.name);
          }
        } catch {
          const def = devList.find(d => d.is_default);
          if (def) setSelected(def.name);
        }
      } catch (e) {
        console.error('Audio initialization failed', e);
      }
    };
    init();

    let lastTime = 0;
    let localEnergy = 0;
    const THROTTLE_MS = 60;

    const unlisten = listen<any>('audio_energy', (event) => {
      const val = typeof event.payload === 'number' ? event.payload : event.payload?.energy || 0;
      const targetEnergy = val * 100;
      
      // Exponential moving average smoothing for organic signal rise and slow decay (low-pass)
      localEnergy = localEnergy * 0.85 + targetEnergy * 0.15;
      
      const now = Date.now();
      if (now - lastTime >= THROTTLE_MS) {
        setEnergy(localEnergy);
        lastTime = now;
      }
    });

    return () => {
      unlisten.then(u => u());
    };
  }, []);

  const handleSelect = async (name: string) => {
    setSelected(name);
    try {
      await invoke('update_setting', { domain: 'audio', key: 'input_device', value: name });
      // Restart engine to apply hardware change immediately
      await invoke('stop_engine');
      await invoke('launch_engine');
    } catch (e) {
      console.error('Failed to update audio device', e);
    }
  };

  return (
    <div className="flex flex-col h-full relative">
      <WizardHeader 
        step="Step 3.0 • Audio Input"
        title="Device Selection"
        description="Configuring audio input for real-time interaction. Select your primary microphone to enable voice understanding."
      />
 
      <div className="flex-1 flex flex-col gap-6 min-h-0">
        {/* Live Analysis Card */}
        <div className="flex-shrink-0 p-5 glass-whisper glass-base relative overflow-hidden">
            <div className="absolute inset-0 bg-gradient-to-br from-[rgb(var(--accent))]/5 to-transparent opacity-50 pointer-events-none" />
            
            <div className="relative z-10 flex items-center gap-6">
            <div className="relative">
                {/* Relaxed steady ambient glow when mic is active */}
                {selected && (
                  <div 
                    className="absolute inset-0 bg-[rgb(var(--accent))]/10 rounded-full blur-xl animate-pulse" 
                    style={{ animationDuration: '3s' }} 
                  />
                )}
                <div className={cn(
                  "w-14 h-14 rounded-2xl flex items-center justify-center transition-all duration-300 relative z-10 border",
                  selected 
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.15)]" 
                    : "bg-white/5 border-transparent text-white/20"
                )}>
                  <Mic className="w-7 h-7" />
                </div>
            </div>
    
            <div className="flex-1">
                <div className="flex items-center justify-between mb-2">
                <span className="text-[11px] font-black text-white/80 uppercase tracking-widest flex items-center gap-2">
                    <Activity className="w-3 h-3" /> Input Signal
                </span>
                <span className="text-[11px] font-bold text-[rgb(var(--accent))] font-mono">{Math.round(energy)}%</span>
                </div>
                <div className="h-1.5 bg-white/5 rounded-full overflow-hidden border border-white/5">
                <motion.div 
                    className="h-full bg-gradient-to-r from-[rgb(var(--accent))] to-[#d8baff]"
                    animate={{ width: `${Math.min(energy, 100)}%` }}
                    transition={{ type: 'tween', ease: 'easeOut', duration: 0.15 }}
                />
                </div>
            </div>
            </div>
        </div>
    
        <div className="flex-1 flex flex-col gap-3 min-h-0 overflow-y-auto pr-2 custom-scrollbar">
            <span className="text-[10px] font-bold text-white/30 uppercase tracking-widest px-1">Source Selection</span>
            <div className="space-y-2">
            {devices.map(device => (
                <button
                key={device.name}
                onClick={() => handleSelect(device.name)}
                className={cn(
                    "w-full p-4 rounded-xl transition-all text-left flex items-center justify-between group",
                    selected === device.name 
                    ? "glass-surface glass-base text-white" 
                    : "glass-whisper glass-base text-[rgb(var(--foreground-muted))]"
                )}
                >
                <div className="flex items-center gap-3">
                    <Volume2 className={cn("w-4 h-4 transition-colors", selected === device.name ? "text-[rgb(var(--accent))]" : "text-white/20")} />
                    <span className="text-[11px] font-bold truncate max-w-[280px] uppercase tracking-tight">{device.name}</span>
                </div>
                {selected === device.name && <Check className="w-4 h-4 text-[rgb(var(--accent))]" />}
                </button>
            ))}
            {devices.length === 0 && (
                <div className="p-8 text-center border border-dashed border-[rgba(var(--border),0.08)] rounded-xl glass-whisper">
                <span className="text-[11px] font-bold text-white/40 uppercase tracking-widest">No devices detected</span>
                </div>
            )}
            </div>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={onNext}
        nextLabel="Finalize Initialization"
        isNextDisabled={!selected}
        showBack={true}
      />
    </div>
  );
};

