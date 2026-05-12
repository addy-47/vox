import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion } from 'framer-motion';
import { Mic, Check, Volume2 } from 'lucide-react';

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
      const devList = await invoke<AudioDevice[]>('list_input_devices');
      setDevices(devList);
      const def = devList.find(d => d.is_default);
      if (def) setSelected(def.name);
    };
    load();

    // Listen for energy level to show meter
    const unlisten = listen<number>('audio_energy', (event) => {
      setEnergy(event.payload * 100);
    });

    return () => {
      unlisten.then(u => u());
    };
  }, []);

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-3xl font-bold text-white mb-2">Audio Calibration</h2>
        <p className="text-neutral-400">Select your preferred microphone and test the input levels.</p>
      </div>

      {/* Live Meter */}
      <div className="p-8 bg-neutral-900 border border-neutral-800 rounded-3xl text-center">
        <div className="inline-flex h-16 w-16 bg-indigo-500/10 rounded-2xl items-center justify-center mb-6">
          <Mic className={`h-8 w-8 ${energy > 5 ? 'text-indigo-400' : 'text-neutral-600'} transition-colors`} />
        </div>
        
        <div className="max-w-xs mx-auto">
          <div className="h-4 bg-neutral-800 rounded-full overflow-hidden mb-2">
            <motion.div 
              className="h-full bg-gradient-to-r from-indigo-600 to-purple-500"
              animate={{ width: `${Math.min(energy * 2, 100)}%` }}
              transition={{ type: 'spring', bounce: 0, duration: 0.1 }}
            />
          </div>
          <p className="text-xs text-neutral-500 uppercase tracking-tighter">Live Input Level</p>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-sm font-medium text-neutral-500 px-2">Input Device</p>
        <div className="grid gap-2">
          {devices.map(device => (
            <button
              key={device.name}
              onClick={() => setSelected(device.name)}
              className={`flex items-center justify-between p-4 rounded-2xl border transition-all ${
                selected === device.name 
                  ? 'bg-white/5 border-white/20 text-white' 
                  : 'bg-transparent border-neutral-800 text-neutral-500 hover:border-neutral-700'
              }`}
            >
              <div className="flex items-center gap-3">
                <Volume2 className="h-4 w-4" />
                <span className="text-sm font-medium truncate max-w-[240px]">{device.name}</span>
              </div>
              {selected === device.name && <Check className="h-4 w-4 text-indigo-500" />}
            </button>
          ))}
        </div>
      </div>

      <button
        onClick={onNext}
        disabled={!selected}
        className="w-full py-4 bg-indigo-600 hover:bg-indigo-500 disabled:bg-neutral-900 disabled:text-neutral-700 text-white font-bold rounded-2xl transition-all"
      >
        Continue to Voice Test
      </button>
    </div>
  );
};
