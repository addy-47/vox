import { memo, useState, useEffect, useCallback } from "react";
import { checkModelExists } from "@/services/modelService";
import { useSettings } from "@/shared/context/SettingsContext";
import { Brain, Sparkles, Volume2, AlertTriangle } from "lucide-react";

/** Extract a compact single-word model identifier from a display name. */
const compactModelName = (name: string): string => {
  if (!name) return "—";
  if (name.length <= 10) return name;

  const tokens = name.split(/[\s-]+/);
  const fillers = new Set(["instruct", "8b", "7b", "3b", "1b", "13b", "70b", "asr", "v2", "v3", "text", "base", "small", "large", "medium", "chat", "hf", "gguf", "q4", "q8", "fp16", "int8", "int4"]);
  const meaningful = tokens.filter(t => !fillers.has(t.toLowerCase()));
  
  if (meaningful.length === 0) return name.slice(0, 10);

  const first = meaningful[0];
  const version = tokens.find(t => /^[\d.]+$/.test(t)) || "";
  return version ? `${first}${version}` : first;
};

export const ModelStatusOverlay = memo(() => {
  const { draftSettings, modelCatalog } = useSettings();
  const [presence, setPresence] = useState<Record<string, boolean>>({});
  const [vw, setVw] = useState(typeof window !== "undefined" ? window.innerWidth : 1200);

  useEffect(() => {
    const handleResize = () => setVw(window.innerWidth);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const isNarrow = vw < 1200;

  const llmId = draftSettings?.llm?.model || "";
  const asrId = draftSettings?.asr?.model || "";
  const ttsKind = draftSettings?.tts?.provider?.kind || "";

  const checkPresence = useCallback(async () => {
    if (!draftSettings) return;
    const items = [llmId, asrId].filter(Boolean);
    const results: Record<string, boolean> = {};

    for (const id of items) {
      try {
        results[id] = await checkModelExists(id);
      } catch {
        results[id] = false;
      }
    }
    setPresence(results);
  }, [llmId, asrId, draftSettings]);

  useEffect(() => {
    checkPresence();
  }, [checkPresence]);

  if (!draftSettings || !modelCatalog) return null;

  // Direct dynamic lookups from catalog using saved IDs in draftSettings
  const activeLlm = modelCatalog.llm.find((m) => m.id === llmId) || modelCatalog.llm[0];
  const activeAsr = modelCatalog.asr.find((m) => m.id === asrId) || modelCatalog.asr[0];
  const activeTts = modelCatalog.tts.find((m) => m.id === ttsKind || m.id.includes(ttsKind)) || modelCatalog.tts[0];
  const activeVoice = modelCatalog.voices.find((v) => v.id === draftSettings.tts.voice) || modelCatalog.voices[0];

  const llmExists = presence[llmId] ?? true;
  const asrExists = presence[asrId] ?? true;

  const llmName = isNarrow && activeLlm ? compactModelName(activeLlm.name) : activeLlm?.name;
  const asrName = isNarrow && activeAsr ? compactModelName(activeAsr.name) : activeAsr?.name;
  const ttsName = isNarrow && activeTts ? compactModelName(activeTts.name) : activeTts?.name;

  return (
    <div
      className="flex items-center text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/60 select-none"
      style={{ gap: isNarrow ? "clamp(0.25rem, 2vw, 0.75rem)" : "1.5rem" }}
    >
      {/* LLM Status Chip */}
      {activeLlm && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
          {llmExists ? (
            <Brain size={isNarrow ? 12 : 16} className="text-[rgb(var(--accent))]/80 shrink-0" />
          ) : (
            <AlertTriangle size={isNarrow ? 12 : 16} className="text-yellow-500 animate-pulse shrink-0" />
          )}
          <div className="min-w-0 overflow-hidden">
            <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate">
              {llmName}
              {!llmExists && <span className="text-[9px] text-yellow-500 font-bold uppercase tracking-wide leading-none shrink-0">Missing</span>}
            </div>
            {!isNarrow && (
              <div className="text-[9px] font-mono mt-0.5 leading-none truncate">{activeLlm.parameters || "LLM"}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeLlm.name}</p>
            {activeLlm.description}
            {activeLlm.tradeoffs && <p className="mt-1.5 pt-1.5 border-t border-[rgba(var(--accent),0.06)] text-[11px] opacity-75">{activeLlm.tradeoffs}</p>}
            {!llmExists && <p className="mt-1.5 text-yellow-500 font-semibold text-[11px]">⚠️ This model file is not downloaded yet.</p>}
          </div>
        </div>
      )}

      {/* ASR Status Chip */}
      {activeAsr && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
          {asrExists ? (
            <Sparkles size={isNarrow ? 12 : 16} className="text-[rgb(var(--accent))]/80 shrink-0" />
          ) : (
            <AlertTriangle size={isNarrow ? 12 : 16} className="text-yellow-500 animate-pulse shrink-0" />
          )}
          <div className="min-w-0 overflow-hidden">
            <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate">
              {asrName}
              {!asrExists && <span className="text-[9px] text-yellow-500 font-bold uppercase tracking-wide leading-none shrink-0">Missing</span>}
            </div>
            {!isNarrow && (
              <div className="text-[9px] font-mono mt-0.5 leading-none truncate">{activeAsr.parameters || "ASR"}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeAsr.name}</p>
            {activeAsr.description}
            {!asrExists && <p className="mt-1.5 text-yellow-500 font-semibold text-[11px]">⚠️ This model file is not downloaded yet.</p>}
          </div>
        </div>
      )}

      {/* TTS Status Chip */}
      {activeTts && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
          <Volume2 size={isNarrow ? 12 : 16} className="text-[rgb(var(--accent))]/80 shrink-0" />
          <div className="min-w-0 overflow-hidden">
            <div className="font-bold text-[rgb(var(--foreground))]/70 leading-none truncate flex items-center gap-1">
              {ttsName}
            </div>
            {!isNarrow && activeVoice && (
              <div className="text-[9px] font-mono mt-0.5 leading-none truncate">{activeVoice.name} · Voice</div>
            )}
          </div>
          {/* Tooltip */}
          <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeTts.name}{activeVoice ? `: ${activeVoice.name}` : ""}</p>
            {activeTts.description}
          </div>
        </div>
      )}
    </div>
  );
});

ModelStatusOverlay.displayName = "ModelStatusOverlay";

