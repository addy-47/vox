import { useState, useEffect, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { checkIfCloudUrl, CLOUD_PROVIDERS } from "@/data/providers";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  Brain, Cloud, Network, Volume2, Sparkles, Mic,
  RefreshCw, ChevronLeft, ChevronRight, AlertCircle, Clock
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField } from "@/shared/ui";

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

  const [url, setUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [prevLlmProvider, setPrevLlmProvider] = useState<any>(null);

  const [remoteTtsEndpoint, setRemoteTtsEndpoint] = useState("");
  const [remoteTtsPath, setRemoteTtsPath] = useState("");
  const [prevTtsProvider, setPrevTtsProvider] = useState<any>(null);

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);

  if (!draftSettings || !settings) return null;
  const currentProvider = draftSettings.llm.provider || { kind: "embedded" };
  const isCloudUrl = checkIfCloudUrl(currentProvider.base_url || "");
  const providerPill = currentProvider.kind === "embedded" ? "local" : isCloudUrl ? "cloud" : "remote";

  const getCloudProviderIndex = (u: string) => {
    const idx = CLOUD_PROVIDERS.findIndex(
      (p) => u.includes(p.id) || (u.includes("google") && p.id === "gemini")
    );
    return idx === -1 ? 0 : idx;
  };
  const cloudIndex = getCloudProviderIndex(currentProvider.base_url || "");

  if (currentProvider !== prevLlmProvider) {
    setPrevLlmProvider(currentProvider);
    if (currentProvider.kind === "open_ai_compat") {
      const baseUrl = currentProvider.base_url || "http://127.0.0.1:11434";
      if (!isCloudUrl) setUrl(baseUrl);
      setApiKey(currentProvider.api_key || "");
    }
  }

  const currentTtsProvider = draftSettings.tts?.provider || { kind: "supertonic" };
  if (currentTtsProvider !== prevTtsProvider) {
    setPrevTtsProvider(currentTtsProvider);
    if (currentTtsProvider.kind === "chatterbox_remote") {
      setRemoteTtsEndpoint(currentTtsProvider.endpoint || "http://127.0.0.1:7860");
      setRemoteTtsPath(currentTtsProvider.remote_path || "~/.vox");
    }
  }

  const handleRemoteTtsEndpointChange = (val: string) => {
    setRemoteTtsEndpoint(val);
    updateDraft("tts", "provider", {
      ...currentTtsProvider,
      endpoint: val || "http://127.0.0.1:7860",
    });
  };

  const handleRemoteTtsPathChange = (val: string) => {
    setRemoteTtsPath(val);
    updateDraft("tts", "provider", {
      ...currentTtsProvider,
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

          if (healthy && providerPill === "remote") {
            const detectedName = currentProvider.base_url?.includes("11434")
              ? "Ollama"
              : "Remote Host";
            if (currentProvider.provider_name !== detectedName) {
              updateDraft("llm", "provider", {
                ...currentProvider,
                provider_name: detectedName,
              });
            }
          } else if (!healthy) {
            setModelsError("Server unreachable");
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
    updateDraft,
  ]);

  const handleUrlChange = (val: string) => {
    setUrl(val);
    updateDraft("llm", "provider", {
      ...currentProvider,
      base_url: val || "http://127.0.0.1:11434",
    });
  };

  const handleApiKeyChange = (key: string) => {
    setApiKey(key);
    updateDraft("llm", "provider", {
      ...currentProvider,
      api_key: key || undefined,
    });
  };

  const handleCloudCycle = (direction: "left" | "right") => {
    const currentIdx = getCloudProviderIndex(currentProvider.base_url || "");
    const nextIdx =
      direction === "left"
        ? (currentIdx - 1 + CLOUD_PROVIDERS.length) % CLOUD_PROVIDERS.length
        : (currentIdx + 1) % CLOUD_PROVIDERS.length;

    updateDraft("llm", "provider", {
      ...currentProvider,
      base_url: CLOUD_PROVIDERS[nextIdx].url,
      provider_name: CLOUD_PROVIDERS[nextIdx].name,
      api_key:
        currentProvider.provider_name === CLOUD_PROVIDERS[nextIdx].name
          ? currentProvider.api_key
          : "",
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
        <span className="text-[12px] font-bold text-emerald-400 flex items-center gap-1 bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 rounded-md">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" /> Online
        </span>
      );
    }
    if (isHealthy === false) {
      return (
        <span className="text-[12px] font-bold text-rose-400 flex items-center gap-1 bg-rose-500/10 border border-rose-500/20 px-1.5 py-0.5 rounded-md">
          <span className="w-1.5 h-1.5 rounded-full bg-rose-500" /> Offline
        </span>
      );
    }
    return null;
  };

  return (
    <div
      className={cn(
        "w-full flex flex-col rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)]",
        layoutMode === "small"
          ? "h-auto min-h-0 max-h-none py-4 space-y-4"
          : isModular
            ? activeCategory === "TTS" && activePill === "remote"
              ? "h-auto min-h-[120px]"
              : "h-[120px] min-h-[120px] max-h-[120px]"
            : "flex-1 min-h-[140px]"
      )}
    >
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
              <span className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase font-mono">
                {draftSettings.asr?.model || "Whisper Medium"}
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              On-device voice transcription powered by whisper.cpp with streaming INT8 quantization.
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "remote" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="w-8 h-8 rounded-full bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center">
              <Clock className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Remote Whisper Server
              </span>
              <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 uppercase font-mono">
                Coming Soon
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              Remote WebSocket streaming STT server integration is scheduled for an upcoming release.
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "cloud" && (
        <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
          <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
            <div className="w-8 h-8 rounded-full bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center">
              <Cloud className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>
          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Cloud Transcription
              </span>
              <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 uppercase font-mono">
                Coming Soon
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
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
            <div className="absolute w-14 h-14 rounded-full border border-[rgb(var(--accent))]/15 animate-pulse-slow" />
            <div className="absolute w-10 h-10 rounded-full border border-[rgb(var(--accent))]/25 animate-pulse" />
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
              <Brain className="text-[rgb(var(--accent))]" size={18} />
            </div>
          </div>

          <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
            <div className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)] animate-pulse" />
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Local Core Ready
              </span>
            </div>
            <div className="space-y-0.5 text-[12px] text-[rgb(var(--foreground-muted))]/70 font-medium">
              <div className="flex justify-between items-center border-b border-[rgba(var(--border),0.04)] pb-0.5">
                <span className="shrink-0">PIPELINE</span>
                <span className="font-mono text-[12px] text-[rgb(var(--accent))] text-right whitespace-nowrap">
                  low-latency
                </span>
              </div>
              <div className="flex justify-between items-center border-b border-[rgba(var(--border),0.04)] pb-0.5">
                <span className="shrink-0">VAD SENSE</span>
                <span className="font-mono text-[12px] text-[rgb(var(--accent))] text-right whitespace-nowrap">
                  earshot rust
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="shrink-0">LLM ENGINE</span>
                <span className="font-mono text-[12px] text-[rgb(var(--accent))] text-right whitespace-nowrap">
                  local gguf
                </span>
              </div>
            </div>
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
              "grid gap-2 items-end flex-1 pb-1",
              layoutMode === "small" ? "grid-cols-1 gap-3" : "grid-cols-[1.5fr_1.5fr]"
            )}
          >
            <div className="space-y-1">
              <label className="text-[12px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                Server URL
              </label>
              <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                <input
                  type="text"
                  value={url}
                  onChange={(e) => handleUrlChange(e.target.value)}
                  placeholder="http://127.0.0.1:11434"
                  className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                />
              </div>
            </div>
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

          <div className="grid grid-cols-2 gap-3 flex-1 min-h-0 items-center">
            <div className="space-y-1">
              <label className="text-[12px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                Cloud Provider
              </label>
              <div className="flex items-center justify-between bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] rounded-lg h-[26px] px-1.5">
                <button
                  onClick={() => handleCloudCycle("left")}
                  className="p-0.5 text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors active:scale-90"
                >
                  <ChevronLeft size={18} />
                </button>
                <span className="text-[12px] font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
                  {CLOUD_PROVIDERS[cloudIndex].name}
                </span>
                <button
                  onClick={() => handleCloudCycle("right")}
                  className="p-0.5 text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors active:scale-90"
                >
                  <ChevronRight size={18} />
                </button>
              </div>
            </div>

            <ApiKeyField
              label="API Key (Required)"
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder={CLOUD_PROVIDERS[cloudIndex].keyPlaceholder}
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
                Local TTS Config
              </span>
              <span className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase">
                {currentTtsProvider.kind === "chatterbox" ? "Chatterbox Local" : "Supertonic 3"}
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              Synthesis voice, speed and quality can be configured in the Models panel.
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

          <div className="grid grid-cols-2 gap-2.5">
            <div className="space-y-1">
              <label className="text-[10px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
                Server HTTP URL
              </label>
              <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                <input
                  type="text"
                  value={remoteTtsEndpoint}
                  onChange={(e) => handleRemoteTtsEndpointChange(e.target.value)}
                  placeholder="http://127.0.0.1:7860"
                  className="w-full bg-transparent border-none outline-none text-[12px] font-mono text-[rgb(var(--foreground))]"
                />
              </div>
            </div>
            <div className="space-y-1">
              <label className="text-[10px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
                Remote Path
              </label>
              <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                <input
                  type="text"
                  value={remoteTtsPath}
                  onChange={(e) => handleRemoteTtsPathChange(e.target.value)}
                  placeholder="~/.vox"
                  className="w-full bg-transparent border-none outline-none text-[12px] font-mono text-[rgb(var(--foreground))]"
                />
              </div>
            </div>
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
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                Microsoft Edge Neural TTS
              </span>
              <span className="text-[11px] font-bold text-emerald-400 uppercase font-mono">
                Zero Config
              </span>
            </div>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
              Free, high-fidelity neural voices streamed directly over Edge WebSockets.
            </p>
          </div>
        </div>
      )}
    </div>
  );
});

LlmConfigDesk.displayName = "LlmConfigDesk";
