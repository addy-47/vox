import { memo, useState, useEffect, useCallback } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Brain, Sparkles, Volume2, AlertTriangle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

export const ModelStatusOverlay = memo(() => {
  const { draftSettings, modelCatalog } = useSettings();
  const [presence, setPresence] = useState<Record<string, boolean>>({});

  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const items = [
      "supertonic_tts",
      draftSettings.asr.model,
      draftSettings.llm.model
    ];
    const results: Record<string, boolean> = {};
    for (const id of items) {
      try {
        results[id] = await invoke<boolean>("check_model_exists", { modelId: id });
      } catch {
        results[id] = false;
      }
    }
    setPresence(results);
  }, [modelCatalog, draftSettings]);

  useEffect(() => {
    checkPresence();
  }, [checkPresence]);

  if (!draftSettings || !modelCatalog) return null;

  const activeLlm = modelCatalog.llm.find((m) => m.id === draftSettings.llm.model) || modelCatalog.llm[0];
  const activeAsr = modelCatalog.asr.find((m) => m.id === draftSettings.asr.model) || modelCatalog.asr[0];
  const activeVoice = modelCatalog.voices.find((v) => v.id === draftSettings.tts.voice) || modelCatalog.voices[0];

  const llmExists = presence[activeLlm.id] ?? true;
  const asrExists = presence[activeAsr.id] ?? true;
  const ttsExists = presence["supertonic_tts"] ?? true;

  return (
    <div className="flex items-center gap-12 text-[10px] leading-relaxed text-[rgb(var(--foreground-muted))]/60 select-none">
      {/* LLM Status Chip */}
      <div className="flex items-center gap-2 group relative cursor-help">
        {llmExists ? (
          <Brain size={13} className="text-[rgb(var(--accent))]/80" />
        ) : (
          <AlertTriangle size={13} className="text-yellow-500 animate-pulse" />
        )}
        <div>
          <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1">
            {activeLlm.name}
            {!llmExists && <span className="text-[7px] text-yellow-500 font-bold uppercase tracking-wide leading-none">Missing</span>}
          </div>
          <div className="text-[8px] font-mono mt-0.5 leading-none">{activeLlm.parameters} · LLM</div>
        </div>
        {/* Tooltip */}
        <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
          <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeLlm.name}</p>
          {activeLlm.description}
          {activeLlm.tradeoffs && <p className="mt-1.5 pt-1.5 border-t border-[rgba(var(--accent),0.06)] text-[10px] opacity-75">{activeLlm.tradeoffs}</p>}
          {!llmExists && <p className="mt-1.5 text-yellow-500 font-semibold text-[10px]">⚠️ This model file is not downloaded yet.</p>}
        </div>
      </div>

      {/* ASR Status Chip */}
      <div className="flex items-center gap-2 group relative cursor-help">
        {asrExists ? (
          <Sparkles size={13} className="text-[rgb(var(--accent))]/80" />
        ) : (
          <AlertTriangle size={13} className="text-yellow-500 animate-pulse" />
        )}
        <div>
          <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1">
            {activeAsr.name}
            {!asrExists && <span className="text-[7px] text-yellow-500 font-bold uppercase tracking-wide leading-none">Missing</span>}
          </div>
          <div className="text-[8px] font-mono mt-0.5 leading-none">ASR Engine</div>
        </div>
        {/* Tooltip */}
        <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
          <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeAsr.name}</p>
          {activeAsr.description}
          {!asrExists && <p className="mt-1.5 text-yellow-500 font-semibold text-[10px]">⚠️ This model file is not downloaded yet.</p>}
        </div>
      </div>

      {/* TTS Status Chip */}
      <div className="flex items-center gap-2 group relative cursor-help">
        {ttsExists ? (
          <Volume2 size={13} className="text-[rgb(var(--accent))]/80" />
        ) : (
          <AlertTriangle size={13} className="text-yellow-500 animate-pulse" />
        )}
        <div>
          <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1">
            Supertonic 3
            {!ttsExists && <span className="text-[7px] text-yellow-500 font-bold uppercase tracking-wide leading-none">Missing</span>}
          </div>
          <div className="text-[8px] font-mono mt-0.5 leading-none">{activeVoice.name} · Voice</div>
        </div>
        {/* Tooltip */}
        <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
          <p className="font-bold text-[rgb(var(--foreground))] mb-1">Supertonic 3: {activeVoice.name}</p>
          Multilingual flow-matching speech synthesizer.
          {!ttsExists && <p className="mt-1.5 text-yellow-500 font-semibold text-[10px]">⚠️ TTS engine assets are not downloaded yet.</p>}
        </div>
      </div>
    </div>
  );
});

ModelStatusOverlay.displayName = "ModelStatusOverlay";
