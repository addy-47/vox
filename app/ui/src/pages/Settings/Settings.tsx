import React, { useState } from "react";
import { Brain, ChevronDown, Activity, Volume2, Shield, Save } from "lucide-react";
import { cn } from "../../shared/lib/utils";

export const Settings: React.FC = () => {
  const [selectedModel] = useState("VOX-ENGINE-8B (LATEST)");
  const [selectedVoice, setSelectedVoice] = useState("ETHER");
  const [alwaysListening, setAlwaysListening] = useState(true);
  const [secureMode, setSecureMode] = useState(true);

  const voices = ["ETHER", "SOLAS", "KRYPTOS", "LYRA"];

  return (
    <div className="h-full w-full overflow-y-auto bg-background px-6 md:px-12 py-10">
      <div className="max-w-6xl mx-auto">
        {/* Page header */}
        <div className="mb-12">
          <div className="flex items-center gap-2 mb-3">
            <div className="w-1.5 h-1.5 rounded-full bg-[#00dbe9]" />
            <span className="text-[10px] font-bold tracking-[0.2em] text-[#00dbe9] uppercase">System Preferences</span>
          </div>
          <h1 className="text-4xl font-bold font-display text-white">Interface <span className="text-white/40">& Modules</span></h1>
          <p className="mt-4 text-white/40 max-w-2xl leading-relaxed">
            Configure the core intelligence parameters and interaction behaviors of the VOX environment.
          </p>
        </div>

        <div className="grid lg:grid-cols-3 gap-8">
          {/* Main Settings Column */}
          <div className="lg:col-span-2 space-y-8">
            {/* AI Model Selection */}
            <div className="premium-card p-8">
              <div className="flex items-center justify-between mb-8">
                <div>
                  <h2 className="text-xl font-bold text-white mb-1">Neural Engine</h2>
                  <p className="text-xs text-white/30">Select the underlying model for reasoning.</p>
                </div>
                <Brain className="text-[#00dbe9]" size={24} />
              </div>

              <div className="grid md:grid-cols-2 gap-6">
                 <div className="space-y-2">
                    <label className="text-[10px] font-bold text-white/20 uppercase tracking-widest">Active Model</label>
                    <div className="flex items-center justify-between px-4 py-3 rounded-xl bg-white/5 border border-white/10 cursor-pointer hover:border-[#00dbe9]/30 transition-all">
                      <span className="text-sm font-mono text-white/70">{selectedModel}</span>
                      <ChevronDown size={16} className="text-white/20" />
                    </div>
                 </div>
                 <div className="grid grid-cols-2 gap-4">
                    <div className="p-4 rounded-xl bg-white/5 border border-white/5">
                        <div className="text-[9px] font-bold text-white/20 uppercase mb-1">Latency</div>
                        <div className="text-sm font-bold text-[#00dbe9]">24ms</div>
                    </div>
                    <div className="p-4 rounded-xl bg-white/5 border border-white/5">
                        <div className="text-[9px] font-bold text-white/20 uppercase mb-1">Status</div>
                        <div className="text-sm font-bold text-white/60 italic">Stable</div>
                    </div>
                 </div>
              </div>
            </div>

            {/* Voice & Language */}
            <div className="premium-card p-8">
              <div className="flex items-center justify-between mb-8">
                <div>
                  <h2 className="text-xl font-bold text-white mb-1">Vocal Profile</h2>
                  <p className="text-xs text-white/30">Customize the acoustic output and tone.</p>
                </div>
                <Volume2 className="text-[#00dbe9]" size={24} />
              </div>

              <div className="flex flex-wrap gap-3 mb-8">
                {voices.map((v) => (
                  <button
                    key={v}
                    onClick={() => setSelectedVoice(v)}
                    className={cn(
                      "px-6 py-2 rounded-full text-[10px] font-bold tracking-widest uppercase transition-all duration-300",
                      selectedVoice === v 
                        ? "bg-[#00dbe9] text-[#050505]" 
                        : "bg-white/5 text-white/40 border border-white/10 hover:bg-white/10"
                    )}
                  >
                    {v}
                  </button>
                ))}
              </div>

              <div className="h-20 w-full bg-white/5 border border-white/5 rounded-xl flex items-center justify-center overflow-hidden">
                 <div className="flex items-center gap-1">
                    {[4, 8, 12, 18, 24, 32, 24, 18, 12, 8, 4].map((h, i) => (
                      <div key={i} className="w-1 rounded-full bg-[#00dbe9]/40 animate-pulse" style={{ height: h, animationDelay: `${i * 0.1}s` }} />
                    ))}
                 </div>
              </div>
            </div>
          </div>

          {/* Sidebar Settings Column */}
          <div className="space-y-6">
            {/* Interaction Toggles */}
            <div className="premium-card p-6 space-y-6">
               <h3 className="text-[10px] font-bold text-white/20 uppercase tracking-[0.2em] mb-4">Interactions</h3>
               
               <div className="flex items-center justify-between">
                  <div>
                    <div className="text-sm font-bold text-white/80">Always Listening</div>
                    <div className="text-[10px] text-white/30">Continuous VAD monitoring</div>
                  </div>
                  <button 
                    onClick={() => setAlwaysListening(!alwaysListening)}
                    className={cn(
                      "w-12 h-6 rounded-full relative transition-all duration-500",
                      alwaysListening ? "bg-[#00dbe9]" : "bg-white/10"
                    )}
                  >
                    <div className={cn(
                      "absolute top-1 w-4 h-4 rounded-full bg-white transition-all duration-500 shadow-lg",
                      alwaysListening ? "left-7" : "left-1"
                    )} />
                  </button>
               </div>

               <div className="flex items-center justify-between">
                  <div>
                    <div className="text-sm font-bold text-white/80">Secure Enclave</div>
                    <div className="text-[10px] text-white/30">Hardware-level encryption</div>
                  </div>
                  <button 
                    onClick={() => setSecureMode(!secureMode)}
                    className={cn(
                      "w-12 h-6 rounded-full relative transition-all duration-500",
                      secureMode ? "bg-[#00dbe9]" : "bg-white/10"
                    )}
                  >
                    <div className={cn(
                      "absolute top-1 w-4 h-4 rounded-full bg-white transition-all duration-500 shadow-lg",
                      secureMode ? "left-7" : "left-1"
                    )} />
                  </button>
               </div>
            </div>

            {/* Quick Actions */}
            <div className="premium-card p-6 bg-[#00dbe9]/5 border-[#00dbe9]/10">
               <div className="flex items-center gap-3 mb-4">
                  <Shield size={16} className="text-[#00dbe9]" />
                  <span className="text-[10px] font-bold tracking-widest text-[#00dbe9] uppercase">Compliance</span>
               </div>
               <p className="text-xs text-white/40 leading-relaxed mb-4">
                 VOX is running in a local-only environment. No data telemetry is being transmitted.
               </p>
               <button className="w-full py-2 rounded-lg bg-white/5 border border-white/10 text-[10px] font-bold text-white/60 tracking-widest uppercase hover:bg-white/10 transition-all">
                  View Data Log
               </button>
            </div>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="mt-12 pt-8 border-t border-white/5 flex items-center justify-between">
           <button className="flex items-center gap-2 text-[10px] font-bold text-white/20 uppercase tracking-widest hover:text-white/40 transition-all">
              <Activity size={14} />
              Reset to Defaults
           </button>
           <div className="flex items-center gap-4">
              <button className="px-6 py-3 rounded-xl text-[10px] font-bold text-white/30 uppercase tracking-widest hover:text-white/50 transition-all">
                Cancel
              </button>
              <button className="px-8 py-3 rounded-xl bg-[#00dbe9] text-[#050505] font-bold text-[10px] tracking-widest uppercase flex items-center gap-2 hover:opacity-90 transition-all shadow-[0_0_20px_rgba(0,219,233,0.2)]">
                <Save size={14} /> Save Configuration
              </button>
           </div>
        </div>
      </div>
    </div>
  );
};
