import { useState, useEffect, memo, useCallback, useMemo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { CLOUD_PROVIDERS, CloudProvider } from "@/data/providersCopy";
import {
  INTERACTION_CONFIG_DESK_COPY,
  PROVIDERS_HEADER_COPY,
  PIPELINE_PROVIDERS_DESK_COPY,
} from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  BrainCircuit,
  Network,
  AudioLines,
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
  ChevronUp,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { activeModelId, activeSelection } from "@/shared/lib/activeSelection";
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
    "all" | "local" | "cloud" | "configured" | "unconfigured"
  >("all");
  const [showAllInline, setShowAllInline] = useState(false);
  const expandedOpenCount = useExpandedModalStore((s) => s.openCount);
  // Compact inline view defaults to "all" providers; the expanded modal view
  // defaults to the "available" (configured) filter.
  useEffect(() => {
    if (expandedOpenCount > 0) setProviderFilter("configured");
    else setProviderFilter("all");
  }, [expandedOpenCount]);
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState("");
  const [modalView, setModalView] = useState<"providers" | "models">("providers");

  const activeLlmProvider = draftLlm?.active?.category ?? "embedded";
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
    if (activeLlmProvider !== "cloud") return null;
    const pName = draftLlm?.cloud?.provider_name;
    if (!pName) return "custom";
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : "custom";
  }, [activeLlmProvider, draftLlm?.cloud?.provider_name]);

  const committedActiveCloudProviderId = useMemo(() => {
    if (committedSettings?.llm?.active?.category !== "cloud") return undefined;
    const pName = committedSettings?.llm?.cloud?.provider_name;
    if (!pName) return undefined;
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : undefined;
  }, [committedSettings?.llm?.active?.category, committedSettings?.llm?.cloud?.provider_name]);

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
    let unconfigured = 0;
    for (const p of CLOUD_PROVIDERS) {
      const key = getProviderKey(p.id);
      if (key && key.trim().length > 0) configured += 1;
      else unconfigured += 1;
    }
    return {
      all: CLOUD_PROVIDERS.length + 1,
      configured,
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

  const customLlmServers = useMemo(() => {
    return draftSettings?.providers?.llm?.servers || committedSettings?.providers?.llm?.servers || [];
  }, [draftSettings?.providers?.llm?.servers, committedSettings?.providers?.llm?.servers]);

  const customTtsServers = useMemo(() => {
    return draftSettings?.providers?.tts?.servers || committedSettings?.providers?.tts?.servers || [];
  }, [draftSettings?.providers?.tts?.servers, committedSettings?.providers?.tts?.servers]);

  const curatedLlmProviders = useMemo(() => {
    if (showAllInline) return filteredProviders;
    return filteredProviders.filter((p) => {
      const isConfigured = Boolean(getProviderKey(p.id)?.trim());
      const isActive = activeCloudProviderId === p.id;
      return isConfigured || isActive;
    });
  }, [filteredProviders, showAllInline, getProviderKey, activeCloudProviderId]);

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
      updateDraft(
        "llm",
        "active",
        activeSelection("cloud", draftLlm?.cloud?.model || "", provider.name)
      );
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
      return draftSettings?.stt?.active?.category === "embedded";
    }
    if (activeCategory === "LLM") {
      return draftSettings?.llm?.active?.category === "embedded";
    }
    if (activeCategory === "TTS") {
      const ttsCategory = draftSettings?.tts?.active?.category ?? "embedded";
      return ttsCategory !== "cloud" && ttsCategory !== "server";
    }
    return false;
  }, [
    activeCategory,
    draftSettings?.stt?.active?.category,
    draftSettings?.llm?.active?.category,
    draftSettings?.tts?.active?.category,
  ]);

  const handleSelectLocal = useCallback(() => {
    if (activeCategory === "STT") {
      updateDraft(
        "stt",
        "active",
        activeSelection(
          "embedded",
          draftSettings?.stt?.last_selected?.embedded ?? draftSettings?.stt?.active?.model ?? ""
        )
      );
    } else if (activeCategory === "LLM") {
      updateDraft(
        "llm",
        "active",
        activeSelection(
          "embedded",
          draftLlm?.last_selected?.embedded ?? draftLlm?.active?.model ?? ""
        )
      );
    } else if (activeCategory === "TTS") {
      updateDraft(
        "tts",
        "active",
        activeSelection(
          "embedded",
          draftSettings?.tts?.last_selected?.embedded ?? draftSettings?.tts?.active?.model ?? ""
        )
      );
    }
  }, [
    activeCategory,
    draftSettings?.stt?.last_selected?.embedded,
    draftSettings?.stt?.active?.model,
    draftLlm?.last_selected?.embedded,
    draftLlm?.active?.model,
    draftSettings?.tts?.last_selected?.embedded,
    draftSettings?.tts?.active?.model,
    updateDraft,
  ]);

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
          updateDraft(
            "llm",
            "active",
            activeSelection("server", draftLlm?.server?.model || "", finalName)
          );
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
      updateDraft("tts", "active", activeSelection("server", "chatterbox_remote"));
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
      updateDraft(
        "llm",
        "active",
        activeSelection("server", draftLlm?.server?.model || "", finalName)
      );
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
      <AudioLines size={14} className="text-[rgb(var(--accent))]" />
    ) : (
      <BrainCircuit size={14} className="text-[rgb(var(--accent))]" />
    );

  const localCardTitle = "On-Device";
  const localCardSubtitle = "100% offline";

  const matchesLocalSearch = useMemo(() => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase().trim();
    return "on-device local offline vox".includes(q);
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
      className="h-full overflow-y-auto pr-1 pb-6 custom-scrollbar overflow-x-hidden"
    >
      <div className="grid grid-cols-2 gap-2 content-start w-full">
      {/* 1. Custom created models / servers */}
      {activeCategory === "LLM" &&
        customLlmServers.map((server) => {
          const isSelected =
            draftLlm?.active?.category === "server" &&
            (draftLlm?.server?.provider_name === server.name || draftLlm?.server?.base_url === server.url);
          const isOllama =
            (server.name && server.name.toLowerCase().includes("ollama")) ||
            (server.id && server.id.toLowerCase().includes("ollama")) ||
            (server.url && (server.url.includes("11434") || server.url.toLowerCase().includes("ollama")));
          const serverLogoId = isOllama ? "ollama" : server.id || server.name || "server";

          return (
            <div
              key={server.id || server.url}
              onClick={() => {
                updateDraft("llm", "active", activeSelection("server", server.model || "", server.name));
                updateDraft("llm", "server", {
                  base_url: server.url,
                  provider_name: server.name,
                  api_key: server.api_key || null,
                  model: server.model || "",
                });
              }}
              className={cn(
                "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
                isSelected
                  ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
                <ProviderLogo providerId={serverLogoId} size={14} className="shrink-0" />
              </div>
              <div className="flex flex-col min-w-0 flex-1 justify-center">
                <span className={cn("text-[11.5px] truncate leading-tight", isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium text-[rgb(var(--foreground))]/90")}>
                  {server.name || "Custom Server"}
                </span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate font-mono mt-0.5 leading-tight">
                  {server.url}
                </span>
              </div>
              {isSelected && (
                <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
                  <Check size={9} strokeWidth={2.5} />
                </div>
              )}
            </div>
          );
        })}

      {activeCategory === "TTS" &&
        customTtsServers.map((server) => {
          const isSelected =
            draftSettings?.tts?.active?.category === "server" &&
            draftChatterboxRemote?.endpoint === server.url;
          return (
            <div
              key={server.id || server.url}
              onClick={() => {
                updateDraft("tts", "active", activeSelection("server", server.id || "chatterbox_remote", server.name));
                updateDraft("tts", "chatterbox_remote", {
                  endpoint: server.url,
                  remote_path: server.remote_path || "~/.vox",
                });
              }}
              className={cn(
                "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
                isSelected
                  ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
                <ProviderLogo providerId="server" size={14} className="shrink-0" />
              </div>
              <div className="flex flex-col min-w-0 flex-1 justify-center">
                <span className={cn("text-[11.5px] truncate leading-tight", isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium text-[rgb(var(--foreground))]/90")}>
                  {server.name || "Voice Server"}
                </span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate font-mono mt-0.5 leading-tight">
                  {server.url}
                </span>
              </div>
              {isSelected && (
                <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
                  <Check size={9} strokeWidth={2.5} />
                </div>
              )}
            </div>
          );
        })}

      {/* 2. On-Device Local Option */}
      {showLocalCard && (
        <div
          onClick={handleSelectLocal}
          className={cn(
            "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
            isLocalActive
              ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
              : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
          )}
        >
          <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
            <ProviderLogo providerId="embedded" size={14} className="shrink-0" />
          </div>
          <div className="flex flex-col min-w-0 flex-1 justify-center">
            <span
              className={cn(
                "text-[11.5px] truncate leading-tight",
                isLocalActive ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium text-[rgb(var(--foreground))]/90"
              )}
            >
              {localCardTitle}
            </span>
            <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
              {localCardSubtitle}
            </span>
          </div>
          {isLocalActive && (
            <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
              <Check size={9} strokeWidth={2.5} />
            </div>
          )}
        </div>
      )}

      {/* 3. Category-Specific Cloud Providers (Curated by default, all when showAllInline) */}
      {activeCategory === "LLM" &&
        (showAllInline ? filteredProviders : curatedLlmProviders).map((provider) => {
          const isSelected = activeCloudProviderId === provider.id && draftLlm?.active?.category === "cloud";
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
                "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
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
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
                    <ProviderLogo providerId={provider.id} size={14} className="shrink-0" />
                  </div>
                  <div className="flex flex-col min-w-0 flex-1 justify-center">
                    <span
                      className={cn(
                        "text-[11.5px] truncate leading-tight",
                        isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium text-[rgb(var(--foreground))]/90"
                      )}
                    >
                      {provider.name}
                    </span>
                    <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                      {provider.tagline || "Cloud API"}
                    </span>
                  </div>
                  <div className="shrink-0 flex items-center gap-1 ml-auto" onClick={(e) => e.stopPropagation()}>
                    {hasKey ? (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, savedKey)}
                        className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                        title={copy.llm.cloud.editKey}
                        aria-label={copy.llm.cloud.editKey}
                      >
                        <Pencil size={11} />
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, "")}
                        className="w-4 h-4 rounded flex items-center justify-center text-[13px] font-bold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-all leading-none cursor-pointer"
                        aria-label="Connect provider"
                      >
                        +
                      </button>
                    )}
                    {isSelected && (
                      <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                        <Check size={9} strokeWidth={2.5} />
                      </div>
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
            onClick={() => updateDraft("tts", "active", activeSelection("cloud", "edge_tts", "microsoft"))}
            className={cn(
              "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
              activeModelId(draftSettings?.tts?.active) === "edge_tts"
                ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
            )}
          >
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
              <ProviderLogo providerId="microsoft" size={14} className="shrink-0" />
            </div>
            <div className="flex flex-col min-w-0 flex-1 justify-center">
              <span className="text-[11.5px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                Edge TTS
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                400+ free cloud voices
              </span>
            </div>
            {activeModelId(draftSettings?.tts?.active) === "edge_tts" && (
              <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
                <Check size={9} strokeWidth={2.5} />
              </div>
            )}
          </div>

          {/* Coming Soon TTS Cloud Providers */}
          <div className="flex items-center gap-2 px-2.5 py-2 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[46px] select-none">
              <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0 opacity-60">
                <ProviderLogo providerId="gcp" size={14} className="shrink-0" />
              </div>
              <div className="flex flex-col min-w-0 flex-1 justify-center">
                <span className="text-[11.5px] font-medium text-[rgb(var(--foreground))] truncate leading-tight">
                  Google Cloud TTS
                </span>
                <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate mt-0.5 leading-tight">
                  Studio & Journey voices
                </span>
              </div>
            </div>

          <div className="flex items-center gap-2 px-2.5 py-2 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[46px] select-none">
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0 opacity-60">
              <ProviderLogo providerId="azure" size={14} className="shrink-0" />
            </div>
            <div className="flex flex-col min-w-0 flex-1 justify-center">
              <span className="text-[11.5px] font-medium text-[rgb(var(--foreground))] truncate leading-tight">
                Azure AI Speech
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate mt-0.5 leading-tight">
                Neural voices
              </span>
            </div>
          </div>
        </>
      )}

      {activeCategory === "STT" && (
        <>
          {/* Coming Soon STT Cloud Providers */}
          <div className="flex items-center gap-2 px-2.5 py-2 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[46px] select-none">
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0 opacity-60">
              <ProviderLogo providerId="openai" size={14} className="shrink-0" />
            </div>
            <div className="flex flex-col min-w-0 flex-1 justify-center">
              <span className="text-[11.5px] font-medium text-[rgb(var(--foreground))] truncate leading-tight">
                Whisper Cloud
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate mt-0.5 leading-tight">
                Hosted transcription
              </span>
            </div>
          </div>
        </>
      )}

      {/* 4. Bottom Inline Expand/Collapse Toggle */}
      {activeCategory === "LLM" && filteredProviders.length > curatedLlmProviders.length && (
        <div className="col-span-2 pt-2 pb-1 flex flex-col items-center justify-center">
          <button
            type="button"
            onClick={() => {
              setShowAllInline((v) => {
                const next = !v;
                setProviderFilter(next ? "all" : "configured");
                return next;
              });
              setTimeout(() => providerLenisRef.current?.resize(), 50);
            }}
            className="flex flex-col items-center justify-center gap-0.5 text-[rgb(var(--accent))] hover:opacity-80 transition-opacity cursor-pointer select-none group"
          >
            <span className="text-[11.5px] font-semibold tracking-tight">
              {showAllInline ? "Show less" : `Show all ${filteredProviders.length} providers`}
            </span>
            {showAllInline ? (
              <ChevronUp size={13} strokeWidth={2.5} className="group-hover:-translate-y-0.5 transition-transform" />
            ) : (
              <ChevronDown size={13} strokeWidth={2.5} className="group-hover:translate-y-0.5 transition-transform" />
            )}
          </button>
        </div>
      )}
      </div>
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
            ? draftLlm?.active?.category === "server"
            : activeCategory === "TTS"
            ? activeModelId(draftSettings?.tts?.active) === "chatterbox_remote"
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
            ? undefined
            : {
                order: sortOrder,
                onToggle: () => setSortOrder((prev) => (prev === "az" ? "za" : "az")),
              }
        }
        search={
          subView === "server"
            ? undefined
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
        <div className="flex flex-col gap-3 flex-1 min-h-0 overflow-y-auto custom-scrollbar justify-start animate-fade-in pt-1">
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
            triggerPlacement="floating"
            customTrigger={activeCategory === "STT" ? () => null : undefined}
            inlineMaxHeightClass="max-h-[160px] sm:max-h-[180px]"
            icon={
              modalView === "models" ? (
                <Network size={16} className="text-[rgb(var(--accent))]" />
              ) : activeCategory === "STT" ? (
                <Mic size={16} className="text-[rgb(var(--accent))]" />
              ) : activeCategory === "TTS" ? (
                <AudioLines size={16} className="text-[rgb(var(--accent))]" />
              ) : (
                <BrainCircuit size={16} className="text-[rgb(var(--accent))]" />
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
