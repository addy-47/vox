import React, { useState, useEffect, useCallback, useMemo } from "react";
import { HexColorPicker } from "react-colorful";
import { cn } from "@/shared/lib/utils";
import { useSettings, VoiceProfile } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { 
  Brain, Volume2, Palette, MousePointerClick, 
  Sun, Moon, Shield, AlertCircle 
} from "lucide-react";

// ─── Sub-Components (Isolated Cards) ──────────────────────────────────────────

const EngineCard: React.FC = React.memo(() => {
  const { draftSettings, modelCatalog } = useSettings();
  const [presence, setPresence] = useState<Record<string, boolean>>({});
  
  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const items = ["ten_vad", draftSettings.asr.model, "vox_translit_rnn", draftSettings.llm.model];
    const results: Record<string, boolean> = {};
    for (const id of items) {
      try {
        results[id] = await invoke<boolean>("check_model_exists", { modelId: id });
      } catch {
        results[id] = false;
      }
    }
    results["earshot"] = true; // Always verified
    setPresence(results);
  }, [modelCatalog, draftSettings]);

  useEffect(() => {
    checkPresence();
  }, [checkPresence]);

  if (!draftSettings || !modelCatalog) return null;

  const activeLlm = modelCatalog.llm.find(m => m.id === draftSettings.llm.model) || modelCatalog.llm[0];
  const activeAsr = modelCatalog.asr.find(m => m.id === draftSettings.asr.model) || modelCatalog.asr[0];
  const isLlmVerified = presence[activeLlm.id];
  const isAsrVerified = presence[activeAsr.id];
  const isTranslitVerified = presence["vox_translit_rnn"];
  const isVadVerified = draftSettings.vad.vad_backend === "earshot" || presence["ten_vad"];

  return (
    <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] flex flex-col h-full overflow-hidden relative">
      {/* Header */}
      <div className="flex items-center gap-3 mb-8 shrink-0">
        <Brain className="text-[rgb(var(--accent))]" size={20} />
        <div className="space-y-1">
          <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Core Engine</h2>
          <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest font-bold opacity-80">Brain & Reasoning Topology</p>
        </div>
      </div>

      {/* Grid of 4 Pipeline Components */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 flex-1">
        
        {/* VAD Card */}
        <div 
          className={cn(
            "p-5 rounded-2xl border glass-whisper glass-base flex flex-col justify-between relative overflow-hidden transition-all duration-300"
          )}
        >
          {!isVadVerified && (
            <div className="absolute inset-0 z-10 bg-[rgb(var(--background))]/60 backdrop-blur-sm flex flex-col items-center justify-center p-4 text-center">
              <AlertCircle className="text-[rgb(var(--accent))] mb-1.5" size={20} />
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider mb-0.5">Model Missing</h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-95 max-w-[180px]">
                Silence detection weights are missing. Install in the Models tab.
              </p>
            </div>
          )}
          <div className="space-y-1">
            <div className="text-[13px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-95">Silence Detection</div>
            <div className="text-[13px] font-bold text-[rgb(var(--foreground))] mt-1">
              {draftSettings.vad.vad_backend === "ten_vad" ? "TenVAD" : "Earshot VAD"}
            </div>
          </div>
          <div className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-relaxed mt-2">
            {draftSettings.vad.vad_backend === "ten_vad" 
              ? "ONNX-based legacy noise filtering." 
              : "Sub-millisecond Rust voice detection."}
          </div>
        </div>

        {/* ASR Card */}
        <div 
          className={cn(
            "p-5 rounded-2xl border glass-whisper glass-base flex flex-col justify-between relative overflow-hidden transition-all duration-300"
          )}
        >
          {!isAsrVerified && (
            <div className="absolute inset-0 z-10 bg-[rgb(var(--background))]/60 backdrop-blur-sm flex flex-col items-center justify-center p-4 text-center">
              <AlertCircle className="text-[rgb(var(--accent))] mb-1.5" size={20} />
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider mb-0.5">Model Missing</h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-95 max-w-[180px]">
                Speech understanding weights are missing. Install in the Models tab.
              </p>
            </div>
          )}
          <div className="space-y-1">
            <div className="text-[13px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-95">Speech Recognition</div>
            <div className="text-[13px] font-bold text-[rgb(var(--foreground))] mt-1">
              {activeAsr.name}
            </div>
          </div>
          <div className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-relaxed mt-2">
            {activeAsr.description}
          </div>
        </div>

        {/* Translit Card */}
        <div 
          className={cn(
            "p-5 rounded-2xl border glass-whisper glass-base flex flex-col justify-between relative overflow-hidden transition-all duration-300"
          )}
        >
          {!isTranslitVerified && (
            <div className="absolute inset-0 z-10 bg-[rgb(var(--background))]/60 backdrop-blur-sm flex flex-col items-center justify-center p-4 text-center">
              <AlertCircle className="text-[rgb(var(--accent))] mb-1.5" size={20} />
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider mb-0.5">Model Missing</h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-95 max-w-[180px]">
                Roman Transliteration weights are missing. Install in the Models tab.
              </p>
            </div>
          )}
          <div className="space-y-1">
            <div className="text-[13px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-95">Roman Transliteration</div>
            <div className="text-[13px] font-bold text-[rgb(var(--foreground))] mt-1">
              Vox Hinglish RNN
            </div>
          </div>
          <div className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-relaxed mt-2">
            Devanagari ASR outputs cleanly transliterated to standard Hinglish syntax.
          </div>
        </div>

        {/* LLM Card */}
        <div 
          className={cn(
            "p-5 rounded-2xl border glass-whisper glass-base flex flex-col justify-between relative overflow-hidden transition-all duration-300"
          )}
        >
          {!isLlmVerified && (
            <div className="absolute inset-0 z-10 bg-[rgb(var(--background))]/60 backdrop-blur-sm flex flex-col items-center justify-center p-4 text-center">
              <AlertCircle className="text-[rgb(var(--accent))] mb-1.5" size={20} />
              <h3 className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider mb-0.5">Model Missing</h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-95 max-w-[180px]">
                AI Reasoning LLM weights are missing. Install in the Models tab.
              </p>
            </div>
          )}
          <div className="space-y-1">
            <div className="text-[13px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-[0.2em] opacity-95">Intelligence Layer</div>
            <div className="text-[13px] font-bold text-[rgb(var(--foreground))] mt-1 flex items-center gap-2">
              {activeLlm.name}
              <span className="text-[13px] font-mono opacity-80 font-normal">({activeLlm.parameters})</span>
            </div>
          </div>
          <div className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-relaxed mt-2">
            {activeLlm.description}
          </div>
        </div>

      </div>
    </div>
  );
});

const VoiceCard: React.FC = React.memo(() => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const voices = modelCatalog?.voices || [];
  const [isTtsVerified, setIsTtsVerified] = useState(true);

  const checkTtsPresence = useCallback(async () => {
    try {
      const ok = await invoke<boolean>("check_model_exists", { modelId: "supertonic_tts" });
      setIsTtsVerified(ok);
    } catch {
      setIsTtsVerified(false);
    }
  }, []);

  useEffect(() => {
    checkTtsPresence();
  }, [checkTtsPresence]);

  // Voice-dependent waveform heights — deterministic based on voice ID
  const activeVoice = voices.find(v => v.id === draftSettings?.tts.voice);
  const waveformHeights = useMemo(() => {
    const seed = activeVoice ? (activeVoice.id * 1337) % 100 : 42;
    const base = [12, 28, 48, 64, 52, 36, 72, 56, 28, 12];
    return base.map(h => Math.max(8, (h + seed) % 72));
  }, [activeVoice]);

  if (!draftSettings || !modelCatalog) return null;

  const isSelected = (v: VoiceProfile) => draftSettings.tts.voice === v.id;

  const handleSelect = (v: VoiceProfile) => {
    if (!isTtsVerified) return;
    updateDraft("tts", "voice", v.id);
  };

  return (
    <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] flex flex-col h-full relative overflow-hidden">
      {!isTtsVerified && (
        <div className="absolute inset-0 z-20 bg-[rgb(var(--background))]/60 backdrop-blur-md flex flex-col items-center justify-center p-6 text-center">
          <AlertCircle className="text-[rgb(var(--accent))] mb-3" size={28} />
          <h3 className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider mb-2">TTS Model Missing</h3>
          <p className="text-[13px] text-[rgb(var(--foreground-muted))] max-w-[240px] leading-relaxed opacity-90">
            Supertonic 3 speech engine is not installed. Manage voice models in the Models tab.
          </p>
        </div>
      )}

      <div className="flex items-center gap-3 mb-6 shrink-0">
        <Volume2 className="text-[rgb(var(--accent))]" size={20} />
        <div className="space-y-1">
          <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Assistant Voice</h2>
          <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest font-bold opacity-80">Supertonic 3 Multilingual</p>
        </div>
      </div>

      {/* Voice Selector */}
      <div className="flex-1 min-h-0">
        <div className="flex flex-wrap gap-2.5 mb-6">
          {voices.map((v) => (
            <button
              key={v.id}
              onClick={() => handleSelect(v)}
              disabled={!isTtsVerified}
              className={cn(
                "px-3 py-2.5 rounded-xl text-[13px] font-bold tracking-[0.1em] uppercase transition-all duration-300",
                isSelected(v)
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md scale-[1.02]" 
                  : "glass-whisper glass-base text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20",
                !isTtsVerified ? "opacity-50 cursor-not-allowed" : ""
              )}
            >
              {v.name}
            </button>
          ))}
        </div>
      </div>

      {/* Voice-dependent waveform */}
      <div className="mt-auto pt-4 animate-pulse">
        <div className="h-16 w-full glass-whisper glass-base rounded-2xl flex items-center justify-center overflow-hidden">
          <div className="flex items-center gap-2">
            {waveformHeights.map((h: number, i: number) => (
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
    <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between mb-8 shrink-0">
        <div className="flex items-center gap-3">
          <Palette className="text-[rgb(var(--accent))]" size={20} />
          <div className="space-y-1">
            <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Appearance</h3>
            <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest font-bold opacity-80">Accent Protocol</p>
          </div>
        </div>
        <div className="flex glass-whisper glass-base p-1 rounded-xl">
          {[
            { id: 'dark', icon: Moon },
            { id: 'light', icon: Sun }
          ].map((t) => (
            <button
              key={t.id}
              onClick={() => updateDraft("ui", "theme", t.id)}
              className={cn(
                "p-2.5 rounded-lg transition-all duration-300",
                theme === t.id 
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <t.icon size={16} />
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
        <div className="mt-auto p-4 rounded-2xl glass-whisper glass-base flex items-center justify-between">
          <div className="flex items-center gap-3">
             <div className="w-8 h-8 rounded-lg flex items-center justify-center bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-lg shadow-[rgb(var(--accent))]/25">
                <Shield size={16} />
             </div>
             <div className="flex flex-col">
                <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Accent System</span>
                <span className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest font-bold opacity-80">Active State</span>
             </div>
          </div>
          <div className="text-[13px] font-mono font-bold text-[rgb(var(--accent))]">
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
    <div className="bg-black/10 p-6 md:p-8 rounded-2xl border border-[rgba(var(--accent),0.05)] flex flex-col h-full">
      <div className="flex items-center gap-3 mb-8 shrink-0">
        <MousePointerClick className="text-[rgb(var(--accent))]" size={20} />
        <div className="space-y-1">
          <h3 className="text-lg font-bold text-[rgb(var(--foreground))]">Interaction</h3>
          <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-widest font-bold opacity-80">How Vox engages with you</p>
        </div>
      </div>
      
      <div className="space-y-6 flex-1 min-h-0">
        <div className="flex items-center justify-between p-1 glass-whisper glass-base rounded-2xl">
          <button 
            onClick={() => updateDraft("interaction", "main_app_mode", "Passive")}
            className={cn(
              "flex-1 px-4 py-2.5 rounded-xl text-[13px] font-bold uppercase transition-all duration-300",
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
              "flex-1 px-4 py-2.5 rounded-xl text-[13px] font-bold uppercase transition-all duration-300",
              draftSettings.interaction.main_app_mode === "PTT" 
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
            )}
          >
            Manual
          </button>
        </div>
        
        <p className="text-[13px] text-[rgb(var(--foreground-muted))] leading-relaxed opacity-95 italic pb-2">
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
