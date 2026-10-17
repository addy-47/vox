import { memo, useState, useMemo, useCallback, useDeferredValue } from "react";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { CLOUD_PROVIDERS, TTS_MODAL_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { useSettingsStore, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { activeSelection, promoteActive } from "@/shared/lib/activeSelection";
import { ModalProviderFooter } from "@/shared/components/settings/SettingsCommitControls";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ArrowRight, Check, Cloud, Globe, HelpCircle, Key, KeyRound, Pencil, Plus, Server, Trash2, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ProviderGuideView } from "../models/catalog/ProviderGuideView";
import { CustomServerFormCard } from "./CustomServerFormCard";
import { SelectableCard } from "./SelectableCard";
import { CatalogFilterMenu, CatalogFilterItem } from "./CatalogHeader";
import { CatalogModalToolbar } from "./CatalogModalToolbar";
import { CatalogModalShell } from "./CatalogModalShell";
import { CatalogFacetRail, type CatalogFacetGroup } from "./CatalogFacetRail";

/** Deletes a custom server by ID and reverts to embedded if active. */
export async function deleteCustomServer(domain: "llm" | "tts", serverId: string): Promise<void> {
  const { commitChanges, draftSettings: draft, updateDraft } = useSettingsStore.getState();
  const list = (domain === "tts" ? draft?.tts?.servers ?? draft?.providers?.tts?.servers : draft?.llm?.servers ?? draft?.providers?.llm?.servers) ?? [];
  console.info(`[Providers] deleteCustomServer domain=${domain} id=${serverId} prevCount=${list.length}`);
  updateDraft(domain, "servers", list.filter((s: { id: string }) => s.id !== serverId));
  const isActive = (domain === "tts" ? draft?.tts?.active?.provider : draft?.llm?.active?.provider) === serverId;
  if (isActive) {
    const fallback = (domain === "tts" ? draft?.tts?.last_selected?.embedded : draft?.llm?.last_selected?.embedded) ?? "";
    updateDraft(domain, "active", activeSelection("embedded", fallback));
  }
  await commitChanges({ domain, key: "servers" });
  if (isActive) await commitChanges({ domain, key: "active" });
}

export interface ProviderModalProps {
  category?: "STT" | "LLM" | "TTS";
  isLocalActive?: boolean;
  onSelectLocal?: () => void;
  activeCloudProviderId: string | null;
  getProviderKey: (id: string) => string | undefined;
  pinnedProviderId: string | undefined;
  getCommittedProviderKey: (id: string) => string | undefined;
  onSelectProvider: (provider: CloudProvider) => void;
  editingProviderId: string | null;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  onStartInlineEdit: (id: string, currentKey: string) => void;
  onCancelInlineKey: () => void;
  onSaveInlineKey: (id: string) => void;
  onNavigateToModels?: (provider: CloudProvider) => void;
  onAddCustomServer?: () => void;
}

export type ProviderFilterCategory = "all" | "cloud" | "configured" | "unconfigured" | "custom" | "guide";

/** Dashed skeleton card for creating a custom server: centered plus-in-dashed-circle
 *  with centered title/description (restored from CloudProvidersModalView.old.tsx). */
const CustomProviderCreationCard = memo(function CustomProviderCreationCard({
  category,
  onClick,
}: {
  category: "STT" | "LLM" | "TTS";
  onClick: () => void;
}) {
  const isTts = category === "TTS";
  return (
    <div
      onClick={onClick}
      className="relative flex flex-col justify-between p-4.5 rounded-xl border border-dashed border-[rgba(var(--foreground),0.2)] hover:border-[rgb(var(--accent))] bg-transparent hover:bg-[rgba(var(--accent),0.03)] transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]"
    >
      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div className="w-12 h-12 rounded-2xl border border-dashed border-[rgba(var(--foreground),0.25)] group-hover:border-[rgb(var(--accent))] flex items-center justify-center text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] group-hover:scale-105 transition-all bg-[rgba(var(--foreground),0.02)]">
          <Plus size={20} strokeWidth={2.25} />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {isTts ? "Add Voice Server" : "Add Custom Server"}
        </h4>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
          {isTts
            ? "Connect Chatterbox GPU node or remote synthesis host."
            : "Connect self-hosted Ollama, vLLM, or LM Studio endpoint."}
        </p>
      </div>
      <div className="mt-auto pt-2.5 flex items-center justify-between text-[11px] font-mono border-t border-dashed border-[rgba(var(--foreground),0.12)] h-[28px]">
        <span className="text-[rgb(var(--foreground-muted))]/60">Self-Hosted</span>
        <span className="text-[rgb(var(--accent))] font-medium flex items-center gap-1 group-hover:translate-x-0.5 transition-transform">
          + Configure <ArrowRight size={11} />
        </span>
      </div>
    </div>
  );
});
CustomProviderCreationCard.displayName = "CustomProviderCreationCard";

export const ProviderModal = memo(({
  category = "LLM", onSelectLocal, activeCloudProviderId, getProviderKey, pinnedProviderId,
  getCommittedProviderKey, onSelectProvider, editingProviderId, editingKeyValue, setEditingKeyValue,
  onStartInlineEdit, onCancelInlineKey, onSaveInlineKey, onNavigateToModels,
}: ProviderModalProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [filterCategory, setFilterCategory] = useState<ProviderFilterCategory>("configured");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);
  const [isCreatingCustomServer, setIsCreatingCustomServer] = useState(false);
  const [editingServer, setEditingServer] = useState<LlmCustomServer | TtsCustomServer | null>(null);

  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const isTts = category === "TTS";

  const customServers = useMemo(() => {
    return (isTts ? draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers : draftSettings?.llm?.servers ?? draftSettings?.providers?.llm?.servers) ?? [];
  }, [isTts, draftSettings]);

  const activeCategoryType = useMemo(() => {
    const domain = category.toLowerCase() as "stt" | "tts" | "llm";
    return draftSettings?.[domain]?.active?.category ?? "embedded";
  }, [category, draftSettings]);

  const isLocalSelected = activeCategoryType === "embedded";
  const activeCloudId = useMemo(() => {
    if (activeCategoryType !== "cloud") return null;
    return activeCloudProviderId ?? (isTts ? draftSettings?.tts?.active?.provider : draftSettings?.llm?.active?.provider) ?? null;
  }, [activeCategoryType, activeCloudProviderId, isTts, draftSettings]);

  const activeServerId = activeCategoryType === "server" ? (isTts ? draftSettings?.tts?.active?.provider : draftSettings?.llm?.active?.provider) ?? null : null;

  const handleSelectServer = useCallback((server: LlmCustomServer | TtsCustomServer) => {
    useSettingsStore.getState().clearRemoteModelsError();
    if (isTts) {
      updateDraft("tts", "active", activeSelection("server", "", server.id));
      window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "tts" }));
    } else {
      const llmServ = server as LlmCustomServer;
      updateDraft("llm", "active", activeSelection("server", llmServ.model || "", llmServ.id));
      window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
    }
  }, [isTts, updateDraft]);

  const handleDeleteServer = useCallback(async (serverId: string) => {
    await deleteCustomServer(isTts ? "tts" : "llm", serverId);
  }, [isTts]);

  const handleSelectLocalProvider = useCallback(() => {
    useSettingsStore.getState().clearRemoteModelsError();
    if (onSelectLocal) {
      onSelectLocal();
    } else {
      const domain = category.toLowerCase() as "stt" | "llm" | "tts";
      const fallback = draftSettings?.[domain]?.last_selected?.embedded ?? "";
      updateDraft(domain, "active", activeSelection("embedded", fallback));
      window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: domain }));
    }
  }, [category, draftSettings, onSelectLocal, updateDraft]);

  const { containerRef: scrollContainerRef, lenisRef } = useLenisScrollContainer<HTMLDivElement>();
  const committedActiveProviderId = isTts
    ? committedSettings?.tts?.active?.provider
    : committedSettings?.llm?.active?.provider;
  const committedCategory = isTts
    ? committedSettings?.tts?.active?.category
    : committedSettings?.llm?.active?.category;
  const isCommittedLocal = committedCategory === "embedded";

  const orderFingerprint = useMemo(
    () => `${committedActiveProviderId ?? ""}|${isCommittedLocal ? "local" : ""}|${pinnedProviderId ?? ""}|${CLOUD_PROVIDERS.map((p) => getCommittedProviderKey(p.id)?.trim() ? "1" : "0").join("")}`,
    [committedActiveProviderId, isCommittedLocal, pinnedProviderId, getCommittedProviderKey]
  );
  useScrollTopOnChange(scrollContainerRef, orderFingerprint, lenisRef);

  const configuredCount = useMemo(() => CLOUD_PROVIDERS.filter((p) => Boolean(p.isConfigured) || Boolean(getProviderKey(p.id)?.trim())).length, [getProviderKey]);
  const unconfiguredCount = useMemo(() => CLOUD_PROVIDERS.filter((p) => !p.isConfigured && !getProviderKey(p.id)?.trim()).length, [getProviderKey]);
  const ttsAvailableCount = useMemo(() => TTS_MODAL_PROVIDERS.filter((p) => p.isConfigured || p.isFree || Boolean(getProviderKey(p.id)?.trim())).length, [getProviderKey]);

  const sourceFilters = useMemo(
    () => [
      { id: "all" as const, label: "All Providers", count: 1 + (isTts ? TTS_MODAL_PROVIDERS.length : CLOUD_PROVIDERS.length) + customServers.length, Icon: Globe },
      { id: "configured" as const, label: "Available", count: 1 + (isTts ? ttsAvailableCount : configuredCount) + customServers.length, Icon: Key },
      { id: "custom" as const, label: "Custom Servers", count: customServers.length, Icon: Server },
      { id: "cloud" as const, label: isTts ? "Cloud Voices" : "Cloud Providers", count: isTts ? TTS_MODAL_PROVIDERS.length : CLOUD_PROVIDERS.length, Icon: Cloud },
    ],
    [isTts, customServers.length, ttsAvailableCount, configuredCount]
  );

  const statusFilters = useMemo(
    () =>
      isTts
        ? []
        : [
            { id: "unconfigured" as const, label: "Needs Key", count: unconfiguredCount, Icon: KeyRound },
          ],
    [isTts, unconfiguredCount]
  );

  const providerFilters = useMemo(
    () => [...sourceFilters, ...statusFilters],
    [sourceFilters, statusFilters]
  );

  const facetGroups = useMemo<CatalogFacetGroup[]>(() => {
    const groups: CatalogFacetGroup[] = [
      {
        title: "Source",
        items: sourceFilters,
      },
    ];
    if (!isTts && statusFilters.length > 0) {
      groups.push({
        title: "Status",
        items: statusFilters,
      });
    }
    return groups;
  }, [sourceFilters, statusFilters, isTts]);

  const facetRailFooter = useMemo(
    () => (
      <button
        type="button"
        onClick={() => setFilterCategory((prev) => (prev === "guide" ? "all" : "guide"))}
        className={cn(
          "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]/50",
          filterCategory === "guide"
            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
        )}
      >
        <div className="flex items-center gap-2.5 truncate">
          <HelpCircle size={14} className={cn("shrink-0", filterCategory === "guide" ? "text-[rgb(var(--accent))]" : "opacity-60")} />
          <span className="truncate">Setup Guide</span>
        </div>
      </button>
    ),
    [filterCategory]
  );

  const ttsFilteredProviders = useMemo(() => {
    if (!isTts) return [];
    let list = TTS_MODAL_PROVIDERS;
    const q = deferredSearch.trim().toLowerCase();
    if (q) list = list.filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q) || p.tagline.toLowerCase().includes(q));
    if (filterCategory === "custom") return [];
    if (filterCategory === "configured") list = list.filter((p) => p.isConfigured || p.isFree || Boolean(getProviderKey(p.id)?.trim()));
    return promoteActive(
      [...list].sort((a, b) => sort === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" })),
      committedSettings?.tts?.active?.provider
    );
  }, [deferredSearch, filterCategory, getProviderKey, isTts, sort, committedSettings?.tts?.active?.provider]);

  const modalFilteredProviders = useMemo(() => {
    let list = CLOUD_PROVIDERS;
    const q = deferredSearch.trim().toLowerCase();
    if (q) list = list.filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q) || p.tagline?.toLowerCase().includes(q) || p.url.toLowerCase().includes(q));
    if (filterCategory === "configured") list = list.filter((p) => Boolean(p.isConfigured) || Boolean((p as { isFree?: boolean }).isFree) || Boolean(getProviderKey(p.id)?.trim()));
    else if (filterCategory === "unconfigured") list = list.filter((p) => !p.isConfigured && !getProviderKey(p.id)?.trim());
    else if (filterCategory === "custom") list = [];

    return promoteActive(
      [...list].sort((a, b) => {
        const aKey = Boolean(getCommittedProviderKey(a.id)?.trim()), bKey = Boolean(getCommittedProviderKey(b.id)?.trim());
        if (aKey !== bKey) return aKey ? -1 : 1;
        return sort === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
      }),
      committedSettings?.llm?.active?.provider
    );
  }, [deferredSearch, filterCategory, getProviderKey, getCommittedProviderKey, sort, committedSettings?.llm?.active?.provider]);

  const matchesLocal = useMemo(() => {
    const q = deferredSearch.trim().toLowerCase();
    return !q || "built-in on-device local neural whisper gguf kokoro vox offline".includes(q);
  }, [deferredSearch]);

  const showLocalCard = (filterCategory === "all" || filterCategory === "configured") && matchesLocal;
  const showCustomSection = filterCategory === "all" || filterCategory === "configured" || filterCategory === "custom";
  const showCustomCreationCard = showCustomSection && (!deferredSearch.trim() || "custom server self-hosted endpoint".includes(deferredSearch.trim().toLowerCase()));

  const filteredCustomServers = useMemo(() => {
    if (!showCustomSection) return [];
    const q = deferredSearch.trim().toLowerCase();
    return promoteActive(
      !q ? [...customServers] : customServers.filter((s) => (s.name && s.name.toLowerCase().includes(q)) || (s.url && s.url.toLowerCase().includes(q)) || (s.id && s.id.toLowerCase().includes(q))),
      isTts ? committedSettings?.tts?.active?.provider : committedSettings?.llm?.active?.provider
    );
  }, [showCustomSection, deferredSearch, customServers, isTts, committedSettings?.tts?.active?.provider, committedSettings?.llm?.active?.provider]);

  const localMeta = {
    STT: { name: "On-Device Speech Recognition", tagline: "High-accuracy local Whisper & Nemotron transcription. 100% offline." },
    LLM: { name: "On-Device Model", tagline: "Local GGUF neural reasoning on CPU or GPU. Zero telemetry, private." },
    TTS: { name: "On-Device Neural Voices", tagline: "Local Kokoro neural synthesis with peak normalization and zero latency." },
  }[category];

  const localCard = showLocalCard ? (
    <SelectableCard
      key="local-provider-card" id="local-provider" name={localMeta.name} tagline={localMeta.tagline} logo={null}
      isSelected={isLocalSelected} onSelect={handleSelectLocalProvider}
      actionSlot={<><span className="text-[rgb(var(--foreground-muted))]/70">Local Hardware</span><span className="text-[rgb(var(--accent))] font-bold flex items-center gap-1.5"><span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />{isLocalSelected ? "Active" : "Offline Ready"}</span></>}
    />
  ) : null;

  const cloudCards = isTts
    ? ttsFilteredProviders.map((entry) => {
        const isSelected = activeCloudId === entry.id || (entry.id === "edge_speech" && ((activeCloudId?.includes("edge") ?? false) || (draftSettings?.tts?.active?.model?.includes("edge") ?? false)));
        return (
          <SelectableCard
            key={entry.id} id={entry.id} name={entry.name} tagline={entry.tagline} logo={entry.logo}
            isSelected={isSelected} isComingSoon={entry.isComingSoon}
            onSelect={() => {
              if (entry.isComingSoon) return;
              useSettingsStore.getState().clearRemoteModelsError();
              const ttsModel = entry.id === "edge_speech" ? entry.id.replace("speech", "tts") : entry.id;
              updateDraft("tts", "active", activeSelection("cloud", ttsModel, entry.id));
              window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "tts" }));
              onSelectProvider({ id: entry.id, name: entry.name, url: entry.url || "", popular: true, keyPlaceholder: "API Key (Optional)..." });
            }}
            actionSlot={<><span className="text-[rgb(var(--foreground-muted))]/70">{entry.isFree ? "Keyless Cloud" : "Cloud Voice"}</span><span className={cn("font-bold flex items-center gap-1.5", entry.isComingSoon ? "text-[rgb(var(--foreground-muted))]/60 font-normal" : "text-[rgb(var(--accent))]")}>{!entry.isComingSoon && <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />}{entry.isComingSoon ? "In Development" : isSelected ? "Active" : "Ready"}</span></>}
          />
        );
      })
    : modalFilteredProviders.map((provider) => {
        const isSelected = activeCloudId === provider.id;
        const savedKey = getProviderKey(provider.id);
        const isEditing = editingProviderId === provider.id;
        const hasKey = Boolean(savedKey && savedKey.trim().length > 0);
        return (
          <SelectableCard
            key={provider.id} id={provider.id} name={provider.name} tagline={provider.tagline || "OpenAI-compatible inference gateway."}
            logo={provider.logo ?? null} isSelected={isSelected} onSelect={() => onSelectProvider(provider)}
            cornerSlot={
              onNavigateToModels && hasKey ? (
                <Tooltip label="Browse models" side="top" asChild>
                  <button
                    type="button"
                    onClick={() => { onSelectProvider(provider); onNavigateToModels(provider); }}
                    className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer"
                    aria-label="Browse models"
                  >
                    <ArrowRight size={13} strokeWidth={2} />
                  </button>
                </Tooltip>
              ) : undefined
            }
            actionSlot={
              isEditing ? (
                <div className="flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 w-full bg-transparent animate-fade-in">
                  <input type="password" autoFocus value={editingKeyValue} onChange={(e) => setEditingKeyValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") onSaveInlineKey(provider.id); if (e.key === "Escape") onCancelInlineKey(); }} placeholder={provider.keyPlaceholder} className="w-full bg-transparent text-[11px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30" />
                  <button type="button" onClick={() => onSaveInlineKey(provider.id)} title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey} aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey} className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0 bg-transparent cursor-pointer"><Check size={13} strokeWidth={2.2} /></button>
                  <button type="button" onClick={onCancelInlineKey} title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey} aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey} className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 bg-transparent cursor-pointer"><X size={13} /></button>
                </div>
              ) : (
                <div className="flex items-center justify-between text-[11px] font-mono w-full">
                  <span className="text-[rgb(var(--foreground-muted))]/70">Cloud Gateway</span>
                  {hasKey ? (
                    <div className="flex items-center gap-1.5">
                      <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" /><span className="text-[rgb(var(--foreground-muted))]">••••••••{savedKey?.slice(-4)}</span>
                      <button type="button" onClick={() => onStartInlineEdit(provider.id, savedKey ?? "")} className="p-1 rounded hover:bg-white/[0.08] hover:text-[rgb(var(--accent))] transition-colors text-[rgb(var(--foreground-muted))]/60 cursor-pointer shrink-0" title="Edit API Key"><Pencil size={11} /></button>
                    </div>
                  ) : (
                    <button type="button" onClick={() => onStartInlineEdit(provider.id, "")} className="text-[rgb(var(--accent))] hover:underline cursor-pointer font-medium flex items-center gap-1">+ Connect Key</button>
                  )}
                </div>
              )
            }
          />
        );
      });

  const customCards = filteredCustomServers.map((server) => {
    if (editingServer?.id === server.id) {
      return <CustomServerFormCard key={server.id} category={category} initial={server} onClose={() => setEditingServer(null)} onServerSaved={() => {}} />;
    }
    const isSelected = activeCategoryType === "server" && activeServerId === server.id;
    return (
      <SelectableCard
        key={server.id || server.url} id={server.id} name={server.name || "Custom Server"} tagline={server.url} logo={null}
        providerName={server.name}
        isSelected={isSelected} onSelect={() => handleSelectServer(server)}
        cornerSlot={
          onNavigateToModels && category === "LLM" ? (
            <Tooltip label="Browse models" side="top" asChild>
              <button
                type="button"
                onClick={() => {
                  handleSelectServer(server);
                  onNavigateToModels({ id: server.id, name: server.name || "Custom Server", url: server.url, popular: false, keyPlaceholder: "" });
                }}
                className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer"
                aria-label="Browse models"
              >
                <ArrowRight size={13} strokeWidth={2} />
              </button>
            </Tooltip>
          ) : undefined
        }
        actionSlot={
          <div className="flex items-center justify-between text-[11px] font-mono w-full">
            <span className="text-[rgb(var(--foreground-muted))]/70">Self-Hosted</span>
            <div className="flex items-center gap-1.5">
              <Tooltip label="Edit Server" side="top">
                <button type="button" onClick={() => { setEditingServer(server); setIsCreatingCustomServer(false); }} className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-all cursor-pointer" aria-label="Edit Server"><Pencil size={13} /></button>
              </Tooltip>
              <Tooltip label="Delete Server" side="top">
                <button type="button" onClick={() => handleDeleteServer(server.id)} className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))] hover:bg-[rgba(var(--error),0.1)] transition-all cursor-pointer" aria-label="Delete Server"><Trash2 size={13} /></button>
              </Tooltip>
            </div>
          </div>
        }
      />
    );
  });

  const creationCard = (
    <>
      {showCustomSection && isCreatingCustomServer && (
        <CustomServerFormCard key="new" category={category} initial={null} onClose={() => setIsCreatingCustomServer(false)} onServerSaved={() => {}} />
      )}
      {showCustomCreationCard && !isCreatingCustomServer && !editingServer && (
        <CustomProviderCreationCard
          category={category}
          onClick={() => setIsCreatingCustomServer(true)}
        />
      )}
    </>
  );

  const currentItemCount = (showLocalCard ? 1 : 0) + (isTts ? ttsFilteredProviders.length : modalFilteredProviders.length) + filteredCustomServers.length;
  const totalItemCount = (isTts ? TTS_MODAL_PROVIDERS.length : CLOUD_PROVIDERS.length) + customServers.length + 1;

  const hasCards =
    showLocalCard ||
    (isTts ? ttsFilteredProviders.length > 0 : modalFilteredProviders.length > 0) ||
    filteredCustomServers.length > 0 ||
    (showCustomSection && isCreatingCustomServer) ||
    (showCustomCreationCard && !editingServer);

  const toolbarNode = (
    <CatalogModalToolbar
      search={{
        value: modalSearch,
        placeholder: "Search providers...",
        onChange: setModalSearch,
      }}
      sort={{
        order: sort,
        onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
      }}
      itemCount={{
        current: currentItemCount,
        total: totalItemCount,
        entityName: "providers",
      }}
      action={
        <Tooltip label="Setup Guide" side="bottom" asChild>
          <button
            type="button"
            onClick={() => setFilterCategory((prev) => (prev === "guide" ? "all" : "guide"))}
            aria-label="Setup Guide"
            className={cn(
              "p-2 rounded-xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--foreground),0.04)] transition-colors cursor-pointer flex items-center justify-center shrink-0 lg:hidden",
              filterCategory === "guide"
                ? "border-[rgb(var(--accent))]/40 bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]"
            )}
          >
            <HelpCircle size={14} strokeWidth={1.75} />
          </button>
        </Tooltip>
      }
      filter={{
        label: "Filter providers",
        open: isFilterDropdownOpen,
        activeCount: filterCategory === "all" ? 0 : 1,
        onToggle: () => setIsFilterDropdownOpen((v) => !v),
        onClose: () => setIsFilterDropdownOpen(false),
        menu: (
          <CatalogFilterMenu
            title="Provider Filters"
            resetLabel={filterCategory !== "all" ? "Reset" : undefined}
            onReset={() => setFilterCategory("all")}
          >
            {providerFilters.map((cat) => (
              <CatalogFilterItem
                key={cat.id}
                icon={cat.Icon}
                label={cat.label}
                count={cat.count}
                active={filterCategory === cat.id}
                onSelect={() => {
                  setFilterCategory(cat.id);
                  setIsFilterDropdownOpen(false);
                }}
              />
            ))}
          </CatalogFilterMenu>
        ),
      }}
    />
  );

  const emptyStateNode = (
    <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
      <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No providers found</p>
      <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
        No providers match your search query or filter.
      </p>
      <button
        type="button"
        onClick={() => {
          setModalSearch("");
          setFilterCategory("all");
        }}
        className="mt-2 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
      >
        Reset Filters
      </button>
    </div>
  );

  return (
    <CatalogModalShell
      toolbar={toolbarNode}
      facetRail={
        <CatalogFacetRail
          groups={facetGroups}
          selectedId={filterCategory}
          onSelect={(id) => setFilterCategory(id as ProviderFilterCategory)}
          footer={facetRailFooter}
        />
      }
      scrollRef={scrollContainerRef}
      footer={<ModalProviderFooter category={category} />}
      isGuideActive={filterCategory === "guide"}
      guideView={
        <ProviderGuideView
          onBackToProviders={() => setFilterCategory("all")}
          category={category}
        />
      }
      isEmpty={!hasCards}
      emptyState={emptyStateNode}
    >
      <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
        {committedCategory === "server" && committedActiveProviderId
          ? <>{customCards}{localCard}{cloudCards}{creationCard}</>
          : committedCategory === "cloud" && committedActiveProviderId
          ? <>{cloudCards}{localCard}{customCards}{creationCard}</>
          : <>{localCard}{cloudCards}{customCards}{creationCard}</>}
      </div>
    </CatalogModalShell>
  );
});

ProviderModal.displayName = "ProviderModal";

export { upsertServer } from "./CustomServerFormCard";
