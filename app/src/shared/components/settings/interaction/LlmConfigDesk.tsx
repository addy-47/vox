import { useState, useEffect, memo, useCallback, useMemo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { checkIfCloudUrl, CLOUD_PROVIDERS } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  Brain, Cloud, Network, Volume2, Sparkles, Mic,
  RefreshCw, AlertCircle, ArrowLeft, Server
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField, UnderlineInput, CarouselSelector } from "@/shared/ui";

interface LlmConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: "local" | "remote" | "cloud";
  isModular: boolean;
  onBack?: () => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const LlmConfigDesk = memo(({
  activeCategory,
  activePill,
  isModular,
  onBack,
  layoutMode,
}: LlmConfigDeskProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);

  if (!draftSettings || !settings) return null;

  const activeLlmProvider = draftSettings.llm.active || "embedded";
  const currentRemoteConfig =
    activeLlmProvider === "server"
      ? draftSettings.llm.server
      : activeLlmProvider === "cloud"
      ? draftSettings.llm.cloud
      : null;

  const currentProvider: LlmProviderConfig = useMemo(() => {
    return activeLlmProvider === "embedded"
      ? { kind: "embedded" }
      : {
          kind: "open_ai_compat",
          base_url: currentRemoteConfig?.base_url || "",
          model: currentRemoteConfig?.model || "",
          api_key: currentRemoteConfig?.api_key || undefined,
          provider_name: currentRemoteConfig?.provider_name || undefined,
        };
  }, [
    activeLlmProvider,
    currentRemoteConfig?.base_url,
    currentRemoteConfig?.model,
    currentRemoteConfig?.api_key,
    currentRemoteConfig?.provider_name,
  ]);

  const isCloudUrl = checkIfCloudUrl(currentProvider.base_url || "");
  const providerPill = currentProvider.kind === "embedded" ? "local" : isCloudUrl ? "cloud" : "remote";

  const getCloudProviderIndex = (u: string) => {
    const idx = CLOUD_PROVIDERS.findIndex(
      (p) => u.includes(p.id) || (u.includes("google") && p.id === "gemini")
    );
    return idx === -1 ? 0 : idx;
  };
  const cloudIndex = getCloudProviderIndex(currentProvider.base_url || "");

  const url =
    activeLlmProvider === "server"
      ? draftSettings.llm.server?.base_url ?? ""
      : activeLlmProvider === "cloud"
      ? draftSettings.llm.cloud?.base_url ?? ""
      : "";

  const apiKey =
    activeLlmProvider === "server"
      ? draftSettings.llm.server?.api_key ?? ""
      : activeLlmProvider === "cloud"
      ? draftSettings.llm.cloud?.api_key ?? ""
      : "";

  const remoteTtsEndpoint = draftSettings.tts.chatterbox_remote?.endpoint ?? "";
  const remoteTtsPath = draftSettings.tts.chatterbox_remote?.remote_path ?? "";

  const handleRemoteTtsEndpointChange = useCallback(
    (val: string) => {
      updateDraft("tts", "chatterbox_remote", {
        ...draftSettings.tts.chatterbox_remote,
        endpoint: val || "http://127.0.0.1:7860",
      });
    },
    [draftSettings.tts.chatterbox_remote, updateDraft]
  );

  const handleRemoteTtsPathChange = useCallback(
    (val: string) => {
      updateDraft("tts", "chatterbox_remote", {
        ...draftSettings.tts.chatterbox_remote,
        remote_path: val || "~/.vox",
      });
    },
    [draftSettings.tts.chatterbox_remote, updateDraft]
  );

  const providerBaseUrl = currentProvider.base_url;
  const providerApiKey = currentProvider.api_key;
  const providerKind = currentProvider.kind;

  useEffect(() => {
    if (providerKind !== "open_ai_compat" || !providerBaseUrl) {
      setIsHealthy(null);
      setModelsError(null);
      return;
    }

    let isMounted = true;
    const timer = setTimeout(() => {
      const runChecks = async () => {
        if (!isMounted) return;
        setCheckingHealth(true);
        setModelsError(null);
        try {
          const healthy = await checkLlmProviderHealth(currentProvider);
          if (!isMounted) return;
          setIsHealthy(healthy);

          if (healthy && providerPill === "remote" && activeLlmProvider === "server") {
            const detectedName = providerBaseUrl?.includes("11434")
              ? "Ollama"
              : "Remote Host";
            const currentServer = useSettingsStore.getState().draftSettings?.llm?.server;
            if (currentServer && currentServer.provider_name !== detectedName) {
              updateDraft("llm", "server", {
                ...currentServer,
                provider_name: detectedName,
              });
            }
          }
        } catch (err) {
          if (!isMounted) return;
          console.error(err);
          setIsHealthy(false);
          setModelsError("Connection failed");
        } finally {
          if (isMounted) {
            setCheckingHealth(false);
          }
        }
      };
      runChecks();
    }, 500);

    return () => {
      isMounted = false;
      clearTimeout(timer);
    };
  }, [
    providerKind,
    providerBaseUrl,
    providerApiKey,
    providerPill,
    activeLlmProvider,
    updateDraft,
  ]);

  const handleUrlChange = useCallback(
    (val: string) => {
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
    },
    [activeLlmProvider, draftSettings.llm.server, draftSettings.llm.cloud, updateDraft]
  );

  const handleApiKeyChange = useCallback(
    (key: string) => {
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
    },
    [activeLlmProvider, draftSettings.llm.server, draftSettings.llm.cloud, updateDraft]
  );

  const handleCloudCycle = useCallback(
    (direction: "left" | "right") => {
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
    },
    [currentProvider.base_url, draftSettings.llm.cloud, updateDraft]
  );

  // ── Standard Level-2 Header ──────────────────────────────────────────────
  // Left: breadcrumb back button + stage/modality title
  // Right: status indicator pill / badge
  const renderHeader = (
    icon: React.ReactNode,
    title: string,
    badge?: React.ReactNode
  ) => {
    const isNetwork = activePill === "remote" || activePill === "cloud";

    const defaultBadge = () => {
      if (isNetwork) {
        if (checkingHealth) {
          return (
            <span className="inline-flex items-center gap-1.5 text-[10px] font-bold text-amber-400 px-2 py-0.5 rounded-full bg-amber-400/10 border border-amber-400/20 animate-pulse">
              <RefreshCw size={9} className="animate-spin" />
              Testing
            </span>
          );
        }
        if (isHealthy === false) {
          return (
            <span className="inline-flex items-center gap-1.5 text-[10px] font-bold text-rose-400 px-2 py-0.5 rounded-full bg-rose-400/10 border border-rose-400/20">
              <span className="w-1.5 h-1.5 rounded-full bg-rose-400 shrink-0" />
              Offline
            </span>
          );
        }
        if (isHealthy === true) {
          return (
            <span className="inline-flex items-center gap-1.5 text-[10px] font-bold text-emerald-400 px-2 py-0.5 rounded-full bg-emerald-400/10 border border-emerald-400/20">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.9)] shrink-0" />
              Online
            </span>
          );
        }
      }
      return (
        <span className="inline-flex items-center gap-1.5 text-[10px] font-bold text-emerald-400 px-2 py-0.5 rounded-full bg-emerald-400/10 border border-emerald-400/20">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.9)] shrink-0" />
          Active
        </span>
      );
    };

    return (
      <div className="flex items-center justify-between gap-2 shrink-0 pb-2 border-b border-[rgba(var(--accent),0.08)]">
        {/* Left: Breadcrumb Back + Title */}
        <div className="flex items-center gap-2 min-w-0">
          {onBack && (
            <button
              type="button"
              onClick={onBack}
              className="inline-flex items-center gap-1 text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer shrink-0 group"
              aria-label="Back to providers"
            >
              <ArrowLeft size={12} strokeWidth={2.5} className="group-hover:-translate-x-0.5 transition-transform" />
              <span>Providers</span>
            </button>
          )}
          {onBack && <span className="text-[rgb(var(--foreground-muted))]/30 text-[10px] shrink-0">/</span>}
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="shrink-0 text-[rgb(var(--accent))]">{icon}</span>
            <span className="font-display font-bold text-[12px] sm:text-[12.5px] text-[rgb(var(--foreground))]/90 truncate">
              {title}
            </span>
          </div>
        </div>

        {/* Right: Badge */}
        <div className="shrink-0">
          {badge !== undefined ? badge : defaultBadge()}
        </div>
      </div>
    );
  };

  const copy = INTERACTION_CONFIG_DESK_COPY;

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] p-3 sm:p-3.5 gap-2.5 justify-between animate-fade-in",
        layoutMode === "small" ? "h-auto min-h-0" : "h-full"
      )}
    >
      {/* ─── SECTION 0: INTEGRATED PIPELINE (MODE = INTEGRATED) ─── */}
      {!isModular && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Sparkles size={14} className="text-[rgb(var(--accent))]" />,
            copy.integrated.title,
            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
              {copy.integrated.badge}
            </span>
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.integrated.description}
            </p>
          </div>
        </div>
      )}

      {/* ─── SECTION 1: STT CATEGORY ─── */}
      {isModular && activeCategory === "STT" && activePill === "local" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Mic size={14} className="text-[rgb(var(--accent))]" />,
            copy.stt.local.title
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.stt.local.description}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "remote" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Server size={14} className="text-[rgb(var(--accent))]" />,
            copy.stt.remote.title,
            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
              {copy.stt.remote.badge}
            </span>
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.stt.remote.description}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "STT" && activePill === "cloud" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Cloud size={14} className="text-[rgb(var(--accent))]" />,
            copy.stt.cloud.title,
            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
              {copy.stt.cloud.badge}
            </span>
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.stt.cloud.description}
            </p>
          </div>
        </div>
      )}

      {/* ─── SECTION 2: LLM CATEGORY ─── */}
      {isModular && activeCategory === "LLM" && activePill === "local" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Brain size={14} className="text-[rgb(var(--accent))]" />,
            copy.llm.local.title
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.llm.local.description}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "LLM" && activePill === "remote" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          {renderHeader(<Network size={14} className="text-[rgb(var(--accent))]" />, copy.llm.remote.title)}

          <div
            className={cn(
              "grid gap-3 items-end flex-1 pb-0.5",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
            )}
          >
            <UnderlineInput
              label={copy.llm.remote.urlLabel}
              value={url}
              onChange={(e) => handleUrlChange(e.target.value)}
              placeholder={copy.llm.remote.urlPlaceholder}
            />
            <ApiKeyField
              label={copy.llm.remote.apiKeyLabel}
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder={copy.llm.remote.apiKeyPlaceholder}
            />
          </div>

          {modelsError && (
            <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
              <AlertCircle size={13} /> {modelsError}
            </span>
          )}
        </div>
      )}

      {isModular && activeCategory === "LLM" && activePill === "cloud" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          {renderHeader(<Cloud size={14} className="text-[rgb(var(--accent))]" />, copy.llm.cloud.title)}

          <div className={cn("grid gap-5 flex-1 min-h-0 items-end pb-0.5", layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2")}>
            <CarouselSelector
              label={copy.llm.cloud.providerLabel}
              value={CLOUD_PROVIDERS[cloudIndex].name}
              onPrev={() => handleCloudCycle("left")}
              onNext={() => handleCloudCycle("right")}
            />

            <ApiKeyField
              label={copy.llm.cloud.apiKeyLabel}
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder={CLOUD_PROVIDERS[cloudIndex].keyPlaceholder}
              error={!apiKey?.trim()}
            />
          </div>

          {modelsError && (
            <span className="text-[11px] text-red-400/80 flex items-center gap-1 ml-0.5 shrink-0">
              <AlertCircle size={13} /> {modelsError}
            </span>
          )}
        </div>
      )}

      {/* ─── SECTION 3: TTS CATEGORY ─── */}
      {isModular && activeCategory === "TTS" && activePill === "local" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Volume2 size={14} className="text-[rgb(var(--accent))]" />,
            copy.tts.local.title
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.tts.local.description}
            </p>
          </div>
        </div>
      )}

      {isModular && activeCategory === "TTS" && activePill === "remote" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          {renderHeader(<Network size={14} className="text-[rgb(var(--accent))]" />, copy.tts.remote.title)}

          <div className="grid grid-cols-2 gap-3 pb-0.5">
            <UnderlineInput
              label={copy.tts.remote.urlLabel}
              value={remoteTtsEndpoint}
              onChange={(e) => handleRemoteTtsEndpointChange(e.target.value)}
              placeholder={copy.tts.remote.urlPlaceholder}
            />
            <UnderlineInput
              label={copy.tts.remote.pathLabel}
              value={remoteTtsPath}
              onChange={(e) => handleRemoteTtsPathChange(e.target.value)}
              placeholder={copy.tts.remote.pathPlaceholder}
            />
          </div>
        </div>
      )}

      {isModular && activeCategory === "TTS" && activePill === "cloud" && (
        <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
          {renderHeader(
            <Sparkles size={14} className="text-[rgb(var(--accent))]" />,
            copy.tts.cloud.title,
            <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
              {copy.tts.cloud.badge}
            </span>
          )}
          <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
            <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
              {copy.tts.cloud.description}
            </p>
          </div>
        </div>
      )}
    </div>
  );
});

LlmConfigDesk.displayName = "LlmConfigDesk";
