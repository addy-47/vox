import React, { useState } from "react";
import { Brain, ChevronDown, Activity, Volume2, Shield, Save, Sun, Moon } from "lucide-react";
import { cn } from "../../shared/lib/utils";

export const Settings: React.FC = () => {
  const [selectedModel] = useState("VOX-ENGINE-8B (LATEST)");
  const [selectedVoice, setSelectedVoice] = useState("ETHER");
  const [alwaysListening, setAlwaysListening] = useState(true);
  const [secureMode, setSecureMode] = useState(true);

  const [theme, setTheme] = React.useState<'dark' | 'light'>(
    (localStorage.getItem('theme') as 'dark' | 'light') || 'dark'
  );

  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  const toggleTheme = () => setTheme(prev => prev === 'dark' ? 'light' : 'dark');

  const voices = ["ETHER", "SOLAS", "KRYPTOS", "LYRA"];

  return (
    <div className="h-full w-full overflow-y-auto bg-[rgb(var(--background))] px-6 md:px-12 py-10 pb-32 md:pb-10 custom-scrollbar">
      <div className="max-w-7xl mx-auto">
        {/* Page header - Unified with History */}
        <div className="mb-12 flex flex-col md:flex-row md:items-center justify-between gap-6 pb-10 border-b border-[rgba(var(--border))]">
          <div className="space-y-4">
            <div className="flex items-center gap-2 mb-1">
              <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
              <span className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Configuration</span>
            </div>
            <h1 className="text-3xl md:text-4xl font-bold text-[rgb(var(--foreground))] tracking-tight">System <span className="text-[rgb(var(--foreground-muted))] opacity-40">Core</span></h1>
          </div>

          {/* Theme Toggle - Desktop Only in Header */}
          <button 
            onClick={toggleTheme}
            className="hidden md:flex items-center gap-3 px-6 py-3 rounded-2xl bg-white/[0.03] border border-white/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all group"
          >
            {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
            <span className="text-[11px] font-bold uppercase tracking-widest">{theme} Mode</span>
          </button>
        </div>

        <div className="grid lg:grid-cols-3 gap-8">
          {/* Main Settings Column */}
          <div className="lg:col-span-2 space-y-8">
            {/* AI Model Selection */}
            <div className="premium-card p-8">
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
                    <div className="p-5 rounded-xl bg-white/[0.03] border border-white/5">
                        <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Latency</div>
                        <div className="text-sm font-bold text-[rgb(var(--accent))]">24ms</div>
                    </div>
                    <div className="p-5 rounded-xl bg-white/[0.03] border border-white/5">
                        <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase mb-2 opacity-40">Status</div>
                        <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-60">Stable</div>
                    </div>
                 </div>
              </div>
            </div>

            {/* Voice Profile */}
            <div className="premium-card p-8">
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
                      "px-6 py-2.5 rounded-xl text-[11px] font-bold tracking-[0.15em] uppercase transition-all duration-500",
                      selectedVoice === v 
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm" 
                        : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-white/10 hover:bg-white/10"
                    )}
                  >
                    {v}
                  </button>
                ))}
              </div>

              <div className="h-24 w-full bg-white/[0.02] border border-white/5 rounded-2xl flex items-center justify-center overflow-hidden">
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
            <div className="premium-card p-8 space-y-8">
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
            <div className="premium-card p-8 border-[rgb(var(--accent))]/10 bg-[rgb(var(--accent))]/5">
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

        {/* Footer Actions */}
        <div className="mt-16 pt-10 border-t border-[rgba(var(--border))] space-y-8">
          {/* Mobile Theme Toggle */}
          <div className="md:hidden premium-card p-6 flex items-center justify-between bg-white/[0.02]">
            <div className="space-y-1">
              <p className="text-[11px] font-bold tracking-[0.2em] text-[rgb(var(--accent))] uppercase">Appearance</p>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] opacity-60">Toggle system visual mode</p>
            </div>
            <button 
              onClick={toggleTheme}
              className="flex items-center gap-3 px-6 py-3 rounded-xl bg-white/[0.05] border border-white/10 text-[rgb(var(--foreground))] transition-all active:scale-95"
            >
              {theme === 'dark' ? <Sun size={16} className="text-[rgb(var(--accent))]" /> : <Moon size={16} className="text-[rgb(var(--accent))]" />}
              <span className="text-[11px] font-bold uppercase tracking-widest">{theme}</span>
            </button>
          </div>

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
