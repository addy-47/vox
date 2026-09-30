import { memo, useState, useEffect, useCallback } from "react";
import { checkModelExists } from "@/services/setupService";
import { getProviderCaps } from "@/services/settingsService";
import type { ProviderCaps } from "@/store/settingsStore";
import { useSettings } from "@/shared/hooks/useSettings";
import { Ear, BrainCircuit, AudioLines, AlertTriangle, AlertCircle } from "lucide-react";
import { MODEL_HUB_COPY } from "@/data/settingsCopy";

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
  const { settings, draftSettings, modelCatalog } = useSettings();
  const activeSettings = settings || draftSettings;
  const [presence, setPresence] = useState<Record<string, boolean>>({});
  const [, setTtsCaps] = useState<ProviderCaps | null>(null);
  const [vw, setVw] = useState(typeof window !== "undefined" ? window.innerWidth : 1200);

  useEffect(() => {
    const handleResize = () => setVw(window.innerWidth);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const isNarrow = vw < 1200;

  const isRemoteLlm = activeSettings?.llm?.active === "server" || activeSettings?.llm?.active === "cloud";
  const isCloudStt = activeSettings?.stt?.active === "cloud";

  const llmId =
    activeSettings?.llm?.active === "embedded"
      ? activeSettings?.llm?.embedded?.model || ""
      : activeSettings?.llm?.active === "server"
      ? activeSettings?.llm?.server?.model || ""
      : activeSettings?.llm?.cloud?.model || "";

  const asrId =
    activeSettings?.stt?.active === "embedded"
      ? activeSettings?.stt?.embedded?.model || ""
      : activeSettings?.stt?.cloud?.model || "";

  const ttsKind = activeSettings?.tts?.active || "";

  const checkPresence = useCallback(async () => {
    if (!activeSettings) return;
    const items = [
      !isRemoteLlm ? llmId : "",
      !isCloudStt ? asrId : "",
    ].filter(Boolean);
    const results: Record<string, boolean> = {};

    for (const id of items) {
      try {
        const res = await checkModelExists(id);
        results[id] = res;
      } catch {
        results[id] = false;
      }
    }
    setPresence(results);
  }, [llmId, asrId, isRemoteLlm, isCloudStt, activeSettings]);

  useEffect(() => {
    checkPresence();
  }, [checkPresence]);

  useEffect(() => {
    let isMounted = true;
    if (!ttsKind) {
      setTtsCaps(null);
      return;
    }
    getProviderCaps(ttsKind)
      .then((caps) => {
        if (isMounted) setTtsCaps(caps);
      })
      .catch(() => {
        if (isMounted) setTtsCaps(null);
      });

    return () => {
      isMounted = false;
    };
  }, [ttsKind]);

  if (!activeSettings || !modelCatalog) return null;

  const activeLlmKind = activeSettings?.llm?.active || "embedded";
  const activeSttKind = activeSettings?.stt?.active || "embedded";

  const catalogLlm = modelCatalog.llm.find((m) => m.id === llmId);

  let llmDisplayName = "";
  let llmSubtitle = "";

  if (activeLlmKind === "server") {
    llmDisplayName = activeSettings?.llm?.server?.model || "Server LLM";
    llmSubtitle = activeSettings?.llm?.server?.provider_name || "Ollama / Remote";
  } else if (activeLlmKind === "cloud") {
    llmDisplayName = activeSettings?.llm?.cloud?.model || "Cloud LLM";
    llmSubtitle = activeSettings?.llm?.cloud?.provider_name || "Cloud API";
  } else {
    llmDisplayName = catalogLlm?.name || activeSettings?.llm?.embedded?.model || "Embedded LLM";
    llmSubtitle = catalogLlm?.parameters || "On-Device GGUF";
  }

  const activeAsr = modelCatalog.stt.find((m) => m.id === asrId) || modelCatalog.stt[0];
  let asrDisplayName = "";
  let asrSubtitle = "";

  if (activeSttKind === "cloud") {
    asrDisplayName = activeSettings?.stt?.cloud?.model || "Cloud STT";
    asrSubtitle = activeSettings?.stt?.cloud?.provider || "Cloud API";
  } else {
    asrDisplayName = activeAsr?.name || asrId || "Nemotron";
    asrSubtitle = activeAsr?.parameters || "ASR";
  }

  const activeTts = modelCatalog.tts.find((m) => m.id === ttsKind) || modelCatalog.tts[0];
  const activeVoiceName = modelCatalog.active_voice_name;
  const ttsDisplayName = activeTts?.name || ttsKind;
  const ttsSubtitle = activeVoiceName || activeTts?.parameters || "";

  const llmExists = isRemoteLlm ? true : (presence[llmId] ?? true);
  const asrExists = isCloudStt ? true : (presence[asrId] ?? true);

  const llmName = isNarrow ? compactModelName(llmDisplayName) : llmDisplayName;
  const asrName = isNarrow ? compactModelName(asrDisplayName) : asrDisplayName;
  const ttsName = isNarrow && activeTts ? compactModelName(ttsDisplayName) : ttsDisplayName;

  return (
    <div
      className="flex items-center text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]/60 select-none"
      style={{ gap: isNarrow ? "clamp(0.35rem, 2vw, 0.85rem)" : "1.75rem" }}
    >
      {/* 1. Listening (ASR) Status Chip */}
      {(activeAsr || activeSttKind === "cloud") && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
          {asrExists ? (
            <Ear size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
          ) : (
            <AlertTriangle size={isNarrow ? 13 : 17} className="text-[rgb(var(--danger))] shrink-0" />
          )}
          <div className="min-w-0 overflow-hidden">
            <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate">
              {asrName}
              {!asrExists && <span className="text-[11px] text-[rgb(var(--danger))] font-bold uppercase tracking-wide leading-none shrink-0">{MODEL_HUB_COPY.missing}</span>}
            </div>
            {!isNarrow && asrSubtitle && (
              <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate">{asrSubtitle}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{asrDisplayName}</p>
            {activeAsr?.description || "Speech recognition model"}
            {!asrExists && (
              <p className="mt-1.5 text-[rgb(var(--danger))] font-semibold text-[11px] flex items-center gap-1">
                <AlertCircle size={12} className="shrink-0" /> {MODEL_HUB_COPY.notDownloadedDesc}
              </p>
            )}
          </div>
        </div>
      )}

      {/* 2. Reasoning (LLM) Status Chip */}
      <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
        {llmExists ? (
          <BrainCircuit size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
        ) : (
          <AlertTriangle size={isNarrow ? 13 : 17} className="text-[rgb(var(--danger))] shrink-0" />
        )}
        <div className="min-w-0 overflow-hidden">
          <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate">
            {llmName}
            {!llmExists && <span className="text-[11px] text-[rgb(var(--danger))] font-bold uppercase tracking-wide leading-none shrink-0">{MODEL_HUB_COPY.missing}</span>}
          </div>
          {!isNarrow && llmSubtitle && (
            <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate">{llmSubtitle}</div>
          )}
        </div>
        {/* Tooltip */}
        <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
          <p className="font-bold text-[rgb(var(--foreground))] mb-1">{llmDisplayName}</p>
          {catalogLlm?.description || MODEL_HUB_COPY.remoteLlmDesc}
          {!catalogLlm && llmId && (
            <p className="mt-1.5 pt-1.5 border-t border-[rgba(var(--accent),0.06)] text-[11px] font-mono opacity-75">
              {llmId}
            </p>
          )}
Addy is a systems and product engineer actively developing Vox, a voice-first local AI assistant. He values high performance, clean software architecture, and zero-bullshit engineering with low latency constraints.
          {catalogLlm?.tradeoffs && <p className="mt-1.5 pt-1.5 border-t border-[rgba(var(--accent),0.06)] text-[11px] opacity-75">{catalogLlm.tradeoffs}</p>}
          {!llmExists && (
            <p className="mt-1.5 text-[rgb(var(--danger))] font-semibold text-[11px] flex items-center gap-1">
              <AlertCircle size={12} className="shrink-0" /> {MODEL_HUB_COPY.notDownloadedDesc}
            </p>
          )}
        </div>
      </div>

      {/* 3. Speaking (TTS) Status Chip */}
      {activeTts && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1">
          <AudioLines size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
          <div className="min-w-0 overflow-hidden">
            <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none truncate flex items-center gap-1">
              {ttsName}
            </div>
            {!isNarrow && ttsSubtitle && (
              <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate">{ttsSubtitle}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80">
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeTts.name}{activeVoiceName ? `: ${activeVoiceName}` : ""}</p>
            {activeTts.description}
          </div>
        </div>
      )}
    </div>
  );
});

ModelStatusOverlay.displayName = "ModelStatusOverlay";
