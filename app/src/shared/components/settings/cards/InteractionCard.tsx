import { memo, useState, useEffect } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import {
  Sliders,
  AlertCircle,
  Brain,
  Cloud,
  Server,
  Network,
  Eye,
  EyeOff,
  Layers,
  Zap,
  Activity,
  Radio,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Volume2,
  Database,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { checkLlmProviderHealth } from "@/services/settingsService";
import { CLOUD_PROVIDERS } from "@/data/providers";

const GeminiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="none" {...props}>
    <path
      d="M12 3c0 4.5 3.5 8 8 8-4.5 0-8 3.5-8 8 0-4.5-3.5-8-8-8 4.5 0 8-3.5 8-8z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

const OpenAiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <path
      d="M21.3 11.1c0-.7-.2-1.4-.6-2-.4-.6-1-.9-1.7-1.1-.1-.7-.4-1.3-.9-1.8s-1.1-.9-1.8-1c-.5-.5-1.1-.9-1.8-1-.7-.2-1.4-.2-2.1 0-.6.2-1.2.5-1.7 1-.5-.5-1.1-.8-1.7-1-.7-.2-1.4-.2-2.1 0-.7.2-1.3.5-1.8 1-.5.5-.8 1.1-.9 1.8-.7.1-1.3.4-1.8.9C3 8.4 2.7 9 2.6 9.7c-.5.5-.9 1.1-1 1.8-.2.7-.2 1.4 0 2.1.2.6.5 1.2 1 1.7-.5.5-.8 1.1-1 1.7-.2.7-.2 1.4 0 2.1.2.7.5 1.3 1 1.8.5.5 1.1.8 1.8.9.1.7.4 1.3.9 1.8.5.5 1.1.9 1.8 1 .5.5 1.1.9 1.8 1 .7.2 1.4.2 2.1 0 .6-.2 1.2-.5 1.7-1 .5.5 1.1.8 1.7 1 .7.2 1.4.2 2.1 0 .7-.2 1.3-.5 1.8-1 .5-.5.8-1.1.9-1.8.7-.1 1.3-.4 1.8-.9.5-.5.8-1.1.9-1.8.5-.5.9-1.1 1-1.8.2-.7.2-1.4 0-2.1-.2-.6-.5-1.2-1-1.7.5-.5.8-1.1 1-1.7.2-.6.2-1.3 0-2zm-8.8 7.3l-2.9-1.7c-.2-.1-.3-.3-.3-.6V12.7l1.4.8c.2.1.3.3.3.6v2.1l1.5.9v-4.2l-1.4-.8c-.2-.1-.3-.3-.3-.6V9.3l2.9 1.7c.2.1.3.3.3.6v3.4l-1.4-.8c-.2-.1-.3-.3-.3-.6v-2.1l-1.5-.9v4.2l1.4.8c.2.1.3.3.3.6v1.9z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

const DeepgramLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    {...props}
  >
    <polygon
      points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5"
      opacity={active ? 0.95 : 0.5}
    />
    <path d="M12 22V12" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L22 8.5" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L2 8.5" opacity={active ? 0.95 : 0.5} />
  </svg>
);

const ElevenLabsLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <rect
      x="5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.95 : 0.45}
    />
    <rect
      x="13.5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.75 : 0.3}
    />
  </svg>
);

const checkIfCloudUrl = (url: string) => {
  if (!url) return false;
  return (
    url.includes("openai.com") ||
    url.includes("googleapis.com") ||
    url.includes("anthropic.com") ||
    url.includes("groq.com") ||
    url.includes("nvidia.com")
  );
};

const REALTIME_PROVIDERS = [
  {
    id: "gemini_live",
    name: "Gemini Live",
    subkey: "gemini",
    icon: GeminiLogo,
    desc: "Sub-300ms Duplex",
    url: "https://aistudio.google.com/apikey",
    tagline:
      "Google's multimodal live streaming with sub-300ms duplex voice interaction",
  },
  {
    id: "openai_realtime",
    name: "OpenAI Realtime",
    subkey: "openai",
    icon: OpenAiLogo,
    desc: "S2S WebSocket",
    url: "https://platform.openai.com/api-keys",
    tagline:
      "OpenAI's speech-to-speech API via persistent WebSocket connections",
  },
  {
    id: "deepgram_voice_agent",
    name: "Deepgram Agent",
    subkey: "deepgram",
    icon: DeepgramLogo,
    desc: "Voice Agent SDK",
    url: "https://console.deepgram.com/",
    tagline:
      "Deepgram's voice agent platform for building custom AI assistants",
  },
  {
    id: "elevenlabs_convai",
    name: "ElevenLabs ConvAI",
    subkey: "elevenlabs",
    icon: ElevenLabsLogo,
    desc: "Conversational AI",
    url: "https://elevenlabs.io/app/settings/api-keys",
    tagline:
      "ElevenLabs' conversational AI with ultra-realistic voice synthesis",
  },
] as const;

const interactionStyles = `
@keyframes wave-bar-1 { 0%, 100% { height: 4px; } 50% { height: 16px; } }
@keyframes wave-bar-2 { 0%, 100% { height: 16px; } 50% { height: 6px; } }
@keyframes wave-bar-3 { 0%, 100% { height: 8px; } 50% { height: 18px; } }
@keyframes wave-bar-4 { 0%, 100% { height: 12px; } 50% { height: 4px; } }

.animate-wave-bar-1 { animation: wave-bar-1 1.2s ease-in-out infinite; }
.animate-wave-bar-2 { animation: wave-bar-2 1.2s ease-in-out infinite 0.2s; }
.animate-wave-bar-3 { animation: wave-bar-3 1.2s ease-in-out infinite 0.4s; }
.animate-wave-bar-4 { animation: wave-bar-4 1.2s ease-in-out infinite 0.6s; }

@keyframes flow-dot { 0% { left: -10%; } 100% { left: 110%; } }
.animate-flow-dot { animation: flow-dot 2s cubic-bezier(0.4, 0, 0.2, 1) infinite; }

@keyframes ring-pulse-slow {
  0%, 100% { transform: scale(1); opacity: 0.15; }
  50% { transform: scale(1.25); opacity: 0.45; }
}
.animate-ring-pulse-slow { animation: ring-pulse-slow 4s ease-in-out infinite; }

@keyframes pulse-glow {
  0%, 100% {
    box-shadow: 0 0 5px rgba(var(--accent-rgb), 0.2), inset 0 0 5px rgba(var(--accent-rgb), 0.1);
  }
  50% {
    box-shadow: 0 0 15px rgba(var(--accent-rgb), 0.6), inset 0 0 10px rgba(var(--accent-rgb), 0.2);
  }
}
.active-glow {
  animation: pulse-glow 2.5s infinite ease-in-out;
}
.custom-select {
  appearance: none;
  background-image: url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.7)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6 9 12 15 18 9'%3e%3c/polyline%3e%3c/svg%3e");
  background-repeat: no-repeat;
  background-position: right 0.5rem center;
  background-size: 0.8em;
}
`;

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const InteractionCard = memo(
  ({ layoutMode = "full-max" }: InteractionCardProps) => {
    const { settings, draftSettings, updateDraft } = useSettings();

    const [showApiKey, setShowApiKey] = useState(false);
    const [url, setUrl] = useState("");
    const [apiKey, setApiKey] = useState("");
    const [prevLlmProvider, setPrevLlmProvider] = useState<any>(null);

    const [activeCategory, setActiveCategory] = useState<"STT" | "LLM" | "TTS">(
      "LLM",
    );
    const [remoteTtsEndpoint, setRemoteTtsEndpoint] = useState("");
    const [remoteTtsPath, setRemoteTtsPath] = useState("");
    const [prevTtsProvider, setPrevTtsProvider] = useState<any>(null);

    const [sttPillOverride, setSttPillOverride] = useState<
      "local" | "remote" | "cloud" | null
    >(null);
    const [ttsPillOverride, setTtsPillOverride] = useState<
      "local" | "remote" | "cloud" | null
    >(null);

    // Live query states
    const [modelsError, setModelsError] = useState<string | null>(null);
    const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
    const [checkingHealth, setCheckingHealth] = useState(false);

    if (!draftSettings || !settings) return null;
    const { interaction, llm } = draftSettings;

    const currentProvider = llm.provider || { kind: "embedded" };

    // Decide active provider category: local, cloud (if url has openai/google/anthropic/groq), remote (otherwise)
    const isCloudUrl = checkIfCloudUrl(currentProvider.base_url || "");

    const providerPill =
      currentProvider.kind === "embedded"
        ? "local"
        : isCloudUrl
          ? "cloud"
          : "remote";

    const isModular = interaction.pipeline_mode === "modular";

    // Selector state lifted to component-level scope
    const sttPill = sttPillOverride || "local";
    const llmPill = providerPill;
    const ttsKind = draftSettings.tts?.provider?.kind || "supertonic";
    const ttsPill =
      ttsPillOverride ||
      (ttsKind === "chatterbox_remote"
        ? "remote"
        : ttsKind === "supertonic" || ttsKind === "chatterbox"
          ? "local"
          : "cloud");
    const activePill =
      activeCategory === "STT"
        ? sttPill
        : activeCategory === "LLM"
          ? llmPill
          : ttsPill;

    // Compute active cloud provider index based on base_url
    const getCloudProviderIndex = (url: string) => {
      const idx = CLOUD_PROVIDERS.findIndex(
        (p) =>
          url.includes(p.id) || (url.includes("google") && p.id === "gemini"),
      );
      return idx === -1 ? 0 : idx;
    };
    const cloudIndex = getCloudProviderIndex(currentProvider.base_url || "");

    // Synchronize local input state with currentProvider settings
    if (currentProvider !== prevLlmProvider) {
      setPrevLlmProvider(currentProvider);
      if (currentProvider.kind === "open_ai_compat") {
        const baseUrl = currentProvider.base_url || "http://127.0.0.1:11434";
        if (isCloudUrl) {
          setApiKey(currentProvider.api_key || "");
        } else {
          setUrl(baseUrl);
          setApiKey(currentProvider.api_key || "");
        }
      }
    }

    // Synchronize local input state for TTS settings
    const currentTtsProvider = draftSettings.tts?.provider || {
      kind: "supertonic",
    };
    if (currentTtsProvider !== prevTtsProvider) {
      setPrevTtsProvider(currentTtsProvider);
      if (currentTtsProvider.kind === "chatterbox_remote") {
        setRemoteTtsEndpoint(
          currentTtsProvider.endpoint || "http://127.0.0.1:7860",
        );
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

    const cycleCategory = () => {
      setActiveCategory((prev) => {
        const next = prev === "STT" ? "LLM" : prev === "LLM" ? "TTS" : "STT";
        const event = new CustomEvent("sync_pipeline_tab", {
          detail: next.toLowerCase(),
        });
        window.dispatchEvent(event);
        return next;
      });
    };

    useEffect(() => {
      const handleSync = (e: Event) => {
        const cat = (e as CustomEvent).detail;
        setActiveCategory(cat);
      };
      window.addEventListener("sync_interaction_category", handleSync);
      return () =>
        window.removeEventListener("sync_interaction_category", handleSync);
    }, []);

    // Live query effect for remote/cloud health checks and provider name persistence
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

            if (healthy) {
              // Detect and persist remote provider name (e.g. Ollama vs OpenAI Compatible Host)
              if (providerPill === "remote") {
                const detectedName = currentProvider.base_url?.includes("11434")
                  ? "Ollama"
                  : "Remote Host";
                if (currentProvider.provider_name !== detectedName) {
                  updateDraft("llm", "provider", {
                    ...currentProvider,
                    provider_name: detectedName,
                  });
                }
              }
            } else {
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
    ]);

    const handleLlmPillChange = (value: string) => {
      if (value === "local") {
        updateDraft("llm", "provider", { kind: "embedded" });
      } else if (value === "remote") {
        // Restore last saved remote configuration if available, otherwise default to Ollama local IP
        const savedRemote =
          settings.llm.provider.kind === "open_ai_compat" &&
          !checkIfCloudUrl(settings.llm.provider.base_url || "")
            ? settings.llm.provider
            : null;
        updateDraft("llm", "provider", {
          kind: "open_ai_compat",
          base_url: savedRemote?.base_url || "http://127.0.0.1:11434",
          api_key: savedRemote?.api_key || "",
          provider_name: savedRemote?.provider_name || "Ollama",
          model: savedRemote?.model || "",
        });
      } else if (value === "cloud") {
        // Restore last saved cloud configuration if available, otherwise default to OpenAI
        const savedCloud =
          settings.llm.provider.kind === "open_ai_compat" &&
          checkIfCloudUrl(settings.llm.provider.base_url || "")
            ? settings.llm.provider
            : null;
        updateDraft("llm", "provider", {
          kind: "open_ai_compat",
          base_url: savedCloud?.base_url || CLOUD_PROVIDERS[0].url,
          api_key: savedCloud?.api_key || "",
          provider_name: savedCloud?.provider_name || CLOUD_PROVIDERS[0].name,
          model: savedCloud?.model || "",
        });
      }
    };

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

    const handleRealtimeApiKeyChange = (value: string) => {
      const provId = draftSettings.realtime?.provider || "gemini_live";
      const item =
        REALTIME_PROVIDERS.find((p) => p.id === provId) ||
        REALTIME_PROVIDERS[0];
      const subkey = item.subkey;

      const currentSubConfig = draftSettings.realtime[subkey];
      updateDraft("realtime", subkey, {
        ...currentSubConfig,
        api_key: value,
      });
    };

    const renderLlmStatusBadge = () => {
      if (providerPill === "local") return null;
      if (checkingHealth) {
        return (
          <span className="text-[11px] font-bold text-yellow-400 animate-pulse flex items-center gap-1">
            <RefreshCw size={14} className="animate-spin" /> Ping
          </span>
        );
      }
      if (isHealthy === true) {
        return (
          <span className="text-[11px] font-bold text-emerald-400 flex items-center gap-1 bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 rounded-md">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" /> Online
          </span>
        );
      }
      if (isHealthy === false) {
        return (
          <span className="text-[11px] font-bold text-rose-400 flex items-center gap-1 bg-rose-500/10 border border-rose-500/20 px-1.5 py-0.5 rounded-md">
            <span className="w-1.5 h-1.5 rounded-full bg-rose-500" /> Offline
          </span>
        );
      }
      return null;
    };

    const isPassive = interaction.main_app_mode === "Passive";

    return (
      <div
        className={cn(
          "w-full flex flex-col text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
          layoutMode === "small"
            ? "bg-transparent p-0 h-auto"
            : cn(
                "glass-card p-5 lg:h-[340px]",
                layoutMode === "full-min"
                  ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]"
                  : "lg:w-[520px]",
              ),
        )}
      >
        <style>{interactionStyles}</style>

        {/* Header Section */}
        {layoutMode !== "small" && (
          <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
            <div className="flex items-center gap-2">
              <Sliders className="text-[rgb(var(--accent))]" size={18} />
              <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
                Interaction Console
              </span>
            </div>
            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40">
              v0.8.6
            </span>
          </div>
        )}

        <div className="flex flex-col gap-3 flex-1">
          {/* Core Controls Dashboard Grid (2 Buttons) */}
          <div
            className={cn(
              "grid gap-2 shrink-0",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-2",
            )}
          >
            {/* Card 1: Trigger Mode (Continuous vs PTT) */}
            <div className="group flex items-center w-full h-[85px] relative">
              <div
                onClick={() =>
                  updateDraft(
                    "interaction",
                    "main_app_mode",
                    isPassive ? "PTT" : "Passive",
                  )
                }
                className="flex-1 p-3 rounded-xl group-hover:rounded-r-none border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between h-full cursor-pointer"
              >
                <div className="flex items-center justify-between">
                  <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">
                    Trigger
                  </span>
                  <div className="flex items-center gap-3">
                    {isPassive ? (
                      <Activity
                        size={16}
                        className="text-[rgb(var(--accent))]"
                      />
                    ) : (
                      <Radio size={16} className="text-[rgb(var(--accent))]" />
                    )}
                  </div>
                </div>

                <div className="flex items-end justify-between mt-2">
                  <div className="flex flex-col">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
                      {isPassive ? "Continuous" : "Push-To-Talk"}
                    </span>
                    <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
                      {isPassive ? "Passive Sense" : "Manual Trigger"}
                    </span>
                  </div>

                  {/* Visualizer widget */}
                  <div className="h-4 flex items-end">
                    {isPassive ? (
                      <div className="flex items-end gap-[1.5px] h-3">
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-1" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-2" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-3" />
                        <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-4" />
                      </div>
                    ) : (
                      <div className="w-3 h-3 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                        <span className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-60" />
                        <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* Slide-out toggle side panel */}
              <div
                onClick={() =>
                  updateDraft(
                    "interaction",
                    "main_app_mode",
                    isPassive ? "PTT" : "Passive",
                  )
                }
                className="h-full w-0 group-hover:w-[38px] opacity-0 group-hover:opacity-100 flex items-center justify-center bg-[rgba(var(--accent),0.05)] border border-transparent border-l-transparent group-hover:border-[rgba(var(--accent),0.15)] group-hover:border-l-transparent rounded-r-xl transition-all duration-300 overflow-hidden cursor-pointer select-none shrink-0"
              >
                <span className="text-[8px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
                  {layoutMode === "small" ? "TAP" : "TOGGLE"}
                </span>
              </div>
            </div>

            {/* Card 2: Pipeline Mode (Modular vs Realtime) */}
            <div className="group flex items-center w-full h-[85px] relative">
              <div
                onClick={() =>
                  updateDraft(
                    "interaction",
                    "pipeline_mode",
                    isModular ? "realtime" : "modular",
                  )
                }
                className="flex-1 p-3 rounded-xl group-hover:rounded-r-none border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between h-full cursor-pointer"
              >
                <div className="flex items-center justify-between">
                  <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">
                    Pipeline
                  </span>
                  <div className="flex items-center gap-3">
                    {isModular ? (
                      <Layers size={16} className="text-[rgb(var(--accent))]" />
                    ) : (
                      <Zap size={16} className="text-[rgb(var(--accent))]" />
                    )}
                  </div>
                </div>

                <div className="flex items-end justify-between mt-2">
                  <div className="flex flex-col">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground))] transition-colors group-hover:text-[rgb(var(--accent))] leading-none">
                      {isModular ? "Modular" : "Realtime"}
                    </span>
                    <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 font-semibold uppercase mt-1 leading-none">
                      {isModular ? "Hybrid Grid" : "Stream Duplex"}
                    </span>
                  </div>

                  {/* Visualizer widget */}
                  <div className="flex items-center">
                    {isModular ? (
                      <div className="flex flex-col gap-[1.5px] items-center">
                        <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                        <span className="w-2.5 h-[2px] bg-[rgb(var(--accent))]/60 rounded animate-pulse" />
                        <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                      </div>
                    ) : (
                      <div className="w-7 h-2 bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] rounded-full relative overflow-hidden flex items-center">
                        <span className="absolute w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-flow-dot shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* Slide-out toggle side panel */}
              <div
                onClick={() =>
                  updateDraft(
                    "interaction",
                    "pipeline_mode",
                    isModular ? "realtime" : "modular",
                  )
                }
                className="h-full w-0 group-hover:w-[38px] opacity-0 group-hover:opacity-100 flex items-center justify-center bg-[rgba(var(--accent),0.05)] border border-transparent border-l-transparent group-hover:border-[rgba(var(--accent),0.15)] group-hover:border-l-transparent rounded-r-xl transition-all duration-300 overflow-hidden cursor-pointer select-none shrink-0"
              >
                <span className="text-[8px] font-bold uppercase tracking-[0.15em] text-[rgb(var(--accent))] rotate-90 whitespace-nowrap">
                  {layoutMode === "small" ? "TAP" : "TOGGLE"}
                </span>
              </div>
            </div>
          </div>

          {/* Category & Provider Selector */}
          {isModular &&
            (() => {
              const handlePillChange = (
                value: "local" | "remote" | "cloud",
              ) => {
                if (activeCategory === "STT") {
                  if (value === "local") {
                    setSttPillOverride(null);
                  } else {
                    setSttPillOverride(value);
                  }
                } else if (activeCategory === "LLM") {
                  handleLlmPillChange(value);
                } else if (activeCategory === "TTS") {
                  if (value === "local") {
                    setTtsPillOverride(null);
                    updateDraft("tts", "provider", { kind: "supertonic" });
                  } else if (value === "remote") {
                    setTtsPillOverride(null);
                    updateDraft("tts", "provider", {
                      kind: "chatterbox_remote",
                      endpoint: remoteTtsEndpoint,
                      language: "en",
                      quality_steps: 8,
                      speed: 1.0,
                      remote_path: remoteTtsPath,
                    });
                  } else {
                    setTtsPillOverride(null);
                    updateDraft("tts", "provider", {
                      kind: "edge_tts",
                      voice: (draftSettings.tts.provider as any)?.voice || "en-US-AriaNeural",
                    });
                  }
                }
              };

              return (
                <div className="shrink-0 flex flex-col gap-2 w-full mt-3">
                  <div className="flex items-center gap-3 w-full pb-2 pt-1 shrink-0 mt-2 px-3">
                    {/* Single Cycling Category Button */}
                    <button
                      type="button"
                      onClick={cycleCategory}
                      className="flex items-center gap-1 pb-2 -mb-[13px] bg-transparent transition-all duration-200 text-[13px] font-black tracking-wider uppercase text-[rgb(var(--accent))] border-b-2 border-transparent outline-none active:scale-95 select-none"
                    >
                      <span>{activeCategory}</span>
                    </button>

                    {/* Accent coloured arrow separator */}
                    <span className="text-[13px] text-[rgb(var(--accent))]/70 font-black select-none tracking-tighter pb-2 -mb-[10px]">
                      ───&gt;
                    </span>

                    {/* Local | Remote | Cloud Pills (left-aligned) */}
                    <div className="flex items-center gap-3">
                      {[
                        { id: "local", label: "Embedded", icon: Brain },
                        { id: "remote", label: "Server", icon: Server },
                        { id: "cloud", label: "Cloud", icon: Cloud },
                      ].map((mode, idx, arr) => {
                        const isActive = activePill === mode.id;
                        const IconComponent = mode.icon;
                        return (
                          <div
                            key={mode.id}
                            className="flex items-center gap-3"
                          >
                            <button
                              type="button"
                              onClick={() => handlePillChange(mode.id as any)}
                              className={cn(
                                "flex items-center justify-center gap-1.5 pb-2 -mb-[10px] border-b-2 transition-all duration-200 bg-transparent text-[10px] font-black uppercase tracking-[0.15em] outline-none",
                                isActive
                                  ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                                  : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80",
                              )}
                            >
                              <span>{mode.label}</span>
                              {layoutMode !== "small" && (
                                <IconComponent size={11} className="shrink-0" />
                              )}
                            </button>
                            {idx < arr.length - 1 && (
                              <span className="text-[10px] text-[rgb(var(--foreground-muted))]/20 font-light select-none pb-2 -mb-[10px]">
                                |
                              </span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })()}

          {/* Persistent Height Config Desk (Reduced Height) */}
          <div
            className={cn(
              "w-full flex flex-col rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)]",
              layoutMode === "small"
                ? "h-auto min-h-0 max-h-none py-4 space-y-4"
                : isModular
                  ? activeCategory === "TTS" && activePill === "remote"
                    ? "h-auto min-h-[120px]"
                    : "h-[120px] min-h-[120px] max-h-[120px]"
                  : "flex-1 min-h-[140px]",
            )}
          >
            {/* STATE 1: Modular + LLM Local Core */}
            {isModular &&
              activeCategory === "LLM" &&
              activePill === "local" &&
              (layoutMode === "small" ? (
                <div className="flex flex-col gap-3 py-1 animate-fade-in w-full">
                  {/* Status Indicator */}
                  <div className="flex items-center gap-2">
                    <div className="w-7 h-7 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center shrink-0">
                      <Brain className="text-[rgb(var(--accent))]" size={14} />
                    </div>
                    <div className="flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)] animate-pulse" />
                      <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                        Local Core Active
                      </span>
                    </div>
                  </div>

                  {/* Simplified Status Badges Grid */}
                  <div className="grid grid-cols-3 gap-2">
                    <div className="bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.05)] p-2.5 rounded-xl flex flex-col justify-between min-w-0">
                      <span className="text-[8px] text-[rgb(var(--foreground-muted))]/55 font-bold uppercase tracking-wider">
                        Pipeline
                      </span>
                      <span className="text-[10px] font-mono text-[rgb(var(--accent))] mt-0.5 truncate font-semibold">
                        Hybrid
                      </span>
                    </div>
                    <div className="bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.05)] p-2.5 rounded-xl flex flex-col justify-between min-w-0">
                      <span className="text-[8px] text-[rgb(var(--foreground-muted))]/55 font-bold uppercase tracking-wider">
                        VAD Sense
                      </span>
                      <span className="text-[10px] font-mono text-[rgb(var(--accent))] mt-0.5 truncate font-semibold">
                        Earshot
                      </span>
                    </div>
                    <div className="bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.05)] p-2.5 rounded-xl flex flex-col justify-between min-w-0">
                      <span className="text-[8px] text-[rgb(var(--foreground-muted))]/55 font-bold uppercase tracking-wider">
                        LLM Engine
                      </span>
                      <span className="text-[10px] font-mono text-[rgb(var(--accent))] mt-0.5 truncate font-semibold">
                        GGUF
                      </span>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
                  {/* Left Side: Ambient Breathing Core */}
                  <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
                    <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
                    <div className="absolute w-14 h-14 rounded-full border border-[rgb(var(--accent))]/15 animate-pulse-slow" />
                    <div className="absolute w-10 h-10 rounded-full border border-[rgb(var(--accent))]/25 animate-pulse" />
                    <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
                      <Brain className="text-[rgb(var(--accent))]" size={18} />
                    </div>
                  </div>

                  {/* Right Side: Active Engine Status */}
                  <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
                    <div className="flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)] animate-pulse" />
                      <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                        Local Core Ready
                      </span>
                    </div>
                    <div className="space-y-0.5 text-[11px] text-[rgb(var(--foreground-muted))]/70 font-medium">
                      <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                        <span>PIPELINE</span>
                        <span className="font-mono text-[11px] text-[rgb(var(--accent))]">
                          LOW-LATENCY HYBRID
                        </span>
                      </div>
                      <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                        <span>VAD SENSE</span>
                        <span className="font-mono text-[11px] text-[rgb(var(--accent))]">
                          EARSHOT RUST
                        </span>
                      </div>
                      <div className="flex justify-between">
                        <span>LLM ENGINE</span>
                        <span className="font-mono text-[11px] text-[rgb(var(--accent))]">
                          LOCAL GGUF
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}

            {/* STATE 2: Modular + LLM Remote Ollama */}
            {isModular &&
              activeCategory === "LLM" &&
              activePill === "remote" && (
                <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
                  <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
                    <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      <Network
                        size={16}
                        className="text-[rgb(var(--accent))]"
                      />
                      Server Connection Settings
                    </span>
                    {renderLlmStatusBadge()}
                  </div>

                  {/* Single Line Input Layout — clean underline inputs */}
                  <div
                    className={cn(
                      "grid gap-2 items-end flex-1 pb-1",
                      layoutMode === "small"
                        ? "grid-cols-1 gap-3"
                        : "grid-cols-[1.5fr_1.5fr]",
                    )}
                  >
                    <div className="space-y-1">
                      <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                        Server URL
                      </label>
                      <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                        <input
                          type="text"
                          value={url}
                          onChange={(e) => handleUrlChange(e.target.value)}
                          placeholder="http://127.0.0.1:11434"
                          className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                        />
                      </div>
                    </div>
                    <div className="space-y-1">
                      <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                        API Key (Optional)
                      </label>
                      <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5 flex items-center gap-1.5">
                        <input
                          type={showApiKey ? "text" : "password"}
                          value={apiKey}
                          onChange={(e) => handleApiKeyChange(e.target.value)}
                          placeholder="Bearer token..."
                          className="flex-1 bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                        />
                        <button
                          type="button"
                          onClick={() => setShowApiKey(!showApiKey)}
                          className="text-[rgb(var(--foreground-muted))]/45 hover:text-[rgb(var(--accent))] transition-colors shrink-0 leading-none"
                        >
                          {showApiKey ? (
                            <EyeOff size={13} />
                          ) : (
                            <Eye size={13} />
                          )}
                        </button>
                      </div>
                    </div>
                  </div>

                  {modelsError && (
                    <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
                      <AlertCircle size={14} /> {modelsError}
                    </span>
                  )}
                </div>
              )}

            {/* STATE 3: Modular + LLM Cloud API */}
            {isModular &&
              activeCategory === "LLM" &&
              activePill === "cloud" && (
                <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
                  <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
                    <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      <Cloud size={16} className="text-[rgb(var(--accent))]" />
                      Cloud API Settings
                    </span>
                    {renderLlmStatusBadge()}
                  </div>

                  {/* Single Line Switcher + Key Layout */}
                  <div
                    className={cn(
                      "grid gap-3 items-end flex-1 pb-1",
                      layoutMode === "small"
                        ? "grid-cols-1 gap-3"
                        : "grid-cols-[1.5fr_2fr]",
                    )}
                  >
                    {/* Carousel Provider Selector */}
                    <div className="space-y-1">
                      <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                        Cloud Provider
                      </label>
                      <div className="flex items-center justify-between bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] rounded-lg h-[26px] px-1.5">
                        <button
                          onClick={() => handleCloudCycle("left")}
                          className="p-0.5 text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors active:scale-90"
                        >
                          <ChevronLeft size={18} />
                        </button>
                        <span className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
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

                    {/* API Key Input */}
                    <div className="space-y-1">
                      <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">
                        API Key (Required)
                      </label>
                      <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5 flex items-center gap-1.5">
                        <input
                          type={showApiKey ? "text" : "password"}
                          value={apiKey}
                          onChange={(e) => handleApiKeyChange(e.target.value)}
                          placeholder={
                            CLOUD_PROVIDERS[cloudIndex].keyPlaceholder
                          }
                          className="flex-1 bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                        />
                        <button
                          type="button"
                          onClick={() => setShowApiKey(!showApiKey)}
                          className="text-[rgb(var(--foreground-muted))]/45 hover:text-[rgb(var(--accent))] transition-colors shrink-0 leading-none"
                        >
                          {showApiKey ? (
                            <EyeOff size={13} />
                          ) : (
                            <Eye size={13} />
                          )}
                        </button>
                      </div>
                    </div>
                  </div>

                  {modelsError && (
                    <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
                      <AlertCircle size={14} /> {modelsError}
                    </span>
                  )}
                </div>
              )}

            {/* STATE: Modular + TTS Local */}
            {isModular &&
              activeCategory === "TTS" &&
              activePill === "local" && (
                <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
                  <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
                    <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
                    <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
                      <Volume2
                        className="text-[rgb(var(--accent))]"
                        size={18}
                      />
                    </div>
                  </div>

                  <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
                    <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
                      <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                        Local TTS Config
                      </span>
                      <span className="text-[10px] font-bold text-[rgb(var(--accent))] uppercase">
                        {currentTtsProvider.kind === "chatterbox"
                          ? "Chatterbox Local"
                          : "Supertonic 3"}
                      </span>
                    </div>
                    <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed font-semibold">
                      Synthesis voice, speed and quality can be configured in
                      the Models panel. Select Chatterbox Local in the Models
                      tab to switch.
                    </p>
                  </div>
                </div>
              )}

            {/* STATE: Modular + TTS Remote (Endpoint URL & Sandbox Path Only) */}
            {isModular &&
              activeCategory === "TTS" &&
              activePill === "remote" && (
                <div className="flex flex-col gap-2.5 h-full justify-center animate-fade-in py-1">
                  <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
                    <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      <Network
                        size={16}
                        className="text-[rgb(var(--accent))]"
                      />
                      Chatterbox GPU Server Endpoint
                    </span>
                  </div>

                  {/* Grid 1: Server Config */}
                  <div className="grid grid-cols-2 gap-2.5">
                    <div className="space-y-1">
                      <label className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
                        Server HTTP URL
                      </label>
                      <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                        <input
                          type="text"
                          value={remoteTtsEndpoint}
                          onChange={(e) =>
                            handleRemoteTtsEndpointChange(e.target.value)
                          }
                          placeholder="http://127.0.0.1:7860"
                          className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                        />
                      </div>
                    </div>
                    <div className="space-y-1">
                      <label className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
                        Server Sandbox Path
                      </label>
                      <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                        <input
                          type="text"
                          value={remoteTtsPath}
                          onChange={(e) =>
                            handleRemoteTtsPathChange(e.target.value)
                          }
                          placeholder="~/.vox"
                          className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              )}

            {/* STATE: Modular + TTS Cloud */}
            {isModular &&
              activeCategory === "TTS" &&
              activePill === "cloud" && (
                <div className="flex flex-col justify-center items-center h-full text-center p-4 animate-fade-in">
                  <span className="text-[11px] font-black uppercase tracking-widest text-[rgb(var(--accent))]/75 mb-1">
                    TTS Cloud API
                  </span>
                  <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold max-w-xs">
                    Remote cloud voice API integration (e.g. ElevenLabs ConvAI
                    cloud) is under development and will be available soon.
                  </p>
                </div>
              )}

            {/* STATE: Modular + STT Local */}
            {isModular &&
              activeCategory === "STT" &&
              activePill === "local" && (
                <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
                  <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
                    <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
                    <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center relative z-10">
                      <Database
                        className="text-[rgb(var(--accent))]"
                        size={18}
                      />
                    </div>
                  </div>

                  <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
                    <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1">
                      <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">
                        Local STT Ready
                      </span>
                      <span className="text-[10px] font-bold text-[rgb(var(--accent))] uppercase">
                        Nemotron-3.5
                      </span>
                    </div>
                    <div className="space-y-0.5 text-[10px] text-[rgb(var(--foreground-muted))]/70 font-semibold uppercase leading-normal">
                      <div className="flex justify-between">
                        <span>Engine</span>
                        <span className="font-mono text-[rgb(var(--accent))]">
                          Sherpa ONNX
                        </span>
                      </div>
                      <div className="flex justify-between">
                        <span>Model</span>
                        <span className="font-mono text-[rgb(var(--accent))]">
                          Nemotron-3.5 Whisper
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              )}

            {/* STATE: Modular + STT Remote/Cloud */}
            {isModular &&
              activeCategory === "STT" &&
              activePill !== "local" && (
                <div className="flex flex-col justify-center items-center h-full text-center p-4 animate-fade-in">
                  <span className="text-[11px] font-black uppercase tracking-widest text-[rgb(var(--accent))]/75 mb-1">
                    STT {activePill} API
                  </span>
                  <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold max-w-xs">
                    Speech-to-Text{" "}
                    {activePill === "remote" ? "remote" : "cloud"} processing is
                    currently under development. Vox will support remote Whisper
                    / Deepgram streaming in an upcoming release.
                  </p>
                </div>
              )}

            {/* STATE 4: Realtime Gateway — 1x4 provider grid + API key below */}
            {!isModular &&
              (() => {
                const provId =
                  draftSettings.realtime?.provider || "gemini_live";
                const currentRealtimeProvider =
                  REALTIME_PROVIDERS.find((p) => p.id === provId) ||
                  REALTIME_PROVIDERS[0];
                const IconComponent = currentRealtimeProvider.icon;
                return (
                  <div className="flex flex-col justify-between h-full animate-fade-in gap-2.5">
                    {/* Realtime Providers Flat Tab Strip */}
                    <div className="flex items-center justify-start gap-3.5 w-full border-b border-[rgba(var(--border),0.12)] pb-2 pt-1 shrink-0 mt-1">
                      {REALTIME_PROVIDERS.map((prov, idx, arr) => {
                        const isSelected = provId === prov.id;
                        const ButtonIcon = prov.icon;
                        const displayName = prov.name.split(" ")[0];
                        return (
                          <div
                            key={prov.id}
                            className="flex items-center gap-3.5"
                          >
                            <button
                              type="button"
                              onClick={() =>
                                updateDraft("realtime", "provider", prov.id)
                              }
                              className={cn(
                                "flex items-center justify-center gap-1.5 pb-2 -mb-[10px] border-b-2 transition-all duration-200 bg-transparent text-[10px] font-black uppercase tracking-[0.15em] outline-none",
                                isSelected
                                  ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                                  : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80",
                              )}
                            >
                              <span>{displayName}</span>
                              {layoutMode !== "small" && (
                                <ButtonIcon
                                  className="w-4 h-4 shrink-0"
                                  active={isSelected}
                                />
                              )}
                            </button>
                            {idx < arr.length - 1 && (
                              <span className="text-[10px] text-[rgb(var(--accent))]/30 font-light select-none pb-2 -mb-[10px]">
                                |
                              </span>
                            )}
                          </div>
                        );
                      })}
                    </div>

                    {/* Borderless Grid layout: Left 50% metadata, Right 50% API Key */}
                    <div
                      className={cn(
                        "grid items-end mt-2 gap-4 shrink-0",
                        layoutMode === "small"
                          ? "grid-cols-1 gap-3"
                          : "grid-cols-2",
                      )}
                    >
                      {/* Left Column: Metadata */}
                      <div className="flex items-start gap-3 min-w-0 pb-1">
                        <div className="p-2 rounded-xl bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.15)] flex items-center justify-center shrink-0">
                          <IconComponent
                            className="w-5 h-5 shrink-0"
                            active={true}
                          />
                        </div>
                        <div className="flex flex-col gap-1 min-w-0">
                          <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground))]/90 leading-none">
                            {currentRealtimeProvider.name} Gateway
                          </span>
                          <p className="text-[10px] text-[rgb(var(--foreground-muted))]/60 leading-normal font-semibold">
                            {currentRealtimeProvider.tagline}
                          </p>
                        </div>
                      </div>

                      {/* Right Column: API Key Input */}
                      <div className="flex flex-col justify-end">
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/55 tracking-wider leading-none">
                            API Key
                          </span>
                          <a
                            href={currentRealtimeProvider.url}
                            target="_blank"
                            rel="noreferrer"
                            className="text-[9px] font-bold text-[rgb(var(--accent))] hover:underline transition-colors shrink-0 leading-none"
                          >
                            Get Key
                          </a>
                        </div>
                        <div className="flex items-center gap-1.5 border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                          <input
                            type={showApiKey ? "text" : "password"}
                            value={
                              (draftSettings.realtime as any)[
                                currentRealtimeProvider.subkey
                              ]?.api_key || ""
                            }
                            onChange={(e) =>
                              handleRealtimeApiKeyChange(e.target.value)
                            }
                            placeholder={`${currentRealtimeProvider.name} Key...`}
                            className="flex-1 bg-transparent border-none outline-none text-[11px] text-[rgb(var(--foreground))] font-mono placeholder:text-[rgb(var(--foreground-muted))]/25 py-0.5"
                          />
                          <button
                            type="button"
                            onClick={() => setShowApiKey(!showApiKey)}
                            className="text-[rgb(var(--foreground-muted))]/45 hover:text-[rgb(var(--accent))] transition-colors shrink-0 leading-none"
                          >
                            {showApiKey ? (
                              <EyeOff size={11} />
                            ) : (
                              <Eye size={11} />
                            )}
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })()}
          </div>
        </div>
      </div>
    );
  },
);

InteractionCard.displayName = "InteractionCard";
