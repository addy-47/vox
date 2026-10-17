import { useState, useEffect, memo, useMemo, useCallback } from "react";
import { useSettingsStore, type LlmProviderConfig, type ActiveCategory, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { CLOUD_PROVIDERS, TTS_MODAL_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY, PIPELINE_PROVIDERS_DESK_COPY, PROVIDERS_HEADER_COPY } from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/catalogService";
import { BrainCircuit, Network, AudioLines, Mic, Globe, Key, KeyRound, HardDrive, Plus, Server } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { activeSelection, getCloudProviderAlias } from "@/shared/lib/activeSelection";
import { ExpandableList } from "@/shared/ui";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { ModelModal } from "@/shared/components/settings/models/LlmCatalogView";
import {
  ProviderModal,
  deleteCustomServer,
} from "@/shared/components/settings/providers/ProviderModal";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import {
  CatalogFilterItem,
  CatalogFilterMenu,
  CatalogHeader,
  type CatalogSortOrder,
} from "@/shared/components/settings/providers/CatalogHeader";
import { upsertServer } from "@/shared/components/settings/providers/CustomServerFormCard";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { CustomServerInlineForm } from "./CustomServerInlineForm";
import { ProviderPillGrid } from "./ProviderPillGrid";

export interface InteractionConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill?: ActiveCategory;
  isModular: boolean;
  onBack?: () => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const InteractionConfigDesk = memo(({ activeCategory, onBack, layoutMode }: InteractionConfigDeskProps) => {
  const [currentCategory, setCurrentCategory] = useState<"STT" | "LLM" | "TTS">(activeCategory);

  useEffect(() => {
    setCurrentCategory(activeCategory);
  }, [activeCategory]);


  useEffect(() => {
    const handleSync = (e: Event) => {
      const cat = (e as CustomEvent).detail;
      if (cat === "stt" || cat === "llm" || cat === "tts") {
        setCurrentCategory(cat.toUpperCase() as "STT" | "LLM" | "TTS");
      }
    };
    window.addEventListener("sync_interaction_category", handleSync);
    window.addEventListener("sync_pipeline_tab", handleSync);
    return () => {
      window.removeEventListener("sync_interaction_category", handleSync);
      window.removeEventListener("sync_pipeline_tab", handleSync);
    };
  }, []);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const draftLlm = draftSettings?.llm;
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [subView, setSubView] = useState<"default" | "server">("default");
  const [customServerUrl, setCustomServerUrl] = useState("");
  const [customServerKey, setCustomServerKey] = useState("");
  const [customServerName, setCustomServerName] = useState("");
  const [customTtsEndpoint, setCustomTtsEndpoint] = useState("");
  const [customTtsPath, setCustomTtsPath] = useState("~/.vox");
  const [editingServerId, setEditingServerId] = useState<string | null>(null);
  const [showHealthErrorConfirm, setShowHealthErrorConfirm] = useState(false);

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [detectedDialect, setDetectedDialect] = useState<string | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [sortOrder, setSortOrder] = useState<CatalogSortOrder>("az");
  const [providerFilter, setProviderFilter] = useState<"all" | "local" | "cloud" | "configured" | "unconfigured">("all");
  const [showAllInline, setShowAllInline] = useState(false);
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [modalView, setModalView] = useState<"providers" | "models">("providers");

  const expandedOpenCount = useExpandedModalStore((s) => s.openCount);
  useEffect(() => {
    setProviderFilter(expandedOpenCount > 0 ? "configured" : "all");
  }, [expandedOpenCount]);

  const providerSelection = useProviderSelection({
    category: currentCategory,
    onNavigateToModels: () => setModalView("models"),
  });
  const {
    activeCloudProviderId,
    pinnedProviderId: committedActiveCloudProviderId,
    getProviderKey,
    getCommittedProviderKey,
    onSelectProvider: handleSelectCloudProvider,
    isLocalActive,
    onSelectLocal: handleSelectLocal,
    editingProviderId,
    editingKeyValue,
    setEditingKeyValue,
    onStartInlineEdit: handleStartInlineEdit,
    onCancelInlineKey: handleCancelInlineKey,
    onSaveInlineKey: handleSaveInlineKey,
  } = providerSelection;

  const committedActiveCategory =
    currentCategory === "TTS"
      ? committedSettings?.tts?.active?.category
      : currentCategory === "STT"
      ? committedSettings?.stt?.active?.category
      : committedSettings?.llm?.active?.category;

  const committedActiveProviderId =
    currentCategory === "TTS"
      ? committedSettings?.tts?.active?.provider
      : currentCategory === "STT"
      ? committedSettings?.stt?.active?.provider
      : committedSettings?.llm?.active?.provider;

  const activeLlmProvider = draftLlm?.active?.category ?? "embedded";
  const llmServerInventory = useMemo(
    () => draftLlm?.servers ?? draftSettings?.providers?.llm?.servers ?? [],
    [draftLlm?.servers, draftSettings?.providers?.llm?.servers]
  );

  const activeServerEntry = useMemo(() => {
    if (activeLlmProvider !== "server" || !draftLlm?.active?.provider) return null;
    return llmServerInventory.find((s) => s.id === draftLlm?.active?.provider) ?? null;
  }, [activeLlmProvider, draftLlm?.active?.provider, llmServerInventory]);

  const currentProvider: LlmProviderConfig = useMemo(() => {
    if (activeLlmProvider === "embedded") return { kind: "embedded" };
    if (activeLlmProvider === "server") {
      return {
        kind: "server",
        provider_id: draftLlm?.active?.provider ?? undefined,
        base_url: activeServerEntry?.url || "",
        model: draftLlm?.active?.model || "",
        api_key: activeServerEntry?.api_key || undefined,
        provider_name: activeServerEntry?.name || undefined,
      };
    }
    const cloudId = draftLlm?.active?.provider ?? undefined;
    const cloudEntry = CLOUD_PROVIDERS.find((p) => p.id === cloudId);
    return {
      kind: "cloud",
      provider_id: cloudId,
      base_url: cloudEntry?.url || "",
      model: draftLlm?.active?.model || "",
      api_key: (cloudId && draftLlm?.cloud_keys?.[cloudId]) || undefined,
      provider_name: cloudEntry?.name || undefined,
    };
  }, [activeLlmProvider, activeServerEntry, draftLlm?.active?.provider, draftLlm?.active?.model, draftLlm?.cloud_keys]);

  const { remoteModels, probingMap, handleProbeCapabilities } = useRemoteLlmProbing(
    currentProvider,
    "llm",
    currentCategory === "LLM"
  );

  const rawCloudProviders = useMemo((): CloudProvider[] => {
    if (currentCategory === "TTS") {
      return TTS_MODAL_PROVIDERS.map((p) => ({
        id: p.id,
        name: p.name,
        tagline: p.tagline,
        url: p.url || "",
        keyPlaceholder: "API Key (Optional)...",
        logo: p.logo,
        isFree: p.isFree,
      }));
    }
    return currentCategory === "LLM" ? CLOUD_PROVIDERS : [];
  }, [currentCategory]);

  const providerFilterCounts = useMemo(() => {
    let configured = 0, unconfigured = 0;
    for (const p of rawCloudProviders) {
      if (p.isFree || getProviderKey(p.id)?.trim()) configured += 1;
      else unconfigured += 1;
    }
    return { all: rawCloudProviders.length + 1, configured, unconfigured };
  }, [rawCloudProviders, getProviderKey]);

  const filteredProviders = useMemo(() => {
    let list = rawCloudProviders;
    const q = searchQuery.trim().toLowerCase();
    if (q) list = list.filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q) || p.tagline?.toLowerCase().includes(q));
    if (providerFilter === "configured") list = list.filter((p) => Boolean(p.isFree) || Boolean(getProviderKey(p.id)?.trim()));
    else if (providerFilter === "unconfigured") list = list.filter((p) => !p.isFree && !getProviderKey(p.id)?.trim());
    else if (providerFilter === "local") list = [];

    return [...list].sort((a, b) => {
      const aKey = Boolean(a.isFree) || Boolean(getCommittedProviderKey(a.id)?.trim());
      const bKey = Boolean(b.isFree) || Boolean(getCommittedProviderKey(b.id)?.trim());
      if (aKey !== bKey) return aKey ? -1 : 1;
      return sortOrder === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
    });
  }, [rawCloudProviders, searchQuery, sortOrder, providerFilter, getProviderKey, getCommittedProviderKey]);

  const customServers = useMemo((): Array<LlmCustomServer | TtsCustomServer> => {
    if (currentCategory === "LLM") return llmServerInventory;
    if (currentCategory === "TTS") return draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
    return [];
  }, [currentCategory, llmServerInventory, draftSettings?.tts?.servers, draftSettings?.providers?.tts?.servers]);

  const curatedProviders = useMemo(() => {
    const list = showAllInline
      ? filteredProviders
      : filteredProviders.filter((p) => Boolean(p.isFree) || Boolean(getProviderKey(p.id)?.trim()) || activeCloudProviderId === p.id);
    // Active-card primacy: the committed active cloud provider always renders first.
    if (committedActiveCloudProviderId) {
      const idx = list.findIndex(
        (p) =>
          p.id === committedActiveCloudProviderId ||
          getCloudProviderAlias(p.id) === committedActiveCloudProviderId
      );
      if (idx > 0) {
        const [pinned] = list.splice(idx, 1);
        list.unshift(pinned);
      }
    }
    console.log("[InteractionConfigDesk] curatedProviders:", {
      currentCategory,
      committedActiveCloudProviderId,
      activeCloudProviderId,
      firstProvider: list[0]?.id,
      listLength: list.length,
      allIds: list.map((p) => p.id),
    });
    return list;
  }, [filteredProviders, showAllInline, getProviderKey, activeCloudProviderId, committedActiveCloudProviderId, currentCategory]);

  const { containerRef: providerGridRef, lenisRef: providerLenisRef } = useLenisScrollContainer<HTMLDivElement>();
  const providerOrderFingerprint = useMemo(
    () => `${committedActiveCloudProviderId}|${rawCloudProviders.map((p) => (getCommittedProviderKey(p.id)?.trim() ? "1" : "0")).join("")}`,
    [committedActiveCloudProviderId, rawCloudProviders, getCommittedProviderKey]
  );
  useScrollTopOnChange(providerGridRef, providerOrderFingerprint, providerLenisRef);

  useEffect(() => {
    if (currentProvider.kind === "embedded") {
      setCheckingHealth(false);
      setIsHealthy(true);
      setDetectedDialect(null);
      setModelsError(null);
      return;
    }
    if (!currentProvider.base_url || (currentProvider.kind === "cloud" && !currentProvider.api_key?.trim())) {
      setCheckingHealth(false);
      setIsHealthy(null);
      setDetectedDialect(null);
      setModelsError(null);
      return;
    }
    let isMounted = true;
    const timer = setTimeout(async () => {
      setCheckingHealth(true);
      setModelsError(null);
      try {
        const res = await checkLlmProviderHealth(currentProvider);
        if (!isMounted) return;
        setIsHealthy(res.healthy);
        setDetectedDialect(res.dialect ?? null);
        setModelsError(res.healthy ? null : (res.reason ?? INTERACTION_CONFIG_DESK_COPY.status.connectionFailed));
      } catch (err) {
        if (!isMounted) return;
        console.error(err);
        setIsHealthy(false);
        setDetectedDialect(null);
        setModelsError(INTERACTION_CONFIG_DESK_COPY.status.connectionFailed);
      } finally {
        if (isMounted) setCheckingHealth(false);
      }
    }, 500);
    return () => {
      isMounted = false;
      clearTimeout(timer);
    };
  }, [currentProvider]);

  useEffect(() => {
    const store = useSettingsStore.getState();
    if (subView === "default") {
      store.setDeskFooter(modelsError ? { domain: "interaction", title: INTERACTION_CONFIG_DESK_COPY.status.connectionFailed, detail: modelsError } : null);
    } else {
      store.setDeskFooter(null);
    }
    return () => useSettingsStore.getState().setDeskFooter(null);
  }, [modelsError, subView]);

  const handleOpenCustomServer = useCallback(() => {
    setCustomServerUrl("");
    setCustomServerKey("");
    setCustomServerName("");
    setCustomTtsEndpoint("");
    setCustomTtsPath("~/.vox");
    setEditingServerId(null);
    setShowHealthErrorConfirm(false);
    setModelsError(null);
    setSubView("server");
  }, []);

  const handleEditServer = useCallback((id: string, url: string, apiKey: string | null, displayName: string) => {
    setCustomServerUrl(url);
    setCustomServerKey(apiKey ?? "");
    setCustomServerName(displayName);
    setCustomTtsEndpoint(url);
    setEditingServerId(id);
    setShowHealthErrorConfirm(false);
    setModelsError(null);
    setSubView("server");
  }, []);

  const handleCancelCustomServer = useCallback(() => {
    setShowHealthErrorConfirm(false);
    setModelsError(null);
    setEditingServerId(null);
    setSubView("default");
  }, []);

  const handleSaveCustomServer = useCallback(async () => {
    if (currentCategory === "LLM") {
      const urlToTest = customServerUrl.trim() || "";
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
          const newServer = {
            id: editingServerId ?? `llm_custom_${Date.now()}`,
            name: finalName,
            url: urlToTest,
            model: "",
            protocol: "openai" as const,
            api_key: customServerKey.trim() || null,
          };
          updateDraft("llm", "servers", upsertServer(llmServerInventory, newServer));
          await useSettingsStore.getState().commitChanges();
          setEditingServerId(null);
          setIsHealthy(true);
          setDetectedDialect(res.dialect ?? null);
          setSubView("default");
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
    } else if (currentCategory === "TTS") {
      const endpoint = customTtsEndpoint.trim() || "";
      const remotePath = customTtsPath.trim() || "~/.vox";
      const existing = editingServerId ? customServers.find((s) => s.id === editingServerId) : undefined;
      const newServer = {
        id: editingServerId ?? `tts_custom_${Date.now()}`,
        name: existing?.name ?? "Voice Server",
        url: endpoint,
        remote_path: remotePath,
        api_key: existing?.api_key ?? null,
      };
      const ttsServers = (draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? []) as TtsCustomServer[];
      updateDraft("tts", "servers", upsertServer(ttsServers, newServer));
      await useSettingsStore.getState().commitChanges();
      setEditingServerId(null);
      setSubView("default");
    }
  }, [
    currentCategory,
    customServerUrl,
    customServerKey,
    customServerName,
    customTtsEndpoint,
    customTtsPath,
    editingServerId,
    llmServerInventory,
    customServers,
    draftSettings?.tts?.servers,
    draftSettings?.providers?.tts?.servers,
    updateDraft,
  ]);

  const handleConfirmSaveAnyway = useCallback(() => {
    if (currentCategory === "LLM") {
      const urlToSave = customServerUrl.trim() || "";
      const finalName = customServerName.trim() || "Custom Server";
      const newServer = {
        id: editingServerId ?? `llm_custom_${Date.now()}`,
        name: finalName,
        url: urlToSave,
        model: "",
        protocol: "openai" as const,
        api_key: customServerKey.trim() || null,
      };
      updateDraft("llm", "servers", upsertServer(llmServerInventory, newServer));
      void useSettingsStore.getState().commitChanges().then(() => setEditingServerId(null));
    }
    setShowHealthErrorConfirm(false);
    setSubView("default");
  }, [currentCategory, customServerUrl, customServerKey, customServerName, editingServerId, llmServerInventory, updateDraft]);

  if (!draftSettings) return null;

  const copy = INTERACTION_CONFIG_DESK_COPY;
  const categoryTitle = PIPELINE_PROVIDERS_DESK_COPY.cards[currentCategory.toLowerCase() as "stt" | "llm" | "tts"].title;
  const categoryIcon = currentCategory === "STT" ? <Mic size={14} className="text-[rgb(var(--accent))]" /> : currentCategory === "TTS" ? <AudioLines size={14} className="text-[rgb(var(--accent))]" /> : <BrainCircuit size={14} className="text-[rgb(var(--accent))]" />;
  const localMeta = {
    STT: { name: "On-Device", tagline: "100% offline speech recognition." },
    LLM: { name: "On-Device", tagline: "100% offline neural model." },
    TTS: { name: "On-Device", tagline: "100% offline neural voices." },
  }[currentCategory];

  const showLocalCard = (providerFilter === "all" || providerFilter === "local" || (providerFilter === "configured" && isLocalActive)) && (!searchQuery.trim() || "on-device local offline vox".includes(searchQuery.toLowerCase().trim()));
  const showCustomCards = providerFilter === "all" || providerFilter === "configured";

  return (
    <div className={cn("w-full flex flex-col flex-1 min-h-0 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] p-3 sm:p-3.5 gap-2.5 justify-between animate-fade-in", layoutMode === "small" ? "h-auto min-h-0 justify-start" : "h-full")}>
      <CatalogHeader
        icon={subView === "server" ? <Server size={14} className="text-[rgb(var(--accent))]" /> : categoryIcon}
        title={
          subView === "server"
            ? currentCategory === "TTS"
              ? copy.tts.server.title
              : PROVIDERS_HEADER_COPY.customConfigTitle
            : categoryTitle
        }
        onBack={subView === "server" ? handleCancelCustomServer : onBack}
        status={
          checkingHealth ? (
            <Tooltip label="Testing connection...">
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-pulse shrink-0 inline-block ml-0.5" />
            </Tooltip>
          ) : isHealthy === true ? (
            <Tooltip label={detectedDialect ? `Online (${detectedDialect})` : "Online"}>
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)] shrink-0 inline-block ml-0.5" />
            </Tooltip>
          ) : isHealthy === false ? (
            <Tooltip label={modelsError || "Connection failed"}>
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--foreground-muted))]/40 shrink-0 inline-block ml-0.5" />
            </Tooltip>
          ) : null
        }
        action={
          subView === "server" ? (
            <div className="flex items-center gap-1.5 sm:gap-2">
              <button
                type="button"
                onClick={handleCancelCustomServer}
                className="px-2 py-1 rounded-lg text-[11.5px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] transition-all cursor-pointer"
              >
                {copy.llm.server.discard}
              </button>
              <button
                type="button"
                onClick={handleSaveCustomServer}
                disabled={checkingHealth}
                className="px-2.5 py-1 rounded-lg text-[11.5px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1 cursor-pointer disabled:opacity-50"
              >
                {checkingHealth ? (
                  <span className="animate-pulse">Probing...</span>
                ) : (
                  <span>{copy.llm.server.saveServer}</span>
                )}
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 sm:gap-2">
              {currentCategory === "STT" ? (
                <button
                  type="button"
                  disabled
                  title={PROVIDERS_HEADER_COPY.comingSoon}
                  className="px-2.5 py-1 rounded-lg text-[11.5px] font-bold bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]/40 flex items-center gap-1 cursor-not-allowed shrink-0"
                >
                  <Plus size={13} strokeWidth={2.5} /><span className="hidden sm:inline">Add Server</span>
                </button>
              ) : (
                <button
                  type="button"
                  onClick={handleOpenCustomServer}
                  className="px-2.5 py-1 rounded-lg text-[11.5px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1 cursor-pointer shrink-0"
                >
                  <Plus size={13} strokeWidth={2.5} /><span className="hidden sm:inline">Add Server</span>
                </button>
              )}
            </div>
          )
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
                    resetLabel={providerFilter === "all" ? undefined : copy.llm.cloud.resetFilters}
                    onReset={() => setProviderFilter("all")}
                  >
                    {[
                      { id: "all" as const, label: copy.llm.cloud.filterAll, count: providerFilterCounts.all, Icon: Globe },
                      { id: "local" as const, label: "On-Device", count: 1, Icon: HardDrive },
                      { id: "configured" as const, label: copy.llm.cloud.filterConfigured, count: providerFilterCounts.configured, Icon: Key },
                      { id: "unconfigured" as const, label: copy.llm.cloud.filterUnconfigured, count: providerFilterCounts.unconfigured, Icon: KeyRound },
                    ].map((item) => (
                      <CatalogFilterItem
                        key={item.id}
                        icon={item.Icon}
                        label={item.label}
                        count={item.count}
                        active={providerFilter === item.id}
                        onSelect={() => { setProviderFilter(item.id); setIsFilterOpen(false); }}
                      />
                    ))}
                  </CatalogFilterMenu>
                ),
              }
        }
        sort={subView === "server" ? undefined : { order: sortOrder, onToggle: () => setSortOrder((prev) => (prev === "az" ? "za" : "az")) }}
        search={subView === "server" ? undefined : { open: isSearchOpen, value: searchQuery, placeholder: copy.llm.cloud.searchPlaceholder, onChange: setSearchQuery, onToggle: () => setIsSearchOpen((v) => !v), onClose: () => { setIsSearchOpen(false); setSearchQuery(""); } }}
      />

      {subView === "server" ? (
        <CustomServerInlineForm
          category={currentCategory}
          url={currentCategory === "TTS" ? customTtsEndpoint : customServerUrl}
          onChangeUrl={currentCategory === "TTS" ? setCustomTtsEndpoint : setCustomServerUrl}
          apiKey={customServerKey}
          onChangeApiKey={setCustomServerKey}
          name={customServerName}
          onChangeName={setCustomServerName}
          ttsPath={customTtsPath}
          onChangeTtsPath={setCustomTtsPath}
          layoutMode={layoutMode}
          modelsError={modelsError}
          showHealthErrorConfirm={showHealthErrorConfirm}
          onDiscard={handleCancelCustomServer}
          onSaveAnyway={handleConfirmSaveAnyway}
        />
      ) : (
        <div className="flex flex-col gap-2.5 flex-1 min-h-0 animate-fade-in">
          <ExpandableList
            className="flex-1 min-h-0"
            domainId="models"
            onDiscard={() => {
              useSettingsStore.getState().discardDomainChanges("models");
            }}
            triggerPlacement="floating"
            customTrigger={currentCategory === "STT" ? () => null : undefined}
            inlineMaxHeightClass="max-h-[160px]"
            icon={modalView === "models" ? <Network size={16} className="text-[rgb(var(--accent))]" /> : categoryIcon}
            onBack={modalView === "models" ? () => setModalView("providers") : undefined}
            title={<span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">{modalView === "models" ? "Available Models" : categoryTitle}</span>}
            subtitle={<span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">{modalView === "models" ? `${remoteModels.length} models` : `${currentCategory} Providers · ${currentCategory === "LLM" ? filteredProviders.length + 1 : 2}`}</span>}
            expandLabel={copy.llm.cloud.expandLabel}
            ariaLabel={copy.llm.cloud.expandAriaLabel}
            modalSubtitle={modalView === "models" ? "Explore, probe, and select remote models for conversation." : `Configure ${currentCategory} inference providers and authentication keys.`}
            headerActions={<SettingsCommitControls domainId="models" />}
            modalContent={currentCategory === "STT" ? undefined : modalView === "models" ? (
              <ModelModal
                remoteModels={remoteModels}
                selectedModelId={draftLlm?.active?.model}
                pinnedModelId={committedSettings?.llm?.active?.model}
                probingMap={probingMap}
                capabilitiesCache={useSettingsStore.getState().capabilitiesCache}
                onSelectModel={(modelId) => {
                  const cur = useSettingsStore.getState().draftSettings?.llm?.active;
                  if (cur && (cur.category === "server" || cur.category === "cloud")) updateDraft("llm", "active", { ...cur, model: modelId });
                }}
                onProbeCapabilities={handleProbeCapabilities}
              />
            ) : (
              <ProviderModal {...providerSelection} onAddCustomServer={handleOpenCustomServer} />
            )}
          >
            <ProviderPillGrid
              currentCategory={currentCategory}
              showLocalCard={showLocalCard}
              isLocalActive={isLocalActive}
              committedActiveCategory={committedActiveCategory}
              committedActiveProviderId={committedActiveProviderId}
              localMeta={localMeta}
              onSelectLocal={handleSelectLocal}
              showCustomCards={showCustomCards}
              customServers={customServers}
              activeCustomServerId={
                currentCategory === "LLM"
                  ? (draftLlm?.active?.category === "server" ? draftLlm?.active?.provider : null)
                  : (draftSettings?.tts?.active?.category === "server" ? draftSettings?.tts?.active?.provider : null)
              }
              onSelectCustomServer={(server) => {
                const domain = currentCategory === "LLM" ? "llm" : "tts";
                updateDraft(domain, "active", activeSelection("server", (server as LlmCustomServer).model || "", server.id));
                window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: domain }));
              }}
              onEditCustomServer={(server) => {
                handleEditServer(server.id, server.url, server.api_key ?? null, server.name || "");
              }}
              onDeleteCustomServer={(serverId) => {
                deleteCustomServer(currentCategory === "LLM" ? "llm" : "tts", serverId);
              }}
              providers={showAllInline ? filteredProviders : curatedProviders}
              activeCloudProviderId={activeCloudProviderId}
              editingProviderId={editingProviderId}
              editingKeyValue={editingKeyValue}
              setEditingKeyValue={setEditingKeyValue}
              getProviderKey={getProviderKey}
              onSelectCloudProvider={handleSelectCloudProvider}
              onStartInlineEdit={handleStartInlineEdit}
              onSaveInlineKey={handleSaveInlineKey}
              onCancelInlineKey={handleCancelInlineKey}
              showAllInline={showAllInline}
              onToggleShowAll={() => {
                setShowAllInline((v) => !v);
                setTimeout(() => providerLenisRef.current?.resize(), 50);
              }}
              totalProvidersCount={filteredProviders.length}
              containerRef={providerGridRef}
            />
        </ExpandableList>
      </div>
      )}
    </div>
  );
});

InteractionConfigDesk.displayName = "InteractionConfigDesk";
