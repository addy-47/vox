import React, { useState } from "react";
import { Brain, ChevronDown, Activity, Volume2, Shield, Save } from "lucide-react";
import { cn } from "../../shared/lib/utils";

export const Settings: React.FC = () => {
  const [activeTab, setActiveTab] = useState<"core" | "tray">("core");
  const [selectedModel] = useState("VOX-ENGINE-8B (LATEST)");
  const [selectedVoice, setSelectedVoice] = useState("ETHER");
  const [alwaysListening, setAlwaysListening] = useState(true);
  const [secureMode, setSecureMode] = useState(true);
  const [trayEnabled, setTrayEnabled] = useState(() => {
    const saved = localStorage.getItem('isTrayEnabled');
    return saved === null ? true : saved === 'true';
  });

  const [theme] = React.useState<'dark' | 'light'>(
    (localStorage.getItem('theme') as 'dark' | 'light') || 'dark'
  );

  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  React.useEffect(() => {
    localStorage.setItem('isTrayEnabled', String(trayEnabled));
  }, [trayEnabled]);


  const voices = ["ETHER", "SOLAS", "KRYPTOS", "LYRA"];

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative">
      {/* Page header */}
      <header className="p-6 md:p-12 border-b border-[rgba(var(--border))] glass-panel shrink-0">
        <div className="max-w-7xl mx-auto w-full flex flex-col md:flex-row md:items-center justify-between gap-8">
          <div className="space-y-4">
            <div className="flex items-center gap-2 mb-1">
              <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
              <span className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Configuration</span>
            </div>
            <h1 className="text-3xl md:text-4xl font-bold text-[rgb(var(--foreground))] tracking-tight">System <span className="text-[rgb(var(--foreground-muted))] opacity-40">Core</span></h1>
          </div>

          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2 p-1.5 bg-white/[0.03] border border-white/10 rounded-2xl">
              <button 
                onClick={() => setActiveTab("core")}
                className={cn(
                  "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all",
                  activeTab === "core" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Core Settings
              </button>
              <button 
                onClick={() => setActiveTab("tray")}
                className={cn(
                  "px-6 py-2.5 rounded-xl text-[11px] font-bold uppercase tracking-widest transition-all",
                  activeTab === "tray" 
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg" 
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                Tray HUD
              </button>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto custom-scrollbar p-6 md:p-12 pb-12 md:pb-12">
        <div className="max-w-7xl mx-auto">
          {activeTab === "core" ? (
            <div className="grid lg:grid-cols-3 gap-8">
              {/* Main Settings Column */}
              <div className="lg:col-span-2 space-y-8">
                {/* AI Model Selection */}
                <div className="premium-card p-6 md:p-8">
                  <div className="flex items-center justify-between mb-8">
                    <div className="space-y-1">
                      <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Neural Engine</h2>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Architectural Model Layer</p>
                    </div>
                    <Brain className="text-[rgb(var(--accent))]" size={20} />
                  </div>

                  <div className="grid md:grid-cols-2 gap-6">
                    <div className="space-y-3">
                        <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Active Inference</label>
                        <div className="flex items-center justify-between px-5 py-4 rounded-xl bg-white/[0.03] border border-white/10 cursor-pointer hover:border-[rgb(var(--accent))]/30 transition-all group">
                          <span className="text-xs font-mono text-[rgb(var(--foreground))] opacity-80">{selectedModel}</span>
                          <ChevronDown size={14} className="text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))]" />
                        </div>
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                        <div className="p-4 md:p-5 rounded-xl bg-white/[0.03] border border-white/5">
                            <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Latency</div>
                            <div className="text-sm font-bold text-[rgb(var(--accent))]">24ms</div>
                        </div>
                        <div className="p-4 md:p-5 rounded-xl bg-white/[0.03] border border-white/5">
                            <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Status</div>
                            <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-60">Stable</div>
                        </div>
                    </div>
                  </div>
                </div>

                {/* Voice Profile */}
                <div className="premium-card p-6 md:p-8">
                  <div className="flex items-center justify-between mb-8">
                    <div className="space-y-1">
                      <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Vocal Profile</h2>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Acoustic Output Parameters</p>
                    </div>
                    <Volume2 className="text-[rgb(var(--accent))]" size={20} />
                  </div>

                  <div className="flex flex-wrap gap-3 mb-8">
                    {voices.map((v) => (
                      <button
                        key={v}
                        onClick={() => setSelectedVoice(v)}
                        className={cn(
                          "px-5 py-2 rounded-xl text-[11px] font-bold tracking-[0.15em] uppercase transition-all duration-500",
                          selectedVoice === v 
                            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm" 
                            : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-white/10 hover:bg-white/10"
                        )}
                      >
                        {v}
                      </button>
                    ))}
                  </div>

                  <div className="h-20 w-full bg-white/[0.02] border border-white/5 rounded-2xl flex items-center justify-center overflow-hidden">
                    <div className="flex items-center gap-1.5">
                        {[4, 12, 24, 42, 32, 18, 48, 36, 12, 6].map((h, i) => (
                          <div key={i} className="w-1 rounded-full bg-[rgb(var(--accent))]/40 animate-pulse" style={{ height: h, animationDelay: `${i * 0.1}s` }} />
                        ))}
                    </div>
                  </div>
                </div>
              </div>

              {/* Sidebar Settings Column */}
              <div className="space-y-8">
                {/* Interaction Toggles */}
                <div className="premium-card p-6 md:p-8 space-y-8">
                  <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40 mb-2">Interactions</h3>
                  
                  <div className="flex items-center justify-between">
                      <div className="space-y-1">
                        <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">Always Listening</div>
                        <div className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-50">Passive VAD Monitor</div>
                      </div>
                      <button 
                        onClick={() => setAlwaysListening(!alwaysListening)}
                        className={cn(
                          "w-12 h-6 rounded-full relative transition-all duration-500",
                          alwaysListening ? "bg-[rgb(var(--accent))]" : "bg-white/[0.05]"
                        )}
                      >
                        <div className={cn(
                          "absolute top-1 w-4 h-4 rounded-full bg-white transition-all duration-500",
                          alwaysListening ? "left-7" : "left-1"
                        )} />
                      </button>
                  </div>

                  <div className="flex items-center justify-between">
                      <div className="space-y-1">
                        <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80">Secure Enclave</div>
                        <div className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-50">Local Neural Isolation</div>
                      </div>
                      <button 
                        onClick={() => setSecureMode(!secureMode)}
                        className={cn(
                          "w-12 h-6 rounded-full relative transition-all duration-500",
                          secureMode ? "bg-[rgb(var(--accent))]" : "bg-white/[0.05]"
                        )}
                      >
                        <div className={cn(
                          "absolute top-1 w-4 h-4 rounded-full bg-white transition-all duration-500",
                          secureMode ? "left-7" : "left-1"
                        )} />
                      </button>
                  </div>
                </div>

                {/* Status Info */}
                <div className="premium-card p-6 md:p-8 border-[rgb(var(--accent))]/10 bg-[rgb(var(--accent))]/5">
                  <div className="flex items-center gap-3 mb-6">
                      <Shield size={16} className="text-[rgb(var(--accent))]" />
                      <span className="text-[11px] font-bold tracking-widest text-[rgb(var(--accent))] uppercase">Gateway Status</span>
                  </div>
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed mb-6 opacity-70">
                    VOX is operating in a localized neural environment. All biometric and vocal signatures are purged post-inference.
                  </p>
                  <button className="w-full py-3 rounded-xl bg-white/[0.03] border border-white/10 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:text-[rgb(var(--foreground))] transition-all">
                      Inspect Metadata
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className="max-w-4xl space-y-8 pb-20">
               <div className="premium-card p-8 space-y-10">
                  <div className="flex items-center justify-between">
                    <div className="space-y-2">
                      <h2 className="text-xl font-bold text-[rgb(var(--foreground))]">Tray HUD Configuration</h2>
                      <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed opacity-60">
                        Adjust visual aesthetics and behavioral parameters of the background HUD.
                      </p>
                    </div>
                    <button 
                        onClick={() => setTrayEnabled(!trayEnabled)}
                        className={cn(
                          "w-14 h-7 rounded-full relative transition-all duration-500",
                          trayEnabled ? "bg-[rgb(var(--accent))]" : "bg-white/[0.05]"
                        )}
                      >
                        <div className={cn(
                          "absolute top-1 w-5 h-5 rounded-full bg-white transition-all duration-500",
                          trayEnabled ? "left-8" : "left-1"
                        )} />
                      </button>
                  </div>

                  <div className="grid md:grid-cols-2 gap-12 pt-10 border-t border-white/5">
                    {/* Visual Settings */}
                    <div className="space-y-8">
                       <h3 className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest">Aesthetics</h3>
                       
                       <div className="space-y-6">
                          <div className="flex items-center justify-between">
                             <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Text Color</span>
                             <div className="flex gap-2">
                                {['accent', 'white', 'muted'].map(c => (
                                   <button 
                                      key={c}
                                      className="w-6 h-6 rounded-lg border border-white/10 transition-transform active:scale-90"
                                      style={{ backgroundColor: c === 'accent' ? 'rgb(var(--accent))' : c === 'white' ? 'white' : 'rgba(255,255,255,0.4)' }}
                                      onClick={() => localStorage.setItem('trayTextColor', c)}
                                   />
                                ))}
                             </div>
                          </div>

                          <div className="space-y-3">
                             <div className="flex justify-between">
                                <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Blur Density</span>
                                <span className="text-[10px] font-mono opacity-40">40px</span>
                             </div>
                             <input type="range" className="w-full h-1.5 bg-white/5 rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]" />
                          </div>

                          <div className="flex items-center justify-between">
                             <div className="space-y-1">
                                <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Glass Tint</span>
                                <p className="text-[10px] opacity-30 uppercase tracking-widest">Enable backdrop colorization</p>
                             </div>
                             <button className="w-10 h-5 rounded-full bg-[rgb(var(--accent))] relative">
                                <div className="absolute right-1 top-1 w-3 h-3 bg-white rounded-full" />
                             </button>
                          </div>
                       </div>
                    </div>

                    {/* Behavioral Settings */}
                    <div className="space-y-8">
                       <h3 className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-widest">Behavior</h3>

                       <div className="space-y-8">
                          <div className="space-y-4">
                             <div className="flex justify-between">
                                <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Hide Delay</span>
                                <span className="text-[10px] font-mono opacity-40">5.0s</span>
                             </div>
                             <div className="grid grid-cols-5 gap-2">
                                {[1, 2, 3, 5, 10].map(s => (
                                   <button 
                                      key={s}
                                      className="py-2 rounded-lg bg-white/5 border border-white/5 text-[10px] font-bold hover:bg-white/10 transition-colors"
                                      onClick={() => localStorage.setItem('trayHideDuration', String(s))}
                                   >
                                      {s}s
                                   </button>
                                ))}
                             </div>
                          </div>

                          <div className="space-y-4">
                             <div className="flex justify-between">
                                <span className="text-xs text-[rgb(var(--foreground))] opacity-70 font-medium">Fade Transition</span>
                                <span className="text-[10px] font-mono opacity-40">0.8s</span>
                             </div>
                             <div className="grid grid-cols-3 gap-3">
                                {['Snappy', 'Smooth', 'Liquid'].map(f => (
                                   <button 
                                      key={f}
                                      className="py-2.5 rounded-lg bg-white/5 border border-white/5 text-[10px] font-bold hover:bg-white/10 transition-colors uppercase tracking-widest"
                                   >
                                      {f}
                                   </button>
                                ))}
                             </div>
                          </div>
                       </div>
                    </div>
                  </div>
               </div>

               <div className="premium-card p-6 border-[rgb(var(--accent))]/20 bg-[rgb(var(--accent))]/5">
                  <p className="text-[10px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase mb-4">Monitor Status</p>
                  <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed opacity-70">
                    Your Tray HUD utilizes a low-latency VAD (Voice Activity Detection) monitor. Adjust the sensitivity if the HUD appears during background noise.
                  </p>
               </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="max-w-7xl mx-auto mt-16 pt-10 border-t border-[rgba(var(--border))] space-y-8">
          <div className="flex flex-col sm:flex-row items-center justify-between gap-6">
            <button className="flex items-center gap-2 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:text-[rgb(var(--accent))] transition-all opacity-40 hover:opacity-100">
               <Activity size={14} />
               Restore Factory Synthesis
            </button>
            <div className="flex items-center gap-4 w-full sm:w-auto">
               <button className="flex-1 sm:flex-none px-8 py-3.5 rounded-xl text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] hover:bg-white/[0.03] transition-all">
                 Discard
               </button>
               <button className="flex-1 sm:flex-none px-10 py-3.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-bold text-[11px] tracking-[0.2em] uppercase flex items-center justify-center gap-3 hover:scale-105 active:scale-95 transition-all shadow-lg">
                 <Save size={14} /> Commit Changes
               </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
