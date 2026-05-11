import React, { useState } from "react";
import { Brain, Volume2, Palette, Cpu, MemoryStick, ChevronLeft, MousePointerClick, Sun, Moon, Shield } from "lucide-react";
import { HexColorPicker } from "react-colorful";
import { cn } from "@/shared/lib/utils";
import { useSettings, VoiceProfile } from "@/shared/context/SettingsContext";

// ─── Sub-Components (Isolated Cards) ──────────────────────────────────────────

const EngineCard: React.FC = React.memo(() => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [flippedView, setFlippedView] = useState<'llm' | 'asr' | 'vad' | null>(null);
  
  if (!draftSettings || !modelCatalog) return null;

  const activeLlm = modelCatalog.llm.find(m => m.id === draftSettings.llm.model) || modelCatalog.llm[0];
  const activeAsr = modelCatalog.asr.find(m => m.id === draftSettings.asr.model) || modelCatalog.asr[0];

  return (
    <div className="relative h-full" style={{ perspective: '1000px' }}>
      <div 
        className="w-full h-full relative transition-transform duration-500"
        style={{ 
          transformStyle: 'preserve-3d', 
          transform: flippedView ? 'rotateY(180deg)' : 'rotateY(0deg)' 
        }}
      >
        {/* Front Face */}
        <div 
          className="premium-card p-6 md:p-8 flex flex-col h-full"
          style={{ backfaceVisibility: 'hidden' }}
        >
          {/* Header */}
          <div className="flex items-center gap-3 mb-8 shrink-0">
            <Brain className="text-[rgb(var(--accent))]" size={20} />
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Core Engine</h2>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Brain & Reasoning</p>
            </div>
          </div>

          {/* Body */}
          <div className="space-y-6 flex-1 min-h-0">
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                  <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-80">Active Model</label>
                  <div className="flex gap-4">
                    <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))] opacity-60">
                        <Cpu size={12} />
                        {activeLlm.parameters}
                    </div>
                    <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground))] opacity-80">
                        <MemoryStick size={12} />
                        {activeLlm.ram_usage}
                    </div>
                  </div>
              </div>
              
              <div className="grid grid-cols-1 gap-3">
                  {modelCatalog.llm.map(m => (
                    <button
                      key={m.id}
                      onClick={() => {
                        updateDraft("llm", "model", m.id);
                        setFlippedView('llm');
                      }}
                      className="w-full text-left p-5 rounded-2xl transition-all duration-300 border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/[0.05]"
                    >
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-l font-bold tracking-tight text-[rgb(var(--foreground))] opacity-80">{m.name}</span>
                      </div>
                      <p className="text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))] opacity-80">
                        {m.description}
                      </p>
                    </button>
                  ))}
              </div>
            </div>

            <div className="grid md:grid-cols-2 gap-6 pb-2">
              <div className="space-y-3">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">Speech Recognition</div>
                <button 
                  onClick={() => setFlippedView('asr')}
                  className="w-full text-left p-4 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/[0.05] transition-colors"
                >
                  <div className="text-l font-bold text-[rgb(var(--foreground))] opacity-80 mb-1">{activeAsr.name}</div>
                  <div className="text-[12px] text-[rgb(var(--foreground-muted))] opacity-80">{activeAsr.description}</div>
                </button>
              </div>
              <div className="space-y-3">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">Voice Detection</div>
                <button 
                  onClick={() => setFlippedView('vad')}
                  className="w-full text-left p-4 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/[0.05] transition-colors"
                >
                  <div className="text-l font-bold text-[rgb(var(--foreground))] opacity-80 mb-1">TenVAD</div>
                  <div className="text-[12px] text-[rgb(var(--foreground-muted))] opacity-80 ">Filters background noise for clear triggers</div>
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Back Face */}
        <div 
          className="premium-card p-6 md:p-8 flex flex-col absolute inset-0"
          style={{ backfaceVisibility: 'hidden', transform: 'rotateY(180deg)' }}
        >
          {/* Header */}
          <div className="flex items-center gap-4 mb-8 shrink-0">
            <button 
              onClick={() => setFlippedView(null)}
              className="w-8 h-8 rounded-full bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] flex items-center justify-center hover:bg-[rgb(var(--foreground))]/10 transition-colors"
            >
              <ChevronLeft size={16} className="text-[rgb(var(--foreground))]" />
            </button>
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">
                {flippedView === 'llm' && 'LLM Settings'}
                {flippedView === 'asr' && 'ASR Settings'}
                {flippedView === 'vad' && 'VAD Settings'}
              </h2>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Configuration</p>
            </div>
          </div>

          {/* Body */}
          <div className="space-y-6 flex-1 min-h-0">
            {flippedView === 'llm' && (
              <div className="space-y-6">
                <div className="space-y-3">
                  <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">System Prompt</label>
                  <textarea 
                    value={draftSettings.assistant?.system_prompt || ""}
                    onChange={(e) => updateDraft("assistant", "system_prompt", e.target.value)}
                    className="w-full h-24 p-4 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] text-[11px] text-[rgb(var(--foreground))] opacity-80 focus:opacity-600 focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all resize-none custom-scrollbar leading-relaxed"
                    placeholder="You are a helpful AI assistant..."
                  />
                </div>
                
                <div className="grid md:grid-cols-2 gap-6">
                  <div className="space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">Context Size</span>
                      <span className="text-[11px] font-mono opacity-60">{draftSettings.llm.ctx_size}</span>
                    </div>
                    <input 
                      type="range" 
                      min="512" max="8192" step="512"
                      value={draftSettings.llm.ctx_size}
                      onChange={(e) => updateDraft("llm", "ctx_size", Number(e.target.value))}
                      className="w-full"
                    />
                  </div>
                  <div className="space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">CPU Threads</span>
                      <span className="text-[11px] font-mono opacity-60">{draftSettings.llm.threads}</span>
                    </div>
                    <input 
                      type="range" 
                      min="1" max="16" step="1"
                      value={draftSettings.llm.threads}
                      onChange={(e) => updateDraft("llm", "threads", Number(e.target.value))}
                      className="w-full"
                    />
                  </div>
                </div>
              </div>
            )}

            {flippedView === 'asr' && (
              <div className="space-y-4">
                <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-80">Recognition Model</label>
                <div className="grid grid-cols-1 gap-3">
                    {modelCatalog.asr.map(m => (
                      <button
                        key={m.id}
                        onClick={() => updateDraft("asr", "model", m.id)}
                        className={cn(
                          "w-full text-left p-5 rounded-2xl transition-all duration-300 border",
                          draftSettings.asr.model === m.id
                            ? "bg-[rgb(var(--accent))]/[0.03] border-[rgb(var(--accent))]/30 shadow-sm"
                            : "bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/[0.05]"
                        )}
                      >
                        <div className="flex items-center justify-between mb-2">
                          <span className={cn(
                            "text-sm font-bold tracking-tight",
                            draftSettings.asr.model === m.id ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))] opacity-80"
                          )}>{m.name}</span>
                          {draftSettings.asr.model === m.id && (
                            <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgb(var(--accent))]" />
                          )}
                        </div>
                        <p className="text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))] opacity-60 font-medium">
                          {m.description}
                        </p>
                      </button>
                    ))}
                </div>
              </div>
            )}

            {flippedView === 'vad' && (
              <div className="space-y-6">
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">Activation Threshold</span>
                    <span className="text-[11px] font-mono opacity-60">{draftSettings.vad.threshold.toFixed(2)}</span>
                  </div>
                  <input 
                    type="range" 
                    min="0.1" max="0.9" step="0.05"
                    value={draftSettings.vad.threshold}
                    onChange={(e) => updateDraft("vad", "threshold", Number(e.target.value))}
                    className="w-full"
                  />
                </div>
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-80">Noise Gate (PTT)</span>
                    <span className="text-[11px] font-mono opacity-60">{draftSettings.vad.ptt_noise_gate.toFixed(3)}</span>
                  </div>
                  <input 
                    type="range" 
                    min="0.001" max="0.1" step="0.005"
                    value={draftSettings.vad.ptt_noise_gate}
                    onChange={(e) => updateDraft("vad", "ptt_noise_gate", Number(e.target.value))}
                    className="w-full"
                  />
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
});

const VoiceCard: React.FC = React.memo(() => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const voices = modelCatalog?.voices || [];
  const [activeTab, setActiveTab] = useState<"en" | "hi">("en");

  if (!draftSettings || !modelCatalog) return null;

  const filteredVoices = voices.filter(v => v.language === activeTab);

  const isSelected = (v: VoiceProfile) => {
    if (v.language === "en") return draftSettings.tts.en_voice === v.id;
    if (v.language === "hi") return draftSettings.tts.hi_voice === v.model_file;
    return false;
  };

  const handleSelect = (v: VoiceProfile) => {
    if (v.language === "en") {
      updateDraft("tts", "en_voice", v.id);
    } else if (v.language === "hi" && v.model_file) {
      updateDraft("tts", "hi_voice", v.model_file);
    }
  };

  // Helper for dynamic wave
  const getHeights = () => {
    const activeVoice = activeTab === "en" 
        ? voices.find(v => v.id === draftSettings.tts.en_voice)
        : voices.find(v => v.model_file === draftSettings.tts.hi_voice);
    
    const seed = activeVoice ? (activeVoice.id * 1337) % 100 : 42;
    const base = [12, 28, 48, 64, 52, 36, 72, 56, 28, 12];
    return base.map(h => Math.max(8, (h + seed) % 72));
  };

  const currentHeights = getHeights();

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col h-full">
      <div className="flex items-center justify-between mb-8 shrink-0">
        <div className="flex items-center gap-3">
          <Volume2 className="text-[rgb(var(--accent))]" size={20} />
          <div className="space-y-1">
            <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Assistant Voice</h2>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Select your preferred voice</p>
          </div>
        </div>

        {/* Language Tabs */}
        <div className="flex bg-[rgb(var(--foreground))]/[0.05] p-1 rounded-xl border border-[rgba(var(--border),0.05)]">
          {(["en", "hi"] as const).map((lang) => (
            <button
              key={lang}
              onClick={() => setActiveTab(lang)}
              className={cn(
                "px-4 py-1.5 rounded-lg text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                activeTab === lang 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/20" 
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/5"
              )}
            >
              {lang}
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 min-h-0">
        <div className="flex flex-wrap gap-2.5 mb-8">
          {filteredVoices.map((v) => (
            <button
              key={v.id}
              onClick={() => handleSelect(v)}
              className={cn(
                "px-2 py-2.5 rounded-l text-[11px] font-bold tracking-[0.15em] uppercase transition-all duration-300",
                isSelected(v)
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md scale-[1.02]" 
                  : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/10 hover:border-[rgba(var(--border),0.1)]"
              )}
            >
              {v.name}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-auto pt-4">
        <div className="h-16 w-full bg-transparent border border-[rgba(var(--border),0.03)] rounded-2xl flex items-center justify-center overflow-hidden">
          <div className="flex items-center gap-2">
            {currentHeights.map((h, i) => (
              <div 
                key={i} 
                className="w-2 rounded-full bg-[rgb(var(--accent))] transition-all duration-500 ease-out" 
                style={{ 
                  height: h, 
                  opacity: 0.4 + (h / 120),
                  animation: `pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite`,
                  animationDelay: `${i * 0.15}s` 
                }} 
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
});

const AppearanceCard: React.FC = React.memo(() => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();

  if (!draftSettings || !modelCatalog) return null;
  const theme = draftSettings.ui.theme;

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between mb-8 shrink-0">
        <div className="flex items-center gap-3">
          <Palette className="text-[rgb(var(--accent))]" size={20} />
          <div className="space-y-1">
            <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Appearance</h3>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-80">Accent Protocol</p>
          </div>
        </div>
        <div className="flex bg-[rgb(var(--foreground))]/[0.05] p-1 rounded-xl border border-[rgba(var(--border),0.05)]">
          {[
            { id: 'dark', icon: Moon },
            { id: 'light', icon: Sun }
          ].map((t) => (
            <button
              key={t.id}
              onClick={() => updateDraft("ui", "theme", t.id)}
              className={cn(
                "p-2 rounded-lg transition-all duration-300",
                theme === t.id 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <t.icon size={14} />
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 min-h-0 flex flex-col gap-8">
        {/* Accent Selection */}
        <div className="flex-1 min-h-0 flex items-center justify-center">
          <div className="animate-in fade-in zoom-in-95 duration-500 w-full">
            <div className="custom-color-picker-v2">
                <HexColorPicker 
                  color={draftSettings.ui.accent_seed} 
                  onChange={(color) => updateDraft("ui", "accent_seed", color)} 
                />
              </div>
            </div>
          </div>

        {/* Preview Section */}
        <div className="mt-auto p-4 rounded-2xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)] flex items-center justify-between">
          <div className="flex items-center gap-3">
             <div className="w-8 h-8 rounded-lg flex items-center justify-center bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/20">
                <Shield size={14} />
             </div>
             <div className="flex flex-col">
                <span className="text-[10px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Accent System</span>
                <span className="text-[9px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest">Active State</span>
             </div>
          </div>
          <div className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
            {draftSettings.ui.accent_seed.toUpperCase()}
          </div>
        </div>
      </div>
    </div>
  );
});

const InteractionCard: React.FC = React.memo(() => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  if (!draftSettings || !modelCatalog) return null;

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col h-full">
      <div className="flex items-center gap-3 mb-8 shrink-0">
        <MousePointerClick className="text-[rgb(var(--accent))]" size={20} />
        <div className="space-y-1">
          <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Interaction</h3>
          <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-80">How Vox engages with you</p>
        </div>
      </div>
      
      <div className="space-y-6 flex-1 min-h-0">
        <div className="flex items-center justify-between p-1 bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] rounded-2xl">
          <button 
            onClick={() => updateDraft("interaction", "main_app_mode", "Passive")}
            className={cn(
              "flex-1 px-4 py-2 rounded-xl text-[11px] font-bold uppercase transition-all duration-300",
              draftSettings.interaction.main_app_mode === "Passive" 
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
            )}
          >
            Always On
          </button>
          <button 
            onClick={() => updateDraft("interaction", "main_app_mode", "PTT")}
            className={cn(
              "flex-1 px-4 py-2 rounded-xl text-[11px] font-bold uppercase transition-all duration-300",
              draftSettings.interaction.main_app_mode === "PTT" 
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
            )}
          >
            Manual
          </button>
        </div>
        
        <p className="text-[14px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-80 italic pb-2">
          {draftSettings.interaction.main_app_mode === "Passive" 
            ? "Vox listens continuously and responds when you speak."
            : "Vox only processes speech when you click the record button."}
        </p>
      </div>
    </div>
  );
});

// ─── Main Component ───────────────────────────────────────────────────────────

export const CoreSettings: React.FC = () => {
  const { draftSettings, modelCatalog } = useSettings();

  if (!draftSettings || !modelCatalog) return null;

  return (
    <div className="h-full overflow-y-auto lg:overflow-hidden custom-scrollbar pr-1 -mr-1">
      <div className="lg:h-full flex flex-col">
        <div className="grid lg:grid-cols-3 gap-8 h-full items-stretch pb-10">
          {/* Intelligence Layer */}
          <div className="lg:col-span-2 flex flex-col gap-8 min-h-0">
            <div className="flex-[1.5] min-h-[400px] lg:min-h-0">
              <EngineCard />
            </div>
            <div className="flex-1 min-h-[300px] lg:min-h-0">
              <VoiceCard />
            </div>
          </div>

          {/* Sidebar Settings Column */}
          <div className="flex flex-col gap-8 min-h-0">
            <div className="flex-[2] min-h-[450px] lg:min-h-0">
              <AppearanceCard />
            </div>
            <div className="flex-1 min-h-[250px] lg:min-h-0">
              <InteractionCard />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
