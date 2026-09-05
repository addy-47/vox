import React, { useEffect, useState } from 'react';
import { launchEngine, stopEngine } from '@/services/pipelineService';
import { listInputDevices, getSettings, updateSetting, type AudioDevice } from '@/services/settingsService';
import { onTelemetry } from '@/services/eventsService';
import { motion } from 'framer-motion';
import { Mic, Check, Volume2, Activity } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { WIZARD_STEP_HEADERS, AUDIO_SETUP_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface Props {
  onNext: () => void;
  onBack: () => void;
}

export const AudioSetupStep: React.FC<Props> = ({ onNext, onBack }) => {
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selected, setSelected] = useState<string>('');
  const [energy, setEnergy] = useState(0);

  useEffect(() => {
    let isMounted = true;

    const init = async () => {
      try {
        // Ensure engine is running so we get energy events
        await launchEngine();
        
        const devList = await listInputDevices();
        if (!isMounted) return;
        setDevices(devList);
        
        // Try to get current device from settings first
        try {
          const boot = await getSettings();
          const settings = boot.settings;
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

    const unlisten = onTelemetry((payload) => {
      const val = payload.energy ?? 0;
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
      isMounted = false;
      unlisten();
      stopEngine().catch(console.error);
    };
  }, []);

  const handleSelect = async (name: string) => {
    setSelected(name);
    try {
      await updateSetting('audio', 'input_device', name);
      // Restart engine to apply hardware change immediately
      await stopEngine();
      await launchEngine();
    } catch (e) {
      console.error('Failed to update audio device', e);
    }
  };

  return (
    <div className="flex flex-col h-full relative">
      <WizardHeader
        step={WIZARD_STEP_HEADERS.audio.step}
        title={WIZARD_STEP_HEADERS.audio.title}
        description={WIZARD_STEP_HEADERS.audio.description}
      />
 
      <div className="flex-1 flex flex-col gap-6 min-h-0">
        {/* Live Analysis Card */}
        <div className="flex-shrink-0 p-5 glass relative overflow-hidden">
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
                    : "bg-[rgba(var(--foreground),0.05)] border-transparent text-[rgb(var(--foreground-muted))]/50"
                )}>
                  <Mic className="w-7 h-7" />
                </div>
            </div>
    
            <div className="flex-1">
                <div className="flex items-center justify-between mb-2">
                <span className="text-[12px] font-black text-[rgb(var(--foreground))]/80 uppercase tracking-widest flex items-center gap-2">
                    <Activity className="w-3 h-3" /> {AUDIO_SETUP_COPY.liveLabel}
                </span>
                <span className="text-[12px] font-bold text-[rgb(var(--accent))] font-mono">{Math.round(energy)}%</span>
                </div>
                <div className="h-1.5 bg-[rgba(var(--foreground),0.05)] rounded-full overflow-hidden border border-[rgba(var(--foreground),0.08)]">
                <motion.div 
                    className="h-full" style={{ background: `linear-gradient(90deg, rgb(var(--accent)) 0%, rgba(var(--accent), 0.3) 100%)` }}
                    animate={{ width: `${Math.min(energy, 100)}%` }}
                    transition={{ type: 'tween', ease: 'easeOut', duration: 0.15 }}
                />
                </div>
            </div>
            </div>
        </div>
    
        <div className="flex-1 flex flex-col gap-3 min-h-0 overflow-y-auto pr-2 custom-scrollbar">
            <span className="text-[12px] font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-widest px-1">{AUDIO_SETUP_COPY.listTitle}</span>
            <div className="space-y-2">
            {devices.map(device => (
                <button
                key={device.name}
                onClick={() => handleSelect(device.name)}
                className={cn(
                    "w-full p-4 rounded-xl transition-all text-left flex items-center justify-between group",
                    selected === device.name 
                    ? "glass text-[rgb(var(--foreground))]" 
                    : "glass text-[rgb(var(--foreground-muted))]"
                )}
                >
                <div className="flex items-center gap-3">
                    <Volume2 className={cn("w-4 h-4 transition-colors", selected === device.name ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/40")} />
                    <span className="text-[12px] font-bold truncate max-w-[280px] uppercase tracking-tight">{device.name}</span>
                </div>
                {selected === device.name && <Check className="w-4 h-4 text-[rgb(var(--accent))]" />}
                </button>
            ))}
            {devices.length === 0 && (
                <div className="p-8 text-center border border-dashed border-[rgba(var(--border),0.08)] rounded-xl glass">
                <span className="text-[12px] font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-widest">{AUDIO_SETUP_COPY.empty}</span>
                </div>
            )}
            </div>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={onNext}
        nextLabel={WIZARD_CTA_LABELS.continueToVerification}
        isNextDisabled={!selected}
        showBack={true}
      />
    </div>
  );
};

