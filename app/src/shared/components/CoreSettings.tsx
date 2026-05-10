import React, { useState } from "react";
import { Brain, Volume2, Palette, Cpu, MemoryStick, ChevronRight, ChevronLeft } from "lucide-react";
import { HexColorPicker } from "react-colorful";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";

// ─── Sub-Components (Isolated Cards) ──────────────────────────────────────────

const EngineCard: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [flippedView, setFlippedView] = useState<'llm' | 'asr' | 'vad' | null>(null);
  
  if (!draftSettings || !modelCatalog) return null;

  const activeLlm = modelCatalog.llm.find(m => m.id === draftSettings.llm.model) || modelCatalog.llm[0];
  const activeAsr = modelCatalog.asr.find(m => m.id === draftSettings.asr.model) || modelCatalog.asr[0];

  return (
    <div className="relative" style={{ perspective: '1000px' }}>
      <div 
        className="w-full relative transition-transform duration-500"
        style={{ 
          transformStyle: 'preserve-3d', 
          transform: flippedView ? 'rotateY(180deg)' : 'rotateY(0deg)' 
        }}
      >
        {/* Front Face */}
        <div 
          className="premium-card p-6 md:p-8 flex flex-col"
          style={{ backfaceVisibility: 'hidden' }}
        >
          {/* Header */}
          <div className="flex items-center justify-between mb-8 shrink-0">
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Core Engine</h2>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Brain & Reasoning</p>
            </div>
            <Brain className="text-[rgb(var(--accent))]" size={20} />
          </div>

          {/* Body */}
          <div className="space-y-8 overflow-y-auto custom-scrollbar pr-1 -mr-1">
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                  <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Active Model</label>
                  <div className="flex gap-4">
                    <div className="flex items-center gap-1.5 text-[10px] font-mono text-[rgb(var(--accent))] opacity-60">
                        <Cpu size={12} />
                        {activeLlm.parameters}
                    </div>
                    <div className="flex items-center gap-1.5 text-[10px] font-mono text-[rgb(var(--foreground))] opacity-40">
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
                      className="w-full text-left p-5 rounded-2xl transition-all duration-300 border bg-white/[0.02] border-[rgba(var(--border),0.05)] hover:bg-white/[0.05]"
                    >
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-sm font-bold tracking-tight text-[rgb(var(--foreground))] opacity-80">{m.name}</span>
                      </div>
                      <p className="text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))] opacity-50 font-medium">
                        {m.description}
                      </p>
                    </button>
                  ))}
              </div>
            </div>

            <div className="grid md:grid-cols-2 gap-6 pb-2">
              <div className="space-y-3">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">Speech Recognition</div>
                <button 
                  onClick={() => setFlippedView('asr')}
                  className="w-full text-left p-4 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.05)] hover:bg-white/[0.05] transition-colors"
                >
                  <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80 mb-1">{activeAsr.name}</div>
                  <div className="text-[10px] text-[rgb(var(--foreground-muted))] opacity-40">{activeAsr.description}</div>
                </button>
              </div>
              <div className="space-y-3">
                <div className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">Voice Detection</div>
                <button 
                  onClick={() => setFlippedView('vad')}
                  className="w-full text-left p-4 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.05)] hover:bg-white/[0.05] transition-colors"
                >
                  <div className="text-sm font-bold text-[rgb(var(--foreground))] opacity-80 mb-1">TenVAD</div>
                  <div className="text-[10px] text-[rgb(var(--foreground-muted))] opacity-40">Filters background noise for clear triggers</div>
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
              className="w-8 h-8 rounded-full bg-white/[0.03] border border-white/5 flex items-center justify-center hover:bg-white/10 transition-colors"
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
          <div className="space-y-8 overflow-y-auto custom-scrollbar pr-1 -mr-1 flex-1">
            {flippedView === 'llm' && (
              <div className="space-y-6">
                <div className="space-y-3">
                  <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">System Prompt</label>
                  <textarea 
                    value={draftSettings.assistant?.system_prompt || ""}
                    onChange={(e) => updateDraft("assistant", "system_prompt", e.target.value)}
                    className="w-full h-24 p-4 rounded-xl bg-white/[0.03] border border-[rgba(var(--border),0.05)] text-[11px] text-[rgb(var(--foreground))] opacity-80 focus:opacity-100 focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all resize-none custom-scrollbar leading-relaxed"
                    placeholder="You are a helpful AI assistant..."
                  />
                </div>
                
                <div className="grid md:grid-cols-2 gap-6">
                  <div className="space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">Context Size</span>
                      <span className="text-[10px] font-mono opacity-60">{draftSettings.llm.ctx_size}</span>
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
                      <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">CPU Threads</span>
                      <span className="text-[10px] font-mono opacity-60">{draftSettings.llm.threads}</span>
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
                <label className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Recognition Model</label>
                <div className="grid grid-cols-1 gap-3">
                    {modelCatalog.asr.map(m => (
                      <button
                        key={m.id}
                        onClick={() => updateDraft("asr", "model", m.id)}
                        className={cn(
                          "w-full text-left p-5 rounded-2xl transition-all duration-300 border",
                          draftSettings.asr.model === m.id
                            ? "bg-[rgb(var(--accent))]/[0.03] border-[rgb(var(--accent))]/30 shadow-sm"
                            : "bg-white/[0.02] border-[rgba(var(--border),0.05)] hover:bg-white/[0.05]"
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
                        <p className="text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))] opacity-50 font-medium">
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
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">Activation Threshold</span>
                    <span className="text-[10px] font-mono opacity-60">{draftSettings.vad.threshold.toFixed(2)}</span>
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
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-40">Noise Gate (PTT)</span>
                    <span className="text-[10px] font-mono opacity-60">{draftSettings.vad.ptt_noise_gate.toFixed(3)}</span>
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
};

const VoiceCard: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  if (!draftSettings || !modelCatalog) return null;

  const VOICE_PROFILES = modelCatalog.voices || [];

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col">
      <div className="flex items-center justify-between mb-8 shrink-0">
        <div className="space-y-1">
          <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Assistant Voice</h2>
          <p className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest opacity-60">Select your preferred voice</p>
        </div>
        <Volume2 className="text-[rgb(var(--accent))]" size={20} />
      </div>

      <div className="overflow-y-auto custom-scrollbar pr-1 -mr-1">
        <div className="flex flex-wrap gap-3 mb-8">
          {VOICE_PROFILES.map((v) => (
            <button
              key={v.id}
              onClick={() => updateDraft("tts", "voice_id", v.id)}
              className={cn(
                "px-5 py-2 rounded-xl text-[11px] font-bold tracking-[0.15em] uppercase transition-colors duration-300",
                draftSettings.tts.voice_id === v.id 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm" 
                  : "bg-white/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-white/10"
              )}
            >
              {v.name}
            </button>
          ))}
        </div>

        <div className="h-20 w-full bg-white/[0.02] border border-[rgba(var(--border),0.03)] rounded-2xl flex items-center justify-center overflow-hidden mb-2">
          <div className="flex items-center gap-1.5">
            {[4, 12, 24, 42, 32, 18, 48, 36, 12, 6].map((h, i) => (
              <div key={i} className="w-1 rounded-full bg-[rgb(var(--accent))]/40 animate-pulse" style={{ height: h, animationDelay: `${i * 0.1}s` }} />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

const AppearanceCard: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [showCustomPicker, setShowCustomPicker] = useState(false);
  if (!draftSettings || !modelCatalog) return null;

  const PRESET_COLORS = modelCatalog.preset_colors || [];

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col">
      <div className="flex items-center justify-between mb-8 shrink-0">
        <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Look & Feel</h3>
        <Palette className="text-[rgb(var(--accent))]" size={16} />
      </div>
      
      <div className="space-y-8 overflow-y-auto custom-scrollbar pr-1 -mr-1 pt-4">
        {/* Presets Grid */}
        <div className="flex flex-wrap justify-center gap-4">
          {PRESET_COLORS.map((color) => (
            <button 
              key={color}
              onClick={() => updateDraft("ui", "accent_seed", color)}
              className={cn(
                "w-10 h-10 rounded-full transition-all duration-300 relative group",
                draftSettings.ui.accent_seed.toLowerCase() === color.toLowerCase() 
                  ? "scale-110 ring-2 ring-[rgb(var(--accent))] ring-offset-4 ring-offset-transparent" 
                  : "hover:scale-105"
              )}
              style={{ backgroundColor: color }}
            >
              <div className="absolute inset-0 rounded-full bg-white opacity-0 group-hover:opacity-20 transition-opacity" />
            </button>
          ))}
          <button 
            onClick={() => setShowCustomPicker(!showCustomPicker)}
            className={cn(
              "px-3 py-2 rounded-xl flex lg:hidden items-center justify-center border border-dashed transition-all duration-300",
              showCustomPicker 
                ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5" 
                : "border-white/10 text-white/40 hover:border-white/20 hover:text-white/60"
            )}
            title="Custom Color"
          >
            <ChevronRight className={cn("transition-transform duration-300", showCustomPicker ? "rotate-90" : "")} size={16} />
            <span className="text-[10px] font-bold uppercase ml-2">Custom</span>
          </button>
        </div>

        {/* Custom Picker Section */}
        <div className={cn(
          "space-y-4 pt-4 border-t border-white/5",
          "lg:block",
          showCustomPicker ? "block animate-in fade-in slide-in-from-top-2 duration-300" : "hidden lg:block"
        )}>
          <div className="flex justify-center">
            <div className="custom-color-picker-v2 w-full max-w-[200px]">
              <HexColorPicker 
                color={draftSettings.ui.accent_seed} 
                onChange={(color) => updateDraft("ui", "accent_seed", color)} 
              />
            </div>
          </div>
        </div>

        <div className="pt-4 border-t border-white/5 pb-2">
          <div className="flex items-center justify-between text-[10px] text-[rgb(var(--foreground-muted))] opacity-40 uppercase tracking-widest font-bold">
            <span>Active Accent</span>
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full shadow-[0_0_8px_rgb(var(--accent))]" style={{ backgroundColor: draftSettings.ui.accent_seed }} />
              <span className="text-[rgb(var(--accent))]">{draftSettings.ui.accent_seed.toUpperCase()}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

const InteractionCard: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  if (!draftSettings || !modelCatalog) return null;

  return (
    <div className="premium-card p-6 md:p-8 flex flex-col">
      <div className="space-y-1 mb-8 shrink-0">
        <h3 className="text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.3em] opacity-40">Interaction</h3>
        <p className="text-[10px] text-[rgb(var(--foreground-muted))] opacity-30">How Vox engages with you</p>
      </div>
      
      <div className="space-y-6 overflow-y-auto custom-scrollbar pr-1 -mr-1">
        <div className="flex items-center justify-between p-1 bg-white/[0.03] border border-white/5 rounded-2xl">
          <button 
            onClick={() => updateDraft("interaction", "main_app_mode", "Passive")}
            className={cn(
              "flex-1 px-4 py-2 rounded-xl text-[10px] font-bold uppercase transition-all duration-300",
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
              "flex-1 px-4 py-2 rounded-xl text-[10px] font-bold uppercase transition-all duration-300",
              draftSettings.interaction.main_app_mode === "PTT" 
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
            )}
          >
            Manual
          </button>
        </div>
        
        <p className="text-[10px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-40 italic pb-2">
          {draftSettings.interaction.main_app_mode === "Passive" 
            ? "Vox listens continuously and responds when you speak."
            : "Vox only processes speech when you click the record button."}
        </p>
      </div>
    </div>
  );
};

// ─── Main Component ───────────────────────────────────────────────────────────

export const CoreSettings: React.FC = () => {
  const { draftSettings, modelCatalog } = useSettings();

  if (!draftSettings || !modelCatalog) return null;

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto custom-scrollbar pr-2 -mr-2">
        <div className="grid lg:grid-cols-3 gap-8 items-start pb-8">
          {/* Intelligence Layer */}
          <div className="lg:col-span-2 space-y-8">
            <EngineCard />
            <VoiceCard />
          </div>

          {/* Sidebar Settings Column */}
          <div className="space-y-8 sticky top-0">
            <AppearanceCard />
            <InteractionCard />
          </div>
        </div>
      </div>
    </div>
  );
};
