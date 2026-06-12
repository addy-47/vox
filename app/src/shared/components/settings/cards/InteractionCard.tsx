import { memo, useState, useEffect } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { 
  Sliders, Globe, RefreshCw, AlertCircle, Brain, 
  Cloud, Network, Eye, EyeOff, Layers, Zap, Activity, Radio,
  ChevronLeft, ChevronRight
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { invoke } from "@tauri-apps/api/core";


const CLOUD_PROVIDERS = [
  { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", keyPlaceholder: "sk-proj-..." },
  { id: "gemini", name: "Gemini", url: "https://generativelanguage.googleapis.com/v1beta", keyPlaceholder: "AIzaSy..." },
  { id: "anthropic", name: "Anthropic", url: "https://api.anthropic.com/v1", keyPlaceholder: "sk-ant-..." },
  { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", keyPlaceholder: "gsk_..." }
];

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

export const InteractionCard = memo(({ layoutMode = "full-max" }: InteractionCardProps) => {
  const { settings, draftSettings, updateDraft } = useSettings();
  
  const [showApiKey, setShowApiKey] = useState(false);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("11434");
  const [apiKey, setApiKey] = useState("");
  const [prevLlmProvider, setPrevLlmProvider] = useState<any>(null);

  // Live query states
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);

  if (!draftSettings || !settings) return null;
  const { interaction, llm } = draftSettings;

  const currentProvider = llm.provider || { kind: "embedded" };
  
  // Decide active provider category: local, cloud (if url has openai/google/anthropic/groq), remote (otherwise)
  const isCloudUrl = currentProvider.base_url?.includes("openai.com") || 
                     currentProvider.base_url?.includes("googleapis.com") || 
                     currentProvider.base_url?.includes("anthropic.com") || 
                     currentProvider.base_url?.includes("groq.com");

  const providerPill = currentProvider.kind === "embedded" 
    ? "local" 
    : (isCloudUrl ? "cloud" : "remote");

  const isModular = interaction.pipeline_mode === "modular";

  // Compute active cloud provider index based on base_url
  const getCloudProviderIndex = (url: string) => {
    const idx = CLOUD_PROVIDERS.findIndex(p => url.includes(p.id) || (url.includes("google") && p.id === "gemini"));
    return idx === -1 ? 0 : idx;
  };
  const cloudIndex = getCloudProviderIndex(currentProvider.base_url || "");

  // Synchronize local input state with currentProvider settings
  if (currentProvider !== prevLlmProvider) {
    setPrevLlmProvider(currentProvider);
    if (currentProvider.kind === "open_ai_compat") {
      const url = currentProvider.base_url || "";
      if (isCloudUrl) {
        setApiKey(currentProvider.api_key || "");
      } else {
        try {
          const urlObj = new URL(url);
          setHost(urlObj.hostname);
          setPort(urlObj.port || "80");
        } catch (e) {
          const cleanUrl = url.replace(/^https?:\/\//, "");
          const parts = cleanUrl.split(":");
          setHost(parts[0] || "");
          setPort(parts[1] || "11434");
        }
        setApiKey(currentProvider.api_key || "");
      }
    }
  }

  // Live query effect for remote/cloud health checks and provider name persistence
  useEffect(() => {
    if (currentProvider.kind !== "open_ai_compat") return;
    if (!currentProvider.base_url) {
      setIsHealthy(null);
      return;
    }

    const timer = setTimeout(() => {
      const runChecks = async () => {
        setCheckingHealth(true);
        setModelsError(null);
        try {
          const healthy = await invoke<boolean>("check_llm_provider_health", {
            provider: currentProvider
          });
          setIsHealthy(healthy);

          if (healthy) {
            // Detect and persist remote provider name (e.g. Ollama vs OpenAI Compatible Host)
            if (providerPill === "remote") {
              const detectedName = currentProvider.base_url?.includes("11434") ? "Ollama" : "Remote Host";
              if (currentProvider.provider_name !== detectedName) {
                updateDraft("llm", "provider", {
                  ...currentProvider,
                  provider_name: detectedName
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
  }, [currentProvider.base_url, currentProvider.api_key, currentProvider.kind]);

  const handleLlmPillChange = (value: string) => {
    if (value === "local") {
      updateDraft("llm", "provider", { kind: "embedded" });
    } else if (value === "remote") {
      // Restore last saved remote configuration if available, otherwise default to Ollama local IP
      const savedRemote = settings.llm.provider.kind === "open_ai_compat" && !settings.llm.provider.base_url?.includes("openai.com")
        ? settings.llm.provider
        : null;
      updateDraft("llm", "provider", { 
        kind: "open_ai_compat", 
        base_url: savedRemote?.base_url || "http://127.0.0.1:11434",
        api_key: savedRemote?.api_key || "",
        provider_name: savedRemote?.provider_name || "Ollama",
        model: savedRemote?.model || ""
      });
    } else if (value === "cloud") {
      // Restore last saved cloud configuration if available, otherwise default to OpenAI
      const savedCloud = settings.llm.provider.kind === "open_ai_compat" && settings.llm.provider.base_url?.includes("openai.com")
        ? settings.llm.provider
        : null;
      updateDraft("llm", "provider", {
        kind: "open_ai_compat",
        base_url: savedCloud?.base_url || CLOUD_PROVIDERS[0].url,
        api_key: savedCloud?.api_key || "",
        provider_name: savedCloud?.provider_name || CLOUD_PROVIDERS[0].name,
        model: savedCloud?.model || ""
      });
    }
  };

  const handleHostPortChange = (h: string, p: string) => {
    setHost(h);
    setPort(p);
    let base_url = "";
    if (h) {
      if (h.startsWith("http://") || h.startsWith("https://")) {
        base_url = h;
      } else {
        base_url = p ? `http://${h}:${p}` : `http://${h}`;
      }
    }
    updateDraft("llm", "provider", {
      ...currentProvider,
      base_url,
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
    const nextIdx = direction === "left"
      ? (currentIdx - 1 + CLOUD_PROVIDERS.length) % CLOUD_PROVIDERS.length
      : (currentIdx + 1) % CLOUD_PROVIDERS.length;
      
    updateDraft("llm", "provider", {
      ...currentProvider,
      base_url: CLOUD_PROVIDERS[nextIdx].url,
      provider_name: CLOUD_PROVIDERS[nextIdx].name,
      api_key: currentProvider.provider_name === CLOUD_PROVIDERS[nextIdx].name ? currentProvider.api_key : ""
    });
  };

  const renderLlmStatusBadge = () => {
    if (providerPill === "local") return null;
    if (checkingHealth) {
      return (
        <span className="text-[11px] font-bold text-yellow-400 animate-pulse flex items-center gap-1">
          <RefreshCw size={11} className="animate-spin" /> Ping
        </span>
      );
    }
    if (isHealthy === true) {
      return (
        <span className="text-[11px] font-bold text-emerald-400 flex items-center gap-1 bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 rounded-md shadow-[0_0_8px_rgba(16,185,129,0.2)]">
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

  // Resolve sub-label status names for segmented controls dynamically
  const remoteSubLabel = currentProvider.kind === "open_ai_compat" && providerPill === "remote"
    ? (currentProvider.provider_name || "Ollama")
    : (settings.llm.provider.kind === "open_ai_compat" && !settings.llm.provider.base_url?.includes("openai.com")
        ? (settings.llm.provider.provider_name || "-")
        : "-");

  const cloudSubLabel = currentProvider.kind === "open_ai_compat" && providerPill === "cloud"
    ? (currentProvider.provider_name || "OpenAI")
    : (settings.llm.provider.kind === "open_ai_compat" && settings.llm.provider.base_url?.includes("openai.com")
        ? (settings.llm.provider.provider_name || "-")
        : "-");

  return (
    <div className={cn(
      "w-full flex flex-col text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
      layoutMode === "small" ? "bg-transparent p-0 h-auto" : "lg:w-[520px] lg:h-[340px] glass-card glass-base p-5"
    )}>
      <style>{interactionStyles}</style>
      
      {/* Header Section */}
      <div className="flex items-center justify-between mb-2 shrink-0">
        <div className="flex items-center gap-2">
          <Sliders className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
            Interaction Console
          </span>
        </div>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">v0.8.4</span>
      </div>

      <div className="flex flex-col gap-3 flex-1 justify-between">
        
        {/* Core Controls Dashboard Grid (2 Buttons) */}
        <div className="grid grid-cols-2 gap-2 shrink-0">
          
          {/* Card 1: Trigger Mode (Continuous vs PTT) */}
          <div 
            onClick={() => updateDraft("interaction", "main_app_mode", isPassive ? "PTT" : "Passive")}
            className="p-3 rounded-xl border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between min-h-[85px] h-[85px] cursor-pointer group relative overflow-hidden"
          >
            <div className="flex items-center justify-between">
              <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">Trigger</span>
              <div className="flex items-center gap-3">
                {layoutMode === "small" ? (
                  <RefreshCw size={10} className="text-[rgb(var(--accent))]/70 group-hover:rotate-180 transition-transform duration-500 shrink-0" />
                ) : (
                  <span className="text-[9px] tracking-wider text-[rgb(var(--accent))]/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300">Click to Toggle</span>
                )}
                {isPassive ? <Activity size={12} className="text-[rgb(var(--accent))]" /> : <Radio size={12} className="text-[rgb(var(--accent))]" />}
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

          {/* Card 2: Pipeline Mode (Modular vs Realtime) */}
          <div 
            onClick={() => updateDraft("interaction", "pipeline_mode", isModular ? "realtime" : "modular")}
            className="p-3 rounded-xl border border-[rgba(var(--accent),0.05)] bg-[rgba(var(--foreground),0.01)] hover:border-[rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.02)] transition-all duration-300 flex flex-col justify-between min-h-[85px] h-[85px] cursor-pointer group relative overflow-hidden"
          >
            <div className="flex items-center justify-between">
              <span className="text-[11px] uppercase font-bold tracking-widest text-[rgb(var(--foreground-muted))]/70">Pipeline</span>
              <div className="flex items-center gap-3">
                {layoutMode === "small" ? (
                  <RefreshCw size={10} className="text-[rgb(var(--accent))]/70 group-hover:rotate-180 transition-transform duration-500 shrink-0" />
                ) : (
                  <span className="text-[8px] tracking-wider text-[rgb(var(--accent))]/70 opacity-0 group-hover:opacity-100 transition-opacity duration-300">Click to Switch</span>
                )}
                {isModular ? <Layers size={12} className="text-[rgb(var(--accent))]" /> : <Zap size={12} className="text-[rgb(var(--accent))]" />}
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

        </div>

        {/* Intelligence Mode Segmented Switcher (Full Width) */}
        <div className="shrink-0">
          {isModular ? (
            <div className="flex bg-[rgba(var(--foreground),0.02)] p-0.5 rounded-xl border border-[rgba(var(--border),0.05)] w-full">
              {[
                { id: "local", label: "Local Core", sub: "GGUF" },
                { id: "remote", label: "Remote Host", sub: remoteSubLabel },
                { id: "cloud", label: "Cloud API", sub: cloudSubLabel }
              ].map(mode => (
                <button 
                  key={mode.id}
                  onClick={() => handleLlmPillChange(mode.id)}
                  className={cn(
                    "flex-1 py-1 rounded-lg flex flex-col items-center justify-center transition-all duration-300 relative border border-transparent h-[36px]",
                    providerPill === mode.id 
                      ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] shadow-[inset_0_1px_0_0_rgba(255,255,255,0.05)] scale-[1.01]" 
                      : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.01)]"
                  )}
                >
                  <span className="text-[11px] font-bold uppercase tracking-wider leading-none">{mode.label}</span>
                  <span className={cn(
                    "text-[11px] font-semibold mt-0.5 opacity-60 uppercase leading-none truncate max-w-[130px]",
                    providerPill === mode.id ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/60"
                  )}>{mode.sub}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className="flex items-center justify-between px-3.5 py-1.5 rounded-xl border border-[rgb(var(--accent))]/15 bg-[rgb(var(--accent))]/5 text-[rgb(var(--accent))] h-[38px]">
              <div className="flex flex-col">
                <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] leading-none">Realtime Gateway</span>
                <span className="text-[11px] text-[rgb(var(--accent))]/70 uppercase font-semibold mt-0.5 leading-none">End-to-End Duplex Streaming Enabled</span>
              </div>
              <span className="text-[11px] font-bold bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] px-2 py-0.5 rounded-md border border-[rgb(var(--accent))]/10 animate-pulse">
                v0.8.5 Preview
              </span>
            </div>
          )}
        </div>

        {/* Persistent Height Config Desk (Reduced Height) */}
        <div className="h-[120px] min-h-[120px] max-h-[120px] w-full flex flex-col glass-whisper glass-base rounded-xl p-3 relative overflow-hidden border border-[rgba(var(--accent),0.06)]">
          
          {/* STATE 1: Modular + Local Core */}
          {isModular && providerPill === "local" && (
            <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
              {/* Left Side: Ambient Breathing Core */}
              <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
                <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/5 animate-ring-pulse-slow" />
                <div className="absolute w-14 h-14 rounded-full border border-[rgb(var(--accent))]/15 animate-pulse-slow" />
                <div className="absolute w-10 h-10 rounded-full border border-[rgb(var(--accent))]/25 animate-pulse" />
                <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center shadow-[0_0_15px_rgba(var(--accent),0.25)] relative z-10">
                  <Brain className="text-[rgb(var(--accent))]" size={14} />
                </div>
              </div>
              
              {/* Right Side: Active Engine Status */}
              <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
                <div className="flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)] animate-pulse" />
                  <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">Local Core Ready</span>
                </div>
                <div className="space-y-0.5 text-[11px] text-[rgb(var(--foreground-muted))]/70 font-medium">
                  <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                    <span>PIPELINE</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))]">LOW-LATENCY HYBRID</span>
                  </div>
                  <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                    <span>VAD SENSE</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))]">EARSHOT RUST</span>
                  </div>
                  <div className="flex justify-between">
                    <span>LLM ENGINE</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))]">LOCAL GGUF</span>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* STATE 2: Modular + Remote Ollama */}
          {isModular && providerPill === "remote" && (
            <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
              <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
                <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <Network size={12} className="text-[rgb(var(--accent))]" />
                  Remote Host Settings
                </span>
                {renderLlmStatusBadge()}
              </div>

              {/* Single Line Input Layout */}
              <div className="grid grid-cols-[1.5fr_0.7fr_1.8fr] gap-2 items-end flex-1 pb-1">
                <div className="space-y-1">
                  <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">Host IP / Domain</label>
                  <input
                    type="text"
                    value={host}
                    onChange={(e) => handleHostPortChange(e.target.value, port)}
                    placeholder="127.0.0.1"
                    className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] focus:border-[rgb(var(--accent))] rounded-lg px-2 py-1 text-[11px] text-[rgb(var(--foreground))] focus:outline-none transition-colors"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">Port</label>
                  <input
                    type="text"
                    value={port}
                    onChange={(e) => handleHostPortChange(host, e.target.value)}
                    placeholder="11434"
                    className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] focus:border-[rgb(var(--accent))] rounded-lg px-2 py-1 text-[11px] text-[rgb(var(--foreground))] focus:outline-none transition-colors text-center"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">API Key (Optional)</label>
                  <div className="relative">
                    <input
                      type={showApiKey ? "text" : "password"}
                      value={apiKey}
                      onChange={(e) => handleApiKeyChange(e.target.value)}
                      placeholder="Bearer token..."
                      className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] focus:border-[rgb(var(--accent))] rounded-lg pl-2 pr-7 py-1 text-[11px] text-[rgb(var(--foreground))] font-mono focus:outline-none transition-colors"
                    />
                    <button
                      type="button"
                      onClick={() => setShowApiKey(!showApiKey)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors"
                    >
                      {showApiKey ? <EyeOff size={11} /> : <Eye size={11} />}
                    </button>
                  </div>
                </div>
              </div>

              {modelsError && (
                <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
                  <AlertCircle size={11} /> {modelsError}
                </span>
              )}
            </div>
          )}

          {/* STATE 3: Modular + Cloud API */}
          {isModular && providerPill === "cloud" && (
            <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
              <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1 shrink-0">
                <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <Cloud size={12} className="text-[rgb(var(--accent))]" />
                  Cloud API Settings
                </span>
                {renderLlmStatusBadge()}
              </div>

              {/* Single Line Switcher + Key Layout */}
              <div className="grid grid-cols-[1.5fr_2fr] gap-3 items-end flex-1 pb-1">
                {/* Carousel Provider Selector */}
                <div className="space-y-1">
                  <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">Cloud Provider</label>
                  <div className="flex items-center justify-between bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] rounded-lg h-[26px] px-1.5">
                    <button 
                      onClick={() => handleCloudCycle("left")}
                      className="p-0.5 text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors active:scale-90"
                    >
                      <ChevronLeft size={14} />
                    </button>
                    <span className="text-[11px] font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
                      {CLOUD_PROVIDERS[cloudIndex].name}
                    </span>
                    <button 
                      onClick={() => handleCloudCycle("right")}
                      className="p-0.5 text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors active:scale-90"
                    >
                      <ChevronRight size={14} />
                    </button>
                  </div>
                </div>

                {/* API Key Input */}
                <div className="space-y-1">
                  <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75 ml-0.5">API Key (Required)</label>
                  <div className="relative">
                    <input
                      type={showApiKey ? "text" : "password"}
                      value={apiKey}
                      onChange={(e) => handleApiKeyChange(e.target.value)}
                      placeholder={CLOUD_PROVIDERS[cloudIndex].keyPlaceholder}
                      className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.15)] focus:border-[rgb(var(--accent))] rounded-lg pl-2 pr-7 py-1 text-[11px] text-[rgb(var(--foreground))] font-mono focus:outline-none transition-colors"
                    />
                    <button
                      type="button"
                      onClick={() => setShowApiKey(!showApiKey)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors"
                    >
                      {showApiKey ? <EyeOff size={11} /> : <Eye size={11} />}
                    </button>
                  </div>
                </div>
              </div>

              {modelsError && (
                <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
                  <AlertCircle size={11} /> {modelsError}
                </span>
              )}
            </div>
          )}

          {/* STATE 4: Realtime Gateway Details */}
          {!isModular && (
            <div className="flex items-center justify-between h-full gap-4 animate-fade-in px-2">
              
              {/* Left Column: Visual Gateway pulsing channel */}
              <div className="flex-1 flex items-center justify-center relative min-w-[90px] h-full">
                <div className="absolute w-20 h-20 rounded-full border border-[rgb(var(--accent))]/10 animate-ring-pulse-slow" />
                <div className="absolute w-12 h-12 rounded-full border border-[rgb(var(--accent))]/25 animate-pulse" />
                <div className="w-8 h-8 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/40 flex items-center justify-center shadow-[0_0_15px_rgba(var(--accent),0.25)] relative z-10 animate-pulse">
                  <Globe className="text-[rgb(var(--accent))]" size={14} />
                </div>
              </div>

              {/* Right Column: Connection Info */}
              <div className="flex-[2] flex flex-col justify-center gap-1.5 h-full">
                <div className="flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent-rgb),0.6)] animate-pulse" />
                  <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]/80">Gateway Active</span>
                </div>
                <div className="space-y-0.5 text-[11px] text-[rgb(var(--foreground-muted))]/70 font-medium">
                  <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                    <span>WS URI</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))] truncate max-w-[150px]">wss://gateway.vox.ai/v1</span>
                  </div>
                  <div className="flex justify-between border-b border-[rgba(var(--border),0.04)] pb-0.5">
                    <span>SECURITY</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))]">TLS DUPLEX SECURE</span>
                  </div>
                  <div className="flex justify-between">
                    <span>FALLBACK</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--accent))]">LOCAL HYBRID LOOP</span>
                  </div>
                </div>
              </div>

            </div>
          )}

        </div>
      </div>
    </div>
  );
});

InteractionCard.displayName = "InteractionCard";
