import { memo, useState, useEffect, useCallback } from "react";
import { checkModelExists } from "@/services/setupService";
import { useSettingsStore } from "@/store/settingsStore";
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

/** Truncate a label with ellipsis when it exceeds maxLen characters */
const truncateWithEllipsis = (str: string, maxLen = 16): string => {
  if (!str) return "—";
  if (str.length <= maxLen) return str;
  return `${str.slice(0, maxLen)}...`;
};

export const ModelStatusOverlay = memo(() => {
  // Atomic selectors, not the whole settings context: this overlay is mounted
  // app-wide, so a whole-context destructure re-rendered it (and re-fired the
  // presence check below) on every settings keystroke.
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const activeSettings = settings || draftSettings;
  const [presence, setPresence] = useState<Record<string, boolean>>({});
  const [vw, setVw] = useState(typeof window !== "undefined" ? window.innerWidth : 1200);

  useEffect(() => {
    const handleResize = () => setVw(window.innerWidth);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const isNarrow = vw < 1200;

  // E3: on touch/coarse-pointer devices these hover-only tooltips are
  // unreachable — add tap-to-toggle on the chip itself (desktop keeps hover).
  const isCoarse =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(pointer: coarse)").matches;
  const [tipOpen, setTipOpen] = useState(false);

  const llmCategory = activeSettings?.llm?.active?.category;
  const sttCategory = activeSettings?.stt?.active?.category;
  const isRemoteLlm = llmCategory === "server" || llmCategory === "cloud";
  const isCloudStt = sttCategory === "cloud";

  const llmId = activeSettings?.llm?.active?.model || "";

  const asrId = activeSettings?.stt?.active?.model || "";

  const ttsKind = activeSettings?.tts?.active?.model || "";

  // Keyed ONLY on the model identities that presence actually depends on.
  // Previously this callback closed over the whole settings object, so every
  // settings change (each persona keystroke, each slider tick) re-fired two
  // sequential checkModelExists IPCs.
  const checkPresence = useCallback(async () => {
    const items = [
      !isRemoteLlm ? llmId : "",
      !isCloudStt ? asrId : "",
    ].filter(Boolean);
    if (items.length === 0) {
      setPresence({});
      return;
    }
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
  }, [llmId, asrId, isRemoteLlm, isCloudStt]);

  useEffect(() => {
    void checkPresence();
  }, [checkPresence]);

  if (!activeSettings || !modelCatalog) return null;

  const activeLlmKind = llmCategory ?? "embedded";
  const activeSttKind = sttCategory ?? "embedded";

  const catalogLlm = modelCatalog.llm.find((m) => m.id === llmId);

  const llmServerEntry = activeSettings?.providers?.llm?.servers?.find(
    (s) => s.id === activeSettings?.llm?.active?.provider
  );

  const llmDisplayName =
    activeLlmKind === "server"
      ? llmId || "Server LLM"
      : activeLlmKind === "cloud"
        ? llmId || "Cloud LLM"
        : catalogLlm?.name || llmId || "Embedded LLM";
  const llmSubtitle =
    activeLlmKind === "server"
      ? llmServerEntry?.name || "Remote"
      : activeLlmKind === "cloud"
        ? activeSettings?.llm?.active?.provider || "Cloud API"
        : catalogLlm?.parameters || "On-Device GGUF";

  const activeAsr = modelCatalog.stt.find((m) => m.id === asrId) || modelCatalog.stt[0];
  const asrDisplayName =
    activeSttKind === "cloud" ? asrId || "Cloud STT" : activeAsr?.name || asrId || "Nemotron";
  const asrSubtitle =
    activeSttKind === "cloud"
      ? activeSettings?.stt?.active?.provider || "Cloud API"
      : activeAsr?.parameters || "ASR";

  const activeTts = modelCatalog.tts.find((m) => m.id === ttsKind) || modelCatalog.tts[0];
  const activeVoiceName = modelCatalog.active_voice_name;
  const ttsDisplayName = activeTts?.name || ttsKind;
  const ttsSubtitle = activeVoiceName || activeTts?.parameters || "";

  const llmExists = isRemoteLlm ? true : (presence[llmId] ?? true);
  const asrExists = isCloudStt ? true : (presence[asrId] ?? true);

  const maxLen = isNarrow ? 12 : 16;
  const rawLlm = isNarrow ? compactModelName(llmDisplayName) : llmDisplayName;
  const rawAsr = isNarrow ? compactModelName(asrDisplayName) : asrDisplayName;
  const rawTts = isNarrow && activeTts ? compactModelName(ttsDisplayName) : ttsDisplayName;

  const llmName = truncateWithEllipsis(rawLlm, maxLen);
  const asrName = truncateWithEllipsis(rawAsr, maxLen);
  const ttsName = truncateWithEllipsis(rawTts, maxLen);

  const subMaxLen = isNarrow ? 14 : 20;
  const safeLlmSub = truncateWithEllipsis(llmSubtitle, subMaxLen);
  const safeAsrSub = truncateWithEllipsis(asrSubtitle, subMaxLen);
  const safeTtsSub = truncateWithEllipsis(ttsSubtitle, subMaxLen);

  return (
    <div
      className="flex items-center text-[12px] leading-relaxed text-[rgb(var(--foreground-muted))]/60 select-none"
      style={{ gap: isNarrow ? "clamp(0.35rem, 2vw, 0.85rem)" : "1.75rem" }}
    >
      {/* 1. Listening (ASR) Status Chip */}
      {(activeAsr || activeSttKind === "cloud") && (
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1"
        onClick={isCoarse ? () => setTipOpen((v) => !v) : undefined}>
          {asrExists ? (
            <Ear size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
          ) : (
            <AlertTriangle size={isNarrow ? 13 : 17} className="text-[rgb(var(--danger))] shrink-0" />
          )}
          <div className="min-w-0 overflow-hidden">
            <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate max-w-[140px] sm:max-w-[180px]">
              {asrName}
              {!asrExists && <span className="text-[11px] text-[rgb(var(--danger))] font-bold uppercase tracking-wide leading-none shrink-0">{MODEL_HUB_COPY.missing}</span>}
            </div>
            {!isNarrow && safeAsrSub && (
              <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate max-w-[140px] sm:max-w-[180px]">{safeAsrSub}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className={`absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80 ${isCoarse && tipOpen ? "!opacity-100 !scale-100" : ""}`}>
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
      <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1"
        onClick={isCoarse ? () => setTipOpen((v) => !v) : undefined}>
        {llmExists ? (
          <BrainCircuit size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
        ) : (
          <AlertTriangle size={isNarrow ? 13 : 17} className="text-[rgb(var(--danger))] shrink-0" />
        )}
        <div className="min-w-0 overflow-hidden">
          <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none flex items-center gap-1 truncate max-w-[140px] sm:max-w-[180px]">
            {llmName}
            {!llmExists && <span className="text-[11px] text-[rgb(var(--danger))] font-bold uppercase tracking-wide leading-none shrink-0">{MODEL_HUB_COPY.missing}</span>}
          </div>
          {!isNarrow && safeLlmSub && (
            <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate max-w-[140px] sm:max-w-[180px]">{safeLlmSub}</div>
          )}
        </div>
        {/* Tooltip */}
        <div className={`absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80 ${isCoarse && tipOpen ? "!opacity-100 !scale-100" : ""}`}>
          <p className="font-bold text-[rgb(var(--foreground))] mb-1">{llmDisplayName}</p>
          {catalogLlm?.description || MODEL_HUB_COPY.remoteLlmDesc}
          {!catalogLlm && llmId && (
            <p className="mt-1.5 pt-1.5 border-t border-[rgba(var(--accent),0.06)] text-[11px] font-mono opacity-75">
              {llmId}
            </p>
          )}
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
        <div className="flex items-center gap-1.5 group relative cursor-help min-w-0 shrink-1"
        onClick={isCoarse ? () => setTipOpen((v) => !v) : undefined}>
          <AudioLines size={isNarrow ? 13 : 17} className="text-[rgb(var(--accent))]/80 shrink-0" />
          <div className="min-w-0 overflow-hidden">
            <div className="text-[12.5px] font-bold text-[rgb(var(--foreground))]/70 leading-none truncate flex items-center gap-1 max-w-[140px] sm:max-w-[180px]">
              {ttsName}
            </div>
            {!isNarrow && safeTtsSub && (
              <div className="text-[11.5px] font-mono mt-0.5 leading-none truncate max-w-[140px] sm:max-w-[180px]">{safeTtsSub}</div>
            )}
          </div>
          {/* Tooltip */}
          <div className={`absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none w-56 p-3 rounded-xl bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-xl z-50 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))]/80 ${isCoarse && tipOpen ? "!opacity-100 !scale-100" : ""}`}>
            <p className="font-bold text-[rgb(var(--foreground))] mb-1">{activeTts.name}{activeVoiceName ? `: ${activeVoiceName}` : ""}</p>
            {activeTts.description}
          </div>
        </div>
      )}
    </div>
  );
});

ModelStatusOverlay.displayName = "ModelStatusOverlay";
