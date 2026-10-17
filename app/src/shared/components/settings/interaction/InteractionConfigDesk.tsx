import { useState, useEffect, memo, useMemo } from "react";
import { useSettingsStore, type LlmProviderConfig, type ActiveCategory, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { CLOUD_PROVIDERS, TTS_MODAL_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY, PIPELINE_PROVIDERS_DESK_COPY } from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/catalogService";
import { BrainCircuit, Network, AudioLines, Mic, Globe, Key, KeyRound, Check, X, Pencil, Trash2, HardDrive, ChevronDown, ChevronUp, Plus } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { activeSelection } from "@/shared/lib/activeSelection";
import { ExpandableList } from "@/shared/ui";
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
import { ProviderLogo } from "@/shared/components/settings/providers/ProviderLogo";
import { CustomServerFormCard } from "@/shared/components/settings/providers/CustomServerFormCard";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";

export interface InteractionConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill?: ActiveCategory;
  isModular: boolean;
  onBack?: () => void;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const InteractionConfigDesk = memo(({ activeCategory, onBack, layoutMode }: InteractionConfigDeskProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const draftLlm = draftSettings?.llm;
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [editingServer, setEditingServer] = useState<LlmCustomServer | TtsCustomServer | null>(null);
  const [isCreatingServer, setIsCreatingServer] = useState(false);
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
    category: activeCategory,
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
    activeCategory === "LLM"
  );

  const rawCloudProviders = useMemo((): CloudProvider[] => {
    if (activeCategory === "TTS") {
      return TTS_MODAL_PROVIDERS.map((p) => ({
        id: p.id,
        name: p.name,
        tagline: p.tagline,
        url: p.url || "",
        keyPlaceholder: "API Key (Optional)...",
        logo: p.logo,
      }));
    }
    return activeCategory === "LLM" ? CLOUD_PROVIDERS : [];
  }, [activeCategory]);

  const providerFilterCounts = useMemo(() => {
    let configured = 0, unconfigured = 0;
    for (const p of rawCloudProviders) {
      if (getProviderKey(p.id)?.trim()) configured += 1;
      else unconfigured += 1;
    }
    return { all: rawCloudProviders.length + 1, configured, unconfigured };
  }, [rawCloudProviders, getProviderKey]);

  const filteredProviders = useMemo(() => {
    let list = rawCloudProviders;
    const q = searchQuery.trim().toLowerCase();
    if (q) list = list.filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q) || p.tagline?.toLowerCase().includes(q));
    if (providerFilter === "configured") list = list.filter((p) => Boolean(getProviderKey(p.id)?.trim()));
    else if (providerFilter === "unconfigured") list = list.filter((p) => !getProviderKey(p.id)?.trim());
    else if (providerFilter === "local") list = [];

    return [...list].sort((a, b) => {
      const aKey = Boolean(getCommittedProviderKey(a.id)?.trim()), bKey = Boolean(getCommittedProviderKey(b.id)?.trim());
      if (aKey !== bKey) return aKey ? -1 : 1;
      return sortOrder === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
    });
  }, [rawCloudProviders, searchQuery, sortOrder, providerFilter, getProviderKey, getCommittedProviderKey]);

  const customServers = useMemo((): Array<LlmCustomServer | TtsCustomServer> => {
    if (activeCategory === "LLM") return llmServerInventory;
    if (activeCategory === "TTS") return draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
    return [];
  }, [activeCategory, llmServerInventory, draftSettings?.tts?.servers, draftSettings?.providers?.tts?.servers]);

  const curatedProviders = useMemo(() => {
    if (showAllInline) return filteredProviders;
    return filteredProviders.filter((p) => Boolean(getProviderKey(p.id)?.trim()) || activeCloudProviderId === p.id);
  }, [filteredProviders, showAllInline, getProviderKey, activeCloudProviderId]);

  const { containerRef: providerGridRef, lenisRef: providerLenisRef } = useLenisScrollContainer<HTMLDivElement>();
  const providerOrderFingerprint = useMemo(
    () => `${committedActiveCloudProviderId}|${rawCloudProviders.map((p) => (getCommittedProviderKey(p.id)?.trim() ? "1" : "0")).join("")}`,
    [committedActiveCloudProviderId, rawCloudProviders, getCommittedProviderKey]
  );
  useScrollTopOnChange(providerGridRef, providerOrderFingerprint, providerLenisRef);

  useEffect(() => {
    if (currentProvider.kind === "embedded" || !currentProvider.base_url || (currentProvider.kind === "cloud" && !currentProvider.api_key?.trim())) {
      setIsHealthy(null); setDetectedDialect(null); setModelsError(null); return;
    }
    let isMounted = true;
    const timer = setTimeout(async () => {
      setCheckingHealth(true); setModelsError(null);
      try {
        const res = await checkLlmProviderHealth(currentProvider);
        if (!isMounted) return;
        setIsHealthy(res.healthy); setDetectedDialect(res.dialect ?? null);
        setModelsError(res.healthy ? null : (res.reason ?? INTERACTION_CONFIG_DESK_COPY.status.connectionFailed));
      } catch (err) {
        if (!isMounted) return;
        console.error(err); setIsHealthy(false); setDetectedDialect(null);
        setModelsError(INTERACTION_CONFIG_DESK_COPY.status.connectionFailed);
      } finally {
        if (isMounted) setCheckingHealth(false);
      }
    }, 500);
    return () => { isMounted = false; clearTimeout(timer); };
  }, [currentProvider]);

  useEffect(() => {
    const store = useSettingsStore.getState();
    store.setDeskFooter(modelsError ? { domain: "interaction", title: INTERACTION_CONFIG_DESK_COPY.status.connectionFailed, detail: modelsError } : null);
    return () => useSettingsStore.getState().setDeskFooter(null);
  }, [modelsError]);

  if (!draftSettings) return null;

  const copy = INTERACTION_CONFIG_DESK_COPY;
  const categoryTitle = PIPELINE_PROVIDERS_DESK_COPY.cards[activeCategory.toLowerCase() as "stt" | "llm" | "tts"].title;
  const categoryIcon = activeCategory === "STT" ? <Mic size={14} className="text-[rgb(var(--accent))]" /> : activeCategory === "TTS" ? <AudioLines size={14} className="text-[rgb(var(--accent))]" /> : <BrainCircuit size={14} className="text-[rgb(var(--accent))]" />;
  const localMeta = {
    STT: { name: "On-Device", tagline: "100% offline speech recognition." },
    LLM: { name: "On-Device", tagline: "100% offline neural model." },
    TTS: { name: "On-Device", tagline: "100% offline neural voices." },
  }[activeCategory];

  const showLocalCard = (providerFilter === "all" || providerFilter === "local" || (providerFilter === "configured" && isLocalActive)) && (!searchQuery.trim() || "on-device local offline vox".includes(searchQuery.toLowerCase().trim()));
  const showCustomCards = providerFilter === "all" || providerFilter === "configured";



  return (
    <div className={cn("w-full flex flex-col flex-1 min-h-0 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] p-3 sm:p-3.5 gap-2.5 justify-between animate-fade-in", layoutMode === "small" ? "h-auto min-h-0 justify-start" : "h-full")}>
      <CatalogHeader
        icon={categoryIcon}
        title={categoryTitle}
        onBack={onBack}
        status={checkingHealth ? <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] animate-pulse">Testing...</span> : isHealthy === true ? <span className="text-[10px] font-mono text-[rgb(var(--accent))]">Online {detectedDialect ? `(${detectedDialect})` : ""}</span> : null}
        action={activeCategory !== "STT" && !isCreatingServer ? (
          <button type="button" onClick={() => { setIsCreatingServer(true); setEditingServer(null); }} className="px-2.5 py-1 rounded-lg text-[11.5px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1 cursor-pointer shrink-0">
            <Plus size={13} strokeWidth={2.5} /><span className="hidden sm:inline">Add Server</span>
          </button>
        ) : undefined}
        filter={{
          label: copy.llm.cloud.filterAria,
          open: isFilterOpen,
          activeCount: providerFilter === "all" ? 0 : 1,
          onToggle: () => setIsFilterOpen((v) => !v),
          onClose: () => setIsFilterOpen(false),
          menu: (
            <CatalogFilterMenu title={copy.llm.cloud.filtersTitle} resetLabel={providerFilter === "all" ? undefined : copy.llm.cloud.resetFilters} onReset={() => setProviderFilter("all")}>
              {[
                { id: "all" as const, label: copy.llm.cloud.filterAll, count: providerFilterCounts.all, Icon: Globe },
                { id: "local" as const, label: "On-Device", count: 1, Icon: HardDrive },
                { id: "configured" as const, label: copy.llm.cloud.filterConfigured, count: providerFilterCounts.configured, Icon: Key },
                { id: "unconfigured" as const, label: copy.llm.cloud.filterUnconfigured, count: providerFilterCounts.unconfigured, Icon: KeyRound },
              ].map((item) => (
                <CatalogFilterItem key={item.id} icon={item.Icon} label={item.label} count={item.count} active={providerFilter === item.id} onSelect={() => { setProviderFilter(item.id); setIsFilterOpen(false); }} />
              ))}
            </CatalogFilterMenu>
          ),
        }}
        sort={{ order: sortOrder, onToggle: () => setSortOrder((prev) => (prev === "az" ? "za" : "az")) }}
        search={{ open: isSearchOpen, value: searchQuery, placeholder: copy.llm.cloud.searchPlaceholder, onChange: setSearchQuery, onToggle: () => setIsSearchOpen((v) => !v), onClose: () => { setIsSearchOpen(false); setSearchQuery(""); } }}
      />

      <div className="flex flex-col gap-2.5 flex-1 min-h-0 animate-fade-in">
        <ExpandableList
          className="flex-1 min-h-0"
          domainId="models"
          triggerPlacement="floating"
          customTrigger={activeCategory === "STT" ? () => null : undefined}
          inlineMaxHeightClass="max-h-[160px]"
          icon={modalView === "models" ? <Network size={16} className="text-[rgb(var(--accent))]" /> : categoryIcon}
          onBack={modalView === "models" ? () => setModalView("providers") : undefined}
          title={<span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">{modalView === "models" ? "Available Models" : categoryTitle}</span>}
          subtitle={<span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">{modalView === "models" ? `${remoteModels.length} models` : `${activeCategory} Providers · ${activeCategory === "LLM" ? filteredProviders.length + 1 : 2}`}</span>}
          expandLabel={copy.llm.cloud.expandLabel}
          ariaLabel={copy.llm.cloud.expandAriaLabel}
          modalSubtitle={modalView === "models" ? "Explore, probe, and select remote models for conversation." : `Configure ${activeCategory} inference providers and authentication keys.`}
          headerActions={<SettingsCommitControls domainId="models" />}
          modalContent={activeCategory === "STT" ? undefined : modalView === "models" ? (
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
            <ProviderModal {...providerSelection} onAddCustomServer={() => setIsCreatingServer(true)} />
          )}
        >
          <div ref={providerGridRef} className="h-full overflow-y-auto pr-1 pb-6 custom-scrollbar overflow-x-hidden">
            <div className="flex flex-col gap-2 content-start w-full">
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
                    <ProviderLogo providerId="on-device" logo={null} size={14} className="shrink-0" />
                  </div>
                  <div className="flex flex-col min-w-0 flex-1 justify-center">
                    <span className="text-[11.5px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                      {localMeta.name}
                    </span>
                    <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                      {localMeta.tagline}
                    </span>
                  </div>
                  {isLocalActive && (
                    <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
                      <Check size={9} strokeWidth={2.5} />
                    </div>
                  )}
                </div>
              )}

              {showCustomCards && customServers.map((server) => {
                if (editingServer?.id === server.id) {
                  return <CustomServerFormCard key={server.id} category={activeCategory} initial={server} onClose={() => setEditingServer(null)} onServerSaved={() => setEditingServer(null)} />;
                }
                const isSelected = activeCategory === "LLM" ? draftLlm?.active?.category === "server" && draftLlm?.active?.provider === server.id : draftSettings?.tts?.active?.category === "server" && draftSettings?.tts?.active?.provider === server.id;
                return (
                  <div
                    key={server.id || server.url}
                    onClick={() => {
                      const domain = activeCategory === "LLM" ? "llm" : "tts";
                      updateDraft(domain, "active", activeSelection("server", (server as LlmCustomServer).model || "", server.id));
                      window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: domain }));
                    }}
                    className={cn(
                      "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
                      isSelected
                        ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                        : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
                      <ProviderLogo providerId={server.id || "custom-server"} logo={null} size={14} className="shrink-0" />
                    </div>
                    <div className="flex flex-col min-w-0 flex-1 justify-center">
                      <span className="text-[11.5px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                        {server.name || "Custom Server"}
                      </span>
                      <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                        {server.url}
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5 shrink-0 ml-auto">
                      {isSelected && (
                        <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))]">
                          <Check size={9} strokeWidth={2.5} />
                        </div>
                      )}
                      <button type="button" onClick={(e) => { e.stopPropagation(); setEditingServer(server); setIsCreatingServer(false); }} className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))]"><Pencil size={11} /></button>
                      <button type="button" onClick={(e) => { e.stopPropagation(); deleteCustomServer(activeCategory === "LLM" ? "llm" : "tts", server.id); }} className="p-1 rounded text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))]"><Trash2 size={11} /></button>
                    </div>
                  </div>
                );
              })}

              {showCustomCards && activeCategory !== "STT" && isCreatingServer && (
                <CustomServerFormCard key="new-server" category={activeCategory} initial={null} onClose={() => setIsCreatingServer(false)} onServerSaved={() => setIsCreatingServer(false)} />
              )}

              {showCustomCards && activeCategory !== "STT" && !isCreatingServer && !editingServer && (
                <div
                  onClick={() => setIsCreatingServer(true)}
                  className="group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px] bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                >
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
                    <ProviderLogo providerId="server" logo={null} size={14} className="shrink-0" />
                  </div>
                  <div className="flex flex-col min-w-0 flex-1 justify-center">
                    <span className="text-[11.5px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                      {activeCategory === "TTS" ? "Add Voice Server" : "Add Custom Server"}
                    </span>
                    <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                      {activeCategory === "TTS" ? "Connect remote synthesis host." : "Connect self-hosted Ollama, vLLM, or LM Studio endpoint."}
                    </span>
                  </div>
                  <div className="text-[rgb(var(--accent))] font-medium text-[11px] shrink-0 ml-auto">+ Configure</div>
                </div>
              )}

              {(showAllInline ? filteredProviders : curatedProviders).map((provider) => {
                const isSelected = activeCloudProviderId === provider.id;
                const isEditing = editingProviderId === provider.id;
                const savedKey = getProviderKey(provider.id) ?? "";
                const hasKey = Boolean(savedKey.trim());

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
                          <ProviderLogo providerId={provider.id} logo={provider.logo ?? null} size={14} className="shrink-0" />
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

              {activeCategory === "STT" && (
                <div className="flex items-center gap-2 px-2.5 py-2 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[46px] select-none">
                  <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0 opacity-60">
                    <ProviderLogo logo={null} size={14} className="shrink-0" />
                  </div>
                  <div className="flex flex-col min-w-0 flex-1 justify-center">
                    <span className="text-[11.5px] font-medium text-[rgb(var(--foreground))] truncate leading-tight">
                      Cloud STT
                    </span>
                    <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate mt-0.5 leading-tight">
                      Hosted transcription
                    </span>
                  </div>
                </div>
              )}

              {activeCategory === "LLM" && filteredProviders.length > curatedProviders.length && (
                <div className="pt-2 pb-1 flex flex-col items-center justify-center">
                  <button
                    type="button"
                    onClick={() => { setShowAllInline((v) => !v); setTimeout(() => providerLenisRef.current?.resize(), 50); }}
                    className="flex flex-col items-center justify-center gap-0.5 text-[rgb(var(--accent))] hover:opacity-80 transition-opacity cursor-pointer select-none group"
                  >
                    <span className="text-[11.5px] font-semibold tracking-tight">{showAllInline ? "Show less" : `Show all ${filteredProviders.length} providers`}</span>
                    {showAllInline ? <ChevronUp size={13} strokeWidth={2.5} className="group-hover:-translate-y-0.5 transition-transform" /> : <ChevronDown size={13} strokeWidth={2.5} className="group-hover:translate-y-0.5 transition-transform" />}
                  </button>
                </div>
              )}
            </div>
          </div>
        </ExpandableList>
      </div>
    </div>
  );
});

InteractionConfigDesk.displayName = "InteractionConfigDesk";
