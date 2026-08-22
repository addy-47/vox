import { useState, useEffect, memo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { checkIfCloudUrl, CLOUD_PROVIDERS } from "@/data/providersCopy";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  Brain, Cloud, Network, Volume2, Sparkles, Mic,
  RefreshCw, AlertCircle, Clock
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField, UnderlineInput, CarouselSelector } from "@/shared/ui";

interface LlmConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: "local" | "remote" | "cloud";
  isModular: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const LlmConfigDesk = memo(({ activeCategory, activePill, isModular, layoutMode }: LlmConfigDeskProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);

  const [url, setUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [prevLlmProvider, setPrevLlmProvider] = useState<any>(null);

  const [remoteTtsEndpoint, setRemoteTtsEndpoint] = useState("");
  const [remoteTtsPath, setRemoteTtsPath] = useState("");

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);

  if (!draftSettings || !settings) return null;

  const activeLlmId =
    draftSettings.llm?.active === "embedded"
      ? draftSettings.llm?.embedded?.model
      : draftSettings.llm?.active === "server"
      ? draftSettings.llm?.server?.model
      : draftSettings.llm?.cloud?.model;
  const activeLlm = modelCatalog?.llm?.find((m) => m.id === activeLlmId) || modelCatalog?.llm?.[0];
  const activeLlmDescription = activeLlm?.description || "";

  const activeAsrId = draftSettings.stt?.embedded?.model;
  const activeAsr = modelCatalog?.asr?.find((m) => m.id === activeAsrId) || modelCatalog?.asr?.[0];
  const activeAsrDescription = activeAsr?.description || "";

  const activeTtsKind = draftSettings.tts?.active;
  const activeTts = modelCatalog?.tts?.find((m) => m.id === activeTtsKind || (activeTtsKind && m.id.includes(activeTtsKind))) || modelCatalog?.tts?.[0];
  const activeTtsDescription = activeTts?.description || "";

  const edgeTtsModel = modelCatalog?.tts?.find((m) => m.id === "edge_tts");
  const edgeTtsDescription = edgeTtsModel?.description || "";

  const activeLlmProvider = draftSettings.llm.active || "embedded";
  const currentRemoteConfig = activeLlmProvider === "server" ? draftSettings.llm.server : activeLlmProvider === "cloud" ? draftSettings.llm.cloud : null;
  const currentProvider: LlmProviderConfig = activeLlmProvider === "embedded"
    ? { kind: "embedded" }
    : {
        kind: "open_ai_compat",
        base_url: currentRemoteConfig?.base_url || "",
        model: currentRemoteConfig?.model || "",
        api_key: currentRemoteConfig?.api_key || undefined,
        provider_name: currentRemoteConfig?.provider_name || undefined,
      };
  const isCloudUrl = checkIfCloudUrl(currentProvider.base_url || "");
  const providerPill = currentProvider.kind === "embedded" ? "local" : isCloudUrl ? "cloud" : "remote";

  const getCloudProviderIndex = (u: string) => {
    const idx = CLOUD_PROVIDERS.findIndex(
      (p) => u.includes(p.id) || (u.includes("google") && p.id === "gemini")
    );
    return idx === -1 ? 0 : idx;
  };
  const cloudIndex = getCloudProviderIndex(currentProvider.base_url || "");

  useEffect(() => {
    if (currentProvider !== prevLlmProvider) {
      setPrevLlmProvider(currentProvider);
      if (currentProvider.kind === "open_ai_compat") {
        const baseUrl = currentProvider.base_url || "http://127.0.0.1:11434";
        if (!isCloudUrl) setUrl(baseUrl);
        setApiKey(currentProvider.api_key || "");
      }
    }
  }, [currentProvider, prevLlmProvider, isCloudUrl]);

  const chatterboxRemote = draftSettings.tts.chatterbox_remote;

  useEffect(() => {
    if (chatterboxRemote) {
      setRemoteTtsEndpoint(chatterboxRemote.endpoint || "http://127.0.0.1:7860");
      setRemoteTtsPath(chatterboxRemote.remote_path || "~/.vox");
    }
  }, [chatterboxRemote]);

  const handleRemoteTtsEndpointChange = (val: string) => {
    setRemoteTtsEndpoint(val);
    updateDraft("tts", "chatterbox_remote", {
      ...draftSettings.tts.chatterbox_remote,
      endpoint: val || "http://127.0.0.1:7860",
    });
  };

  const handleRemoteTtsPathChange = (val: string) => {
    setRemoteTtsPath(val);
    updateDraft("tts", "chatterbox_remote", {
      ...draftSettings.tts.chatterbox_remote,
      remote_path: val || "~/.vox",
    });
  };

  useEffect(() => {
    if (currentProvider.kind !== "open_ai_compat" || !currentProvider.base_url) {
      setIsHealthy(null);
      setModelsError(null);
      return;
    }

    const timer = setTimeout(() => {
      const runChecks = async () => {
        setCheckingHealth(true);
        setModelsError(null);
        try {
          const healthy = await checkLlmProviderHealth(currentProvider);
          setIsHealthy(healthy);

          if (healthy && providerPill === "remote" && activeLlmProvider === "server") {
            const detectedName = currentProvider.base_url?.includes("11434")
              ? "Ollama"
              : "Remote Host";
            if (currentProvider.provider_name !== detectedName) {
              updateDraft("llm", "server", {
                ...draftSettings.llm.server,
                provider_name: detectedName,
              });
            }
          }
        } catch (err) {
          console.error(err);
          setIsHealthy(false);
          setModelsError("Connection failed");
        } finally {
          setCheckingHealth(false);
        }
      };
      runChecks();
    }, 500);

    return () => clearTimeout(timer);
  }, [
    currentProvider.base_url,
    currentProvider.api_key,
    currentProvider.kind,
    currentProvider.provider_name,
    providerPill,
    activeLlmProvider,
    updateDraft,
  ]);

  const handleUrlChange = (val: string) => {
    setUrl(val);
    if (activeLlmProvider === "server") {
      updateDraft("llm", "server", {
        ...draftSettings.llm.server,
        base_url: val || "http://127.0.0.1:11434",
      });
    } else if (activeLlmProvider === "cloud") {
      updateDraft("llm", "cloud", {
        ...draftSettings.llm.cloud,
        base_url: val || CLOUD_PROVIDERS[0].url,
      });
    }
  };

  const handleApiKeyChange = (key: string) => {
    setApiKey(key);
    if (activeLlmProvider === "server") {
      updateDraft("llm", "server", {
        ...draftSettings.llm.server,
        api_key: key || null,
      });
    } else if (activeLlmProvider === "cloud") {
      updateDraft("llm", "cloud", {
        ...draftSettings.llm.cloud,
        api_key: key || null,
      });
    }
  };

  const handleCloudCycle = (direction: "left" | "right") => {
    const currentIdx = getCloudProviderIndex(currentProvider.base_url || "");
    const nextIdx =
      direction === "left"
        ? (currentIdx - 1 + CLOUD_PROVIDERS.length) % CLOUD_PROVIDERS.length
        : (currentIdx + 1) % CLOUD_PROVIDERS.length;

    updateDraft("llm", "cloud", {
      ...draftSettings.llm.cloud,
      base_url: CLOUD_PROVIDERS[nextIdx].url,
      provider_name: CLOUD_PROVIDERS[nextIdx].name,
      api_key:
        draftSettings.llm.cloud?.provider_name === CLOUD_PROVIDERS[nextIdx].name
          ? draftSettings.llm.cloud?.api_key
          : null,
    });
  };

  const renderLlmStatusBadge = () => {
    if (providerPill === "local") return null;
    if (checkingHealth) {
      return (
        <span className="text-[12px] font-bold text-yellow-400 animate-pulse flex items-center gap-1">
          <RefreshCw size={14} className="animate-spin" /> Ping
        </span>
      );
    }
    if (isHealthy === true) {
      return (
        <span className="text-[12px] font-bold text-emerald-400 flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" /> Online
        </span>
      );
    }
    if (isHealthy === false) {
      return (
        <span className="text-[12px] font-bold text-rose-400 flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-rose-500" /> Offline
        </span>
      );
    }
    return null;
  };

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 pt-2.5 pb-0.5 justify-between",
        layoutMode === "small"
          ? "h-auto min-h-0 max-h-none py-2 space-y-3"
          : isModular
            ? activeCategory === "TTS" && activePill === "remote"
              ? "h-auto min-h-[115px]"
              : "h-[115px] min-h-[115px] max-h-[115px]"
            : "flex-1 min-h-[115px]"
      )}
    >
      {/* ─── SECTION 0: INTEGRATED PIPELINE (MODE = INTEGRATED) ─── */}
      {!isModular && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2 py-1">
          <div className="flex-1 flex items-center justify-center relative min-w-[80px] h-full">
            <div className="absolute w-16 h-16 rounded-full border border-[rgb(var(--accent))]/10 animate-ring-pulse-slow" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Sparkles className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Unified End-to-End Voice Engine
              </span>
              <span className="text-[11px] font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
                Sub-200ms
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-relaxed font-semibold">
              Streaming STT, LLM inference, and TTS audio synthesis are tightly coupled in zero-copy memory for minimal perceived latency.
            </p>
          </div>
        </div>
      )}
      {/* ─── SECTION 1: STT CATEGORY ─── */}
      {isModular && activeCategory === "STT" && activePill === "local" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Mic className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Embedded STT Engine
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              {activeAsrDescription}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "remote" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[80px] h-full">
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center relative z-10">
              <Clock className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-0.5 h-full">
            <span className="text-[12px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/80">
              Server STT
            </span>
            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
              Coming Soon
            </span>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal font-medium line-clamp-1 mt-0.5">
              Remote WebSocket speech-to-text server streaming will be available in a future update.
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "cloud" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[80px] h-full">
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center relative z-10">
              <Cloud className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-0.5 h-full">
            <span className="text-[12px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/80">
              Cloud STT
            </span>
            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]">
              Coming Soon
            </span>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal font-medium line-clamp-1 mt-0.5">
              Ultra-fast Groq & Deepgram cloud speech-to-text integration is in active development.
            </p>
          </div>
        </div>
      )}

      {/* ─── SECTION 2: LLM CATEGORY ─── */}
      {isModular && activeCategory === "LLM" && activePill === "local" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Brain className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>

          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Embedded LLM Engine
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              {activeLlmDescription}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "LLM" && activePill === "remote" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
            <span className="font-bold text-[12px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
              <Network size={16} className="text-[rgb(var(--accent))]" />
              Server Connection Settings
            </span>
            {renderLlmStatusBadge()}
          </div>

          <div
            className={cn(
              "grid gap-3 items-end flex-1 pb-1",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
            )}
          >
            <UnderlineInput
              label="Server URL"
              value={url}
              onChange={(e) => handleUrlChange(e.target.value)}
              placeholder="http://127.0.0.1:11434"
            />
            <ApiKeyField
              label="API Key (Optional)"
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder="Bearer token..."
            />
          </div>

          {modelsError && (
            <span className="text-[12px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
              <AlertCircle size={14} /> {modelsError}
            </span>
          )}
        </div>
      )}

      {isModular && activeCategory === "LLM" && activePill === "cloud" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
            <span className="font-bold text-[12px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
              <Cloud size={16} className="text-[rgb(var(--accent))]" />
              Cloud API Settings
            </span>
            {renderLlmStatusBadge()}
          </div>

          <div className={cn("grid gap-5 flex-1 min-h-0 items-end", layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2")}>
            <CarouselSelector
              label="Cloud Provider"
              value={CLOUD_PROVIDERS[cloudIndex].name}
              onPrev={() => handleCloudCycle("left")}
              onNext={() => handleCloudCycle("right")}
            />

            <ApiKeyField
              label="API Key (Required)"
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder={CLOUD_PROVIDERS[cloudIndex].keyPlaceholder}
              error={!apiKey?.trim()}
            />
          </div>

          {modelsError && (
            <span className="text-[12px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
              <AlertCircle size={14} /> {modelsError}
            </span>
          )}
        </div>
      )}

      {/* ─── SECTION 3: TTS CATEGORY ─── */}
      {isModular && activeCategory === "TTS" && activePill === "local" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Volume2 className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>

          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Embedded TTS Engine
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              {activeTtsDescription}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "TTS" && activePill === "remote" && (
        <div className="flex flex-col gap-2.5 h-full justify-center animate-fade-in py-1">
          <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
            <span className="font-bold text-[12px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
              <Network size={16} className="text-[rgb(var(--accent))]" />
              Chatterbox GPU Server Endpoint
            </span>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <UnderlineInput
              label="Server HTTP URL"
              value={remoteTtsEndpoint}
              onChange={(e) => handleRemoteTtsEndpointChange(e.target.value)}
              placeholder="http://127.0.0.1:7860"
            />
            <UnderlineInput
              label="Remote Path"
              value={remoteTtsPath}
              onChange={(e) => handleRemoteTtsPathChange(e.target.value)}
              placeholder="~/.vox"
            />
          </div>
        </div>
      )}

      {isModular && activeCategory === "TTS" && activePill === "cloud" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Sparkles className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>

          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[11px] font-bold text-white-400 uppercase">
                Zero Config
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              {edgeTtsDescription}
            </p>
          </div>
        </div>
      )}
    </div>
  );
});

LlmConfigDesk.displayName = "LlmConfigDesk";
