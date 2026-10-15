import { useState, useEffect, memo, useCallback, useMemo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { CLOUD_PROVIDERS, CloudProvider } from "@/data/providersCopy";
import {
  INTERACTION_CONFIG_DESK_COPY,
  PROVIDERS_HEADER_COPY,
  PIPELINE_PROVIDERS_DESK_COPY,
} from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  Brain,
  Cloud,
  Network,
  Volume2,
  Sparkles,
  Mic,
  AlertCircle,
  Server,
  Globe,
  Key,
  KeyRound,
  Check,
  X,
  Pencil,
  HardDrive,
  ChevronDown,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField, ExpandableList, UnderlineInput } from "@/shared/ui";
import { ProviderLogo } from "@/shared/components/settings/models/ProviderLogo";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { ProviderTier } from "./ProviderSelectorView";
import { RemoteModelsModalView } from "@/shared/components/settings/models/LlmCatalogView";
import { CloudProvidersModalView } from "@/shared/components/settings/models/CloudProvidersModalView";
import {
  CatalogFilterItem,
  CatalogFilterMenu,
  type CatalogSortOrder,
} from "@/shared/components/settings/models/catalog/CatalogHeader";
import { ProvidersHeader } from "@/shared/components/settings/models/catalog/ProvidersHeader";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";

export interface LlmConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill?: ProviderTier;
  isModular: boolean;
  onBack?: () => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const LlmConfigDesk = memo(({
  activeCategory,
  onBack,
  layoutMode,
}: LlmConfigDeskProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const draftLlm = draftSettings?.llm;
  const draftChatterboxRemote = draftSettings?.tts?.chatterbox_remote;
  const hasDraftSettings = Boolean(draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Subview toggle: "catalog" (default provider list) vs "server" (+ Custom endpoint setup)
  const [subView, setSubView] = useState<"catalog" | "server">("catalog");

  // Local state for custom server form fields
  const [customServerUrl, setCustomServerUrl] = useState(draftLlm?.server?.base_url || "http://127.0.0.1:11434");
  const [customServerKey, setCustomServerKey] = useState(draftLlm?.server?.api_key || "");
  const [customServerName, setCustomServerName] = useState(draftLlm?.server?.provider_name || "");
  const [customTtsEndpoint, setCustomTtsEndpoint] = useState(draftChatterboxRemote?.endpoint || "http://127.0.0.1:7860");
  const [customTtsPath, setCustomTtsPath] = useState(draftChatterboxRemote?.remote_path || "~/.vox");
  const [showHealthErrorConfirm, setShowHealthErrorConfirm] = useState(false);

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [detectedDialect, setDetectedDialect] = useState<string | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [sortOrder, setSortOrder] = useState<CatalogSortOrder>("az");
  const [providerFilter, setProviderFilter] = useState<
    "all" | "local" | "cloud" | "configured" | "popular" | "unconfigured"
  >("all");
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState("");
  const [modalView, setModalView] = useState<"providers" | "models">("providers");

  const activeLlmProvider = draftLlm?.active || "embedded";
  const currentRemoteConfig =
    activeLlmProvider === "server"
      ? draftLlm?.server
      : activeLlmProvider === "cloud"
      ? draftLlm?.cloud
      : null;

  const currentProvider: LlmProviderConfig = useMemo(() => {
    if (activeLlmProvider === "embedded") {
      return { kind: "embedded" };
    }
    if (activeLlmProvider === "server") {
      return {
        kind: "server",
        base_url: currentRemoteConfig?.base_url || "",
        model: currentRemoteConfig?.model || "",
        api_key: currentRemoteConfig?.api_key || undefined,
        provider_name: currentRemoteConfig?.provider_name || undefined,
      };
    }
    return {
      kind: "cloud",
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

  const {
    remoteModels,
    probingMap,
    handleProbeCapabilities,
  } = useRemoteLlmProbing(currentProvider, "llm", activeCategory === "LLM");

  const activeCloudProviderId = useMemo(() => {
    const pName = draftLlm?.cloud?.provider_name;
    if (!pName) return "custom";
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : "custom";
  }, [draftLlm?.cloud?.provider_name]);

  const committedActiveCloudProviderId = useMemo(() => {
    const pName = committedSettings?.llm?.cloud?.provider_name;
    if (!pName) return "custom";
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : "custom";
  }, [committedSettings?.llm?.cloud?.provider_name]);

  const getProviderKey = useCallback(
    (providerId: string) => {
      const keys = draftLlm?.cloud_keys;
      if (keys && keys[providerId]) return keys[providerId];
      const target = CLOUD_PROVIDERS.find((p) => p.id === providerId);
      if (
        draftLlm?.cloud?.api_key &&
        target &&
        draftLlm.cloud.provider_name?.toLowerCase() === target.name.toLowerCase()
      ) {
        return draftLlm.cloud.api_key;
      }
      return undefined;
    },
    [draftLlm?.cloud_keys, draftLlm?.cloud]
  );

  const getCommittedProviderKey = useCallback(
    (providerId: string) => {
      const keys = committedSettings?.llm?.cloud_keys;
      if (keys && keys[providerId]) return keys[providerId];
      const target = CLOUD_PROVIDERS.find((p) => p.id === providerId);
      if (
        committedSettings?.llm?.cloud?.api_key &&
        target &&
        committedSettings.llm.cloud.provider_name?.toLowerCase() === target.name.toLowerCase()
      ) {
        return committedSettings.llm.cloud.api_key;
      }
      return undefined;
    },
    [committedSettings?.llm?.cloud_keys, committedSettings?.llm?.cloud]
  );

  const openSearch = useCallback(() => setIsSearchOpen(true), []);
  const closeSearch = useCallback(() => {
    setIsSearchOpen(false);
    setSearchQuery("");
  }, []);

  const providerFilterCounts = useMemo(() => {
    let configured = 0;
    let popular = 0;
    let unconfigured = 0;
    for (const p of CLOUD_PROVIDERS) {
      const key = getProviderKey(p.id);
      if (key && key.trim().length > 0) configured += 1;
      else unconfigured += 1;
      if (p.popular) popular += 1;
    }
    return {
      all: CLOUD_PROVIDERS.length + 1,
      configured,
      popular,
      unconfigured,
    };
  }, [getProviderKey]);

  const filteredProviders = useMemo(() => {
    let list = CLOUD_PROVIDERS;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      list = CLOUD_PROVIDERS.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.id.toLowerCase().includes(q) ||
          p.tagline?.toLowerCase().includes(q)
      );
    }

    if (providerFilter === "configured") {
      list = list.filter((p) => Boolean(getProviderKey(p.id)?.trim()));
    } else if (providerFilter === "popular") {
      list = list.filter((p) => p.popular);
    } else if (providerFilter === "unconfigured") {
      list = list.filter((p) => !getProviderKey(p.id)?.trim());
    } else if (providerFilter === "local") {
      list = [];
    }

    return [...list].sort((a, b) => {
      const aKey = Boolean(getCommittedProviderKey(a.id)?.trim());
      const bKey = Boolean(getCommittedProviderKey(b.id)?.trim());

      if (aKey && !bKey) return -1;
      if (!aKey && bKey) return 1;

      const cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      return sortOrder === "az" ? cmp : -cmp;
    });
  }, [searchQuery, sortOrder, providerFilter, getProviderKey, getCommittedProviderKey]);

  const resetProviderFilter = useCallback(() => setProviderFilter("all"), []);

  const { containerRef: providerGridRef, lenisRef: providerLenisRef } =
    useLenisScrollContainer<HTMLDivElement>();

  const providerOrderFingerprint = useMemo(
    () =>
      `${committedActiveCloudProviderId}|${CLOUD_PROVIDERS.map((p) =>
        getCommittedProviderKey(p.id)?.trim() ? "1" : "0"
      ).join("")}`,
    [committedActiveCloudProviderId, getCommittedProviderKey]
  );
  useScrollTopOnChange(providerGridRef, providerOrderFingerprint, providerLenisRef);



  const providerBaseUrl = currentProvider.base_url;
  const providerApiKey = currentProvider.api_key;
  const providerKind = currentProvider.kind;

  useEffect(() => {
    if (providerKind === "embedded" || !providerBaseUrl) {
      setIsHealthy(null);
      setDetectedDialect(null);
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
          const res = await checkLlmProviderHealth(currentProvider);
          if (!isMounted) return;
          setIsHealthy(res.healthy);
          setDetectedDialect(res.dialect ?? null);
          setModelsError(
            res.healthy ? null : (res.reason ?? INTERACTION_CONFIG_DESK_COPY.status.connectionFailed)
          );

          if (res.healthy && res.dialect && activeLlmProvider === "server") {
            const currentServer = useSettingsStore.getState().draftSettings?.llm?.server;
            if (currentServer && currentServer.provider_name !== res.dialect) {
              updateDraft("llm", "server", {
                ...currentServer,
                provider_name: res.dialect,
              });
            }
          }
        } catch (err) {
          if (!isMounted) return;
          console.error(err);
          setIsHealthy(false);
          setDetectedDialect(null);
          setModelsError(INTERACTION_CONFIG_DESK_COPY.status.connectionFailed);
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
    activeLlmProvider,
    updateDraft,
  ]);



  const handleSelectCloudProvider = useCallback(
    (provider: CloudProvider) => {
      const savedKey = getProviderKey(provider.id);
      updateDraft("llm", "active", "cloud");
      updateDraft("llm", "cloud", {
        base_url: provider.url,
        provider_name: provider.name,
        api_key: savedKey || null,
        model: draftLlm?.cloud?.model || "",
      });
    },
    [getProviderKey, draftLlm?.cloud?.model, updateDraft]
  );

  const handleStartInlineEdit = useCallback((providerId: string, currentKey?: string) => {
    setEditingProviderId(providerId);
    setEditingKeyValue(currentKey || "");
  }, []);

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
  }, []);

  const handleSaveInlineKey = useCallback(
    (providerId: string) => {
      const trimmed = editingKeyValue.trim();
      const currentKeys = { ...(draftLlm?.cloud_keys || {}) };
      if (trimmed) {
        currentKeys[providerId] = trimmed;
      } else {
        delete currentKeys[providerId];
      }

      updateDraft("llm", "cloud_keys", currentKeys);

      const targetProvider = CLOUD_PROVIDERS.find((p) => p.id === providerId);
      if (!targetProvider) {
        setEditingProviderId(null);
        setEditingKeyValue("");
        return;
      }
      const isCurrentlyActive =
        activeCloudProviderId === providerId ||
        draftLlm?.cloud?.provider_name?.toLowerCase() === targetProvider.name.toLowerCase();

      if (isCurrentlyActive) {
        updateDraft("llm", "cloud", {
          ...draftLlm?.cloud,
          base_url: targetProvider.url,
          provider_name: targetProvider.name,
          api_key: trimmed || null,
        });
      }

      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [editingKeyValue, draftLlm?.cloud_keys, draftLlm?.cloud, activeCloudProviderId, updateDraft]
  );

  // Determine active states per category
  const isLocalActive = useMemo(() => {
    if (activeCategory === "STT") {
      return draftSettings?.stt?.active === "embedded";
    }
    if (activeCategory === "LLM") {
      return draftSettings?.llm?.active === "embedded";
    }
    if (activeCategory === "TTS") {
      const ttsId = draftSettings?.tts?.active || "";
      return ttsId !== "edge_tts" && ttsId !== "chatterbox_remote";
    }
    return false;
  }, [activeCategory, draftSettings?.stt?.active, draftSettings?.llm?.active, draftSettings?.tts?.active]);

  const handleSelectLocal = useCallback(() => {
    if (activeCategory === "STT") {
      updateDraft("stt", "active", "embedded");
    } else if (activeCategory === "LLM") {
      updateDraft("llm", "active", "embedded");
    } else if (activeCategory === "TTS") {
      updateDraft("tts", "active", "kokoro");
    }
  }, [activeCategory, updateDraft]);

  const handleOpenCustomServer = useCallback(() => {
    if (activeCategory === "LLM") {
      setCustomServerUrl(draftLlm?.server?.base_url || "http://127.0.0.1:11434");
      setCustomServerKey(draftLlm?.server?.api_key || "");
      setCustomServerName(draftLlm?.server?.provider_name || "");
    } else if (activeCategory === "TTS") {
      setCustomTtsEndpoint(draftChatterboxRemote?.endpoint || "http://127.0.0.1:7860");
      setCustomTtsPath(draftChatterboxRemote?.remote_path || "~/.vox");
    }
    setShowHealthErrorConfirm(false);
    setModelsError(null);
    setSubView("server");
  }, [activeCategory, draftLlm?.server, draftChatterboxRemote]);

  const handleCancelCustomServer = useCallback(() => {
    setShowHealthErrorConfirm(false);
    setModelsError(null);
    setSubView("catalog");
  }, []);

  const handleSaveCustomServer = useCallback(async () => {
    if (activeCategory === "LLM") {
      const urlToTest = customServerUrl.trim() || "http://127.0.0.1:11434";
      setCheckingHealth(true);
      setModelsError(null);
      setShowHealthErrorConfirm(false);
      try {
        const res = await checkLlmProviderHealth({
          kind: "server",
          base_url: urlToTest,
          api_key: customServerKey.trim() || undefined,
          provider_name: customServerName.trim() || undefined,
        });
        if (res.healthy) {
          const finalName = customServerName.trim() || res.dialect || "Custom Server";
          updateDraft("llm", "server", {
            base_url: urlToTest,
            api_key: customServerKey.trim() || null,
            provider_name: finalName,
            model: draftLlm?.server?.model || "",
          });
          updateDraft("llm", "active", "server");
          setIsHealthy(true);
          setDetectedDialect(res.dialect ?? null);
          setSubView("catalog");
        } else {
          setIsHealthy(false);
          setDetectedDialect(null);
          setModelsError(res.reason || INTERACTION_CONFIG_DESK_COPY.status.connectionFailed);
          setShowHealthErrorConfirm(true);
        }
      } catch (err) {
        console.error(err);
        setIsHealthy(false);
        setDetectedDialect(null);
        setModelsError(INTERACTION_CONFIG_DESK_COPY.status.connectionFailed);
        setShowHealthErrorConfirm(true);
      } finally {
        setCheckingHealth(false);
      }
    } else if (activeCategory === "TTS") {
      const endpoint = customTtsEndpoint.trim() || "http://127.0.0.1:7860";
      const remotePath = customTtsPath.trim() || "~/.vox";
      updateDraft("tts", "chatterbox_remote", {
        endpoint,
        remote_path: remotePath,
      });
      updateDraft("tts", "active", "chatterbox_remote");
      setSubView("catalog");
    }
  }, [
    activeCategory,
    customServerUrl,
    customServerKey,
    customServerName,
    customTtsEndpoint,
    customTtsPath,
    draftLlm?.server?.model,
    updateDraft,
  ]);

  const handleConfirmSaveAnyway = useCallback(() => {
    if (activeCategory === "LLM") {
      const urlToSave = customServerUrl.trim() || "http://127.0.0.1:11434";
      const finalName = customServerName.trim() || "Custom Server";
      updateDraft("llm", "server", {
        base_url: urlToSave,
        api_key: customServerKey.trim() || null,
        provider_name: finalName,
        model: draftLlm?.server?.model || "",
      });
      updateDraft("llm", "active", "server");
    }
    setShowHealthErrorConfirm(false);
    setSubView("catalog");
  }, [activeCategory, customServerUrl, customServerKey, customServerName, draftLlm?.server?.model, updateDraft]);

  if (!hasDraftSettings) return null;

  const copy = INTERACTION_CONFIG_DESK_COPY;

  const categoryTitle =
    activeCategory === "STT"
      ? PIPELINE_PROVIDERS_DESK_COPY.cards.stt.title
      : activeCategory === "TTS"
      ? PIPELINE_PROVIDERS_DESK_COPY.cards.tts.title
      : PIPELINE_PROVIDERS_DESK_COPY.cards.llm.title;

  const categoryIcon =
    activeCategory === "STT" ? (
      <Mic size={14} className="text-[rgb(var(--accent))]" />
    ) : activeCategory === "TTS" ? (
      <Volume2 size={14} className="text-[rgb(var(--accent))]" />
    ) : (
      <Brain size={14} className="text-[rgb(var(--accent))]" />
    );

  const localCardTitle =
    activeCategory === "STT"
      ? "Built-in Speech Recognition"
      : activeCategory === "TTS"
      ? "Built-in Neural Voices"
      : "Built-in Language Model";

  const localCardSubtitle =
    activeCategory === "STT"
      ? "Whisper & Nemotron (100% Offline)"
      : activeCategory === "TTS"
      ? "Kokoro / Supertonic (Offline)"
      : "On-Device GGUF Engine";

  const matchesLocalSearch = useMemo(() => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase().trim();
    return "built-in on-device local neural whisper gguf kokoro vox offline".includes(q);
  }, [searchQuery]);

  const showLocalCard =
    (providerFilter === "all" ||
      providerFilter === "local" ||
      (providerFilter === "configured" && isLocalActive)) &&
    matchesLocalSearch;

  /** Inline render helper for provider cards */
  const ProviderRows = () => (
    <div
      ref={providerGridRef}
      className="h-full overflow-y-auto pr-1 grid grid-cols-1 sm:grid-cols-2 gap-2 content-start custom-scrollbar"
    >
      {/* 1. On-Device Local Option (Always 1st in all categories) */}
      {showLocalCard && (
        <div
          onClick={handleSelectLocal}
          className={cn(
            "group flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl transition-all cursor-pointer select-none border min-h-[44px]",
            isLocalActive
              ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
              : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
          )}
        >
          <div className="flex items-center gap-2.5 min-w-0">
            <ProviderLogo providerId="embedded" size={16} className="shrink-0" />
            <div className="flex flex-col min-w-0">
              <span
                className={cn(
                  "text-[13px] truncate leading-tight",
                  isLocalActive ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium"
                )}
              >
                {localCardTitle}
              </span>
              <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 truncate">
                {localCardSubtitle}
              </span>
            </div>
          </div>

          <span className="text-[9.5px] font-mono px-1.5 py-0.5 rounded font-bold bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shrink-0">
            On-Device
          </span>
        </div>
      )}

      {/* 2. Category-Specific Cloud Providers */}
      {activeCategory === "LLM" &&
        filteredProviders.map((provider) => {
          const isSelected = activeCloudProviderId === provider.id && draftLlm?.active === "cloud";
          const savedKey = getProviderKey(provider.id);
          const isEditing = editingProviderId === provider.id;
          const hasKey = Boolean(savedKey?.trim());

          return (
            <div
              key={provider.id}
              onClick={() => {
                if (!isEditing) {
                  handleSelectCloudProvider(provider);
                }
              }}
              className={cn(
                "group flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl transition-all cursor-pointer select-none border min-h-[44px]",
                isSelected
                  ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              {isEditing ? (
                <div
                  className="w-full flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    type="password"
                    autoFocus
                    value={editingKeyValue}
                    onChange={(e) => setEditingKeyValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSaveInlineKey(provider.id);
                      if (e.key === "Escape") handleCancelInlineKey();
                    }}
                    placeholder={provider.keyPlaceholder}
                    className="w-full bg-transparent text-[11px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30"
                  />
                  <button
                    type="button"
                    onClick={() => handleSaveInlineKey(provider.id)}
                    title={copy.llm.cloud.saveKey}
                    aria-label={copy.llm.cloud.saveKey}
                    className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0"
                  >
                    <Check size={13} />
                  </button>
                  <button
                    type="button"
                    onClick={handleCancelInlineKey}
                    title={copy.llm.cloud.cancelKey}
                    aria-label={copy.llm.cloud.cancelKey}
                    className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0"
                  >
                    <X size={13} />
                  </button>
                </div>
              ) : (
                <>
                  <div className="flex items-center gap-2.5 min-w-0">
                    <ProviderLogo providerId={provider.id} size={16} className="shrink-0" />
                    <span
                      className={cn(
                        "text-[13px] truncate leading-none",
                        isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium"
                      )}
                    >
                      {provider.name}
                    </span>
                  </div>

                  <div className="shrink-0 flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                    {hasKey ? (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, savedKey)}
                        className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                        title={copy.llm.cloud.editKey}
                        aria-label={copy.llm.cloud.editKey}
                      >
                        <Pencil size={12} />
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, "")}
                        className="text-[10.5px] font-medium text-[rgb(var(--accent))] hover:underline transition-all"
                      >
                        {copy.llm.cloud.connect}
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          );
        })}

      {activeCategory === "TTS" && (
        <>
          {/* Edge TTS Cloud Provider */}
          <div
            onClick={() => updateDraft("tts", "active", "edge_tts")}
            className={cn(
              "group flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl transition-all cursor-pointer select-none border min-h-[44px]",
              draftSettings?.tts?.active === "edge_tts"
                ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
            )}
          >
            <div className="flex items-center gap-2.5 min-w-0">
              <ProviderLogo providerId="microsoft" size={16} className="shrink-0" />
              <div className="flex flex-col min-w-0">
                <span className="text-[13px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                  Microsoft Edge TTS
                </span>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/70 truncate">
                  400+ Cloud Voices · Free & Keyless
                </span>
              </div>
            </div>
            <span className="text-[9.5px] font-mono px-1.5 py-0.5 rounded font-bold bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shrink-0">
              Cloud
            </span>
          </div>

          {/* Coming Soon TTS Cloud Providers */}
          <div className="flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[44px] select-none">
            <div className="flex items-center gap-2.5 min-w-0">
              <ProviderLogo providerId="google" size={16} className="shrink-0 opacity-60" />
              <div className="flex flex-col min-w-0">
                <span className="text-[13px] font-medium text-[rgb(var(--foreground))] truncate">
                  Google Cloud TTS
                </span>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))] truncate">
                  Studio and Journey natural neural synthesis
                </span>
              </div>
            </div>
            <span className="text-[9.5px] font-mono px-1.5 py-0.5 rounded font-bold bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] shrink-0">
              Coming Soon
            </span>
          </div>
        </>
      )}

      {activeCategory === "STT" && (
        <>
          {/* Coming Soon STT Cloud Providers */}
          <div className="flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[44px] select-none">
            <div className="flex items-center gap-2.5 min-w-0">
              <ProviderLogo providerId="openai" size={16} className="shrink-0 opacity-60" />
              <div className="flex flex-col min-w-0">
                <span className="text-[13px] font-medium text-[rgb(var(--foreground))] truncate">
                  OpenAI Whisper Cloud
                </span>
                <span className="text-[10.5px] text-[rgb(var(--foreground-muted))] truncate">
                  Hosted speech transcription endpoint
                </span>
              </div>
            </div>
            <span className="text-[9.5px] font-mono px-1.5 py-0.5 rounded font-bold bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] shrink-0">
              Coming Soon
            </span>
          </div>
        </>
      )}
    </div>
  );

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] p-3 sm:p-3.5 gap-2.5 justify-between animate-fade-in",
        layoutMode === "small" ? "h-auto min-h-0 justify-start" : "h-full"
      )}
    >
      {/* ─── 1. STANDALONE PERSISTENT PROVIDERS HEADER (ZERO LAYOUT SHIFT) ─── */}
      <ProvidersHeader
        icon={subView === "server" ? <Server size={14} className="text-[rgb(var(--accent))]" /> : categoryIcon}
        title={
          subView === "server"
            ? activeCategory === "TTS"
              ? copy.tts.server.title
              : PROVIDERS_HEADER_COPY.customConfigTitle
            : categoryTitle
        }
        onBack={subView === "server" ? handleCancelCustomServer : onBack}
        isCustomDisabled={activeCategory === "STT"}
        customTooltip={
          activeCategory === "STT"
            ? PROVIDERS_HEADER_COPY.comingSoon
            : PROVIDERS_HEADER_COPY.customAria
        }
        isCustomActive={
          subView === "server" ||
          (activeCategory === "LLM"
            ? draftLlm?.active === "server"
            : activeCategory === "TTS"
            ? draftSettings?.tts?.active === "chatterbox_remote"
            : false)
        }
        onCustomClick={handleOpenCustomServer}
        inlineCustomActions={
          subView === "server"
            ? {
                onSave: handleSaveCustomServer,
                onCancel: handleCancelCustomServer,
                isSaving: checkingHealth,
              }
            : null
        }
        status={
          subView === "server" ? (
            checkingHealth ? (
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] animate-pulse">
                Testing...
              </span>
            ) : isHealthy === true ? (
              <span className="text-[10px] font-mono text-[rgb(var(--accent))]">
                Online {detectedDialect ? `(${detectedDialect})` : ""}
              </span>
            ) : null
          ) : null
        }
        filter={
          subView === "server"
            ? undefined
            : {
                label: copy.llm.cloud.filterAria,
                open: isFilterOpen,
                activeCount: providerFilter === "all" ? 0 : 1,
                onToggle: () => setIsFilterOpen((v) => !v),
                onClose: () => setIsFilterOpen(false),
                menu: (
                  <CatalogFilterMenu
                    title={copy.llm.cloud.filtersTitle}
                    resetLabel={
                      providerFilter === "all" ? undefined : copy.llm.cloud.resetFilters
                    }
                    onReset={resetProviderFilter}
                  >
                    {(
                      [
                        {
                          id: "all",
                          label: copy.llm.cloud.filterAll,
                          count: providerFilterCounts.all,
                          Icon: Globe,
                        },
                        {
                          id: "local",
                          label: "On-Device",
                          count: 1,
                          Icon: HardDrive,
                        },
                        {
                          id: "configured",
                          label: copy.llm.cloud.filterConfigured,
                          count: providerFilterCounts.configured,
                          Icon: Key,
                        },
                        {
                          id: "popular",
                          label: copy.llm.cloud.filterPopular,
                          count: providerFilterCounts.popular,
                          Icon: Sparkles,
                        },
                        {
                          id: "unconfigured",
                          label: copy.llm.cloud.filterUnconfigured,
                          count: providerFilterCounts.unconfigured,
                          Icon: KeyRound,
                        },
                      ] as const
                    ).map((item) => (
                      <CatalogFilterItem
                        key={item.id}
                        icon={item.Icon}
                        label={item.label}
                        count={item.count}
                        active={providerFilter === item.id}
                        onSelect={() => {
                          setProviderFilter(item.id);
                          setIsFilterOpen(false);
                        }}
                      />
                    ))}
                  </CatalogFilterMenu>
                ),
              }
        }
        sort={
          subView === "server"
            ? { order: sortOrder, onToggle: () => {} }
            : {
                order: sortOrder,
                onToggle: () => setSortOrder((prev) => (prev === "az" ? "za" : "az")),
              }
        }
        search={
          subView === "server"
            ? {
                open: false,
                value: "",
                onChange: () => {},
                onToggle: () => {},
                onClose: () => {},
              }
            : {
                open: isSearchOpen,
                value: searchQuery,
                placeholder: copy.llm.cloud.searchPlaceholder,
                onChange: setSearchQuery,
                onToggle: () => (isSearchOpen ? closeSearch() : openSearch()),
                onClose: closeSearch,
              }
        }
      />

      {/* ─── 2. BODY CONTENT: SERVER FORM OR COMPACT CATALOG ─── */}
      {subView === "server" ? (
        <div className="flex flex-col gap-3 flex-1 min-h-0 justify-between animate-fade-in pt-1">
          {activeCategory === "LLM" && (
            <div
              className={cn(
                "grid gap-3 items-end flex-1 pb-1",
                layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
              )}
            >
              <UnderlineInput
                label={copy.llm.server.urlLabel}
                value={customServerUrl}
                onChange={(e) => setCustomServerUrl(e.target.value)}
                placeholder={copy.llm.server.urlPlaceholder}
              />
              <ApiKeyField
                label={copy.llm.server.apiKeyLabel}
                value={customServerKey}
                onChange={setCustomServerKey}
                placeholder={copy.llm.server.apiKeyPlaceholder}
              />
              <div className="col-span-1 sm:col-span-2">
                <UnderlineInput
                  label="Server Name (optional)"
                  value={customServerName}
                  onChange={(e) => setCustomServerName(e.target.value)}
                  placeholder="Auto-detect (e.g. Ollama, vLLM, LM Studio)"
                />
              </div>
            </div>
          )}

          {activeCategory === "TTS" && (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-1 flex-1 items-end">
              <UnderlineInput
                label={copy.tts.server.urlLabel}
                value={customTtsEndpoint}
                onChange={(e) => setCustomTtsEndpoint(e.target.value)}
                placeholder={copy.tts.server.urlPlaceholder}
              />
              <UnderlineInput
                label={copy.tts.server.pathLabel}
                value={customTtsPath}
                onChange={(e) => setCustomTtsPath(e.target.value)}
                placeholder={copy.tts.server.pathPlaceholder}
              />
            </div>
          )}

          {showHealthErrorConfirm && modelsError && (
            <div className="p-2.5 rounded-lg border border-[rgba(var(--error),0.3)] bg-[rgba(var(--error),0.06)] flex flex-col gap-2 shrink-0 animate-fade-in">
              <div className="flex items-center gap-1.5 text-[11px] text-[rgb(var(--error))]">
                <AlertCircle size={13} className="shrink-0" />
                <span>Could not reach server: {modelsError}</span>
              </div>
              <div className="flex items-center justify-end gap-2">
                <button
                  type="button"
                  onClick={handleCancelCustomServer}
                  className="px-2 py-0.5 rounded text-[10.5px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  Discard
                </button>
                <button
                  type="button"
                  onClick={handleConfirmSaveAnyway}
                  className="px-2.5 py-0.5 rounded text-[10.5px] font-bold bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] border border-[rgba(var(--accent),0.3)] transition-all cursor-pointer"
                >
                  Save Anyway
                </button>
              </div>
            </div>
          )}

          {modelsError && !showHealthErrorConfirm && (
            <span className="text-[11px] text-[rgb(var(--error))]/80 flex items-center gap-1 ml-0.5 shrink-0">
              <AlertCircle size={13} /> {modelsError}
            </span>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-2.5 flex-1 min-h-0 animate-fade-in">
          {/* Provider List & Expandable Modal */}
          <ExpandableList
            className="flex-1 min-h-0"
            domainId="models"
            triggerPlacement="row"
            customTrigger={
              activeCategory === "STT"
                ? () => null
                : (open) => (
                    <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] mt-auto shrink-0 flex items-center justify-center">
                      <button
                        type="button"
                        onClick={open}
                        className="w-full flex items-center justify-center gap-1.5 py-1 text-[11px] font-medium text-[rgb(var(--foreground-muted))]/75 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer group select-none"
                      >
                        <span>Show all {activeCategory} providers</span>
                        <ChevronDown size={13} className="group-hover:translate-y-0.5 transition-transform" />
                      </button>
                    </div>
                  )
            }
            inlineMaxHeightClass="max-h-[140px] sm:max-h-[160px]"
            icon={
              modalView === "models" ? (
                <Network size={16} className="text-[rgb(var(--accent))]" />
              ) : activeCategory === "STT" ? (
                <Mic size={16} className="text-[rgb(var(--accent))]" />
              ) : activeCategory === "TTS" ? (
                <Volume2 size={16} className="text-[rgb(var(--accent))]" />
              ) : (
                <Cloud size={16} className="text-[rgb(var(--accent))]" />
              )
            }
            onBack={modalView === "models" ? () => setModalView("providers") : undefined}
            title={
              <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
                {modalView === "models" ? "Available Models" : `${categoryTitle}`}
              </span>
            }
            subtitle={
              modalView === "models" ? (
                <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
                  {remoteModels.length} models
                </span>
              ) : (
                <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
                  {activeCategory} Providers · {activeCategory === "LLM" ? filteredProviders.length + 1 : 2}
                </span>
              )
            }
            expandLabel={copy.llm.cloud.expandLabel}
            ariaLabel={copy.llm.cloud.expandAriaLabel}
            modalSubtitle={
              modalView === "models"
                ? "Explore, probe, and select remote models for conversation."
                : `Configure ${activeCategory} inference providers and authentication keys.`
            }
            headerActions={<SettingsCommitControls domainId="models" />}
            modalContent={
              activeCategory === "STT" ? undefined : modalView === "models" ? (
                <RemoteModelsModalView
                  remoteModels={remoteModels}
                  selectedModelId={draftLlm?.cloud?.model}
                  pinnedModelId={committedSettings?.llm?.cloud?.model}
                  probingMap={probingMap}
                  capabilitiesCache={useSettingsStore.getState().capabilitiesCache}
                  onSelectModel={(modelId) => {
                    const cur = draftLlm?.cloud;
                    if (cur) {
                      updateDraft("llm", "cloud", {
                        ...cur,
                        model: modelId,
                      });
                    }
                  }}
                  onProbeCapabilities={handleProbeCapabilities}
                />
              ) : (
                <CloudProvidersModalView
                  category={activeCategory}
                  isLocalActive={isLocalActive}
                  onSelectLocal={handleSelectLocal}
                  activeCloudProviderId={activeCloudProviderId}
                  getProviderKey={getProviderKey}
                  pinnedProviderId={committedActiveCloudProviderId}
                  getCommittedProviderKey={getCommittedProviderKey}
                  onSelectProvider={handleSelectCloudProvider}
                  editingProviderId={editingProviderId}
                  editingKeyValue={editingKeyValue}
                  setEditingKeyValue={setEditingKeyValue}
                  onStartInlineEdit={handleStartInlineEdit}
                  onCancelInlineKey={handleCancelInlineKey}
                  onSaveInlineKey={handleSaveInlineKey}
                  onNavigateToModels={() => setModalView("models")}
                  onAddCustomServer={handleOpenCustomServer}
                />
              )
            }
          >
            {ProviderRows()}
          </ExpandableList>

          {modelsError && !showHealthErrorConfirm && (
            <span className="text-[10px] text-[rgb(var(--accent))] flex items-center gap-1 ml-0.5 shrink-0 font-mono">
              <AlertCircle size={11} /> {modelsError}
            </span>
          )}
        </div>
      )}
    </div>
  );
});

LlmConfigDesk.displayName = "LlmConfigDesk";
