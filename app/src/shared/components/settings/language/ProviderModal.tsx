import { memo, useState, useMemo, useCallback, useDeferredValue } from "react";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { type CloudProvider } from "@/data/providersCopy";
import { useCatalogStore } from "@/store/catalogStore";
import { PROVIDERS_CONFIG_DESK_COPY, LLM_CATALOG_COPY, LANGUAGE_CARD_COPY } from "@/data/settingsCopy";
import { useSettingsStore, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { useShallow } from "zustand/react/shallow";
import { activeSelection, getEffectiveCloudKeyMap, promoteActive, resolveProviderModel } from "@/shared/lib/activeSelection";
import { requestSpeechTab } from "@/shared/lib/speechTabSync";
import { filterProviders } from "@/shared/lib/catalogFilter";
import { ModalProviderFooter } from "@/shared/components/settings/SettingsCommitControls";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ArrowRight, Check, Cloud, Globe, HelpCircle, Key, KeyRound, Pencil, Server, Trash2, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ProviderGuideView } from "./ProviderGuideView";
import { CustomServerFormCard } from "./CustomServerFormCard";
import { SelectableCard } from "../shared/SelectableCard";
import { AddCard } from "../shared/AddCard";
import { CardFilterMenu, CardFilterItem } from "../shared/CardHeader";
import { CatalogModalToolbar } from "../shared/CatalogModalToolbar";
import { CatalogModalShell } from "../shared/CatalogModalShell";
import { CatalogFacetRail, type CatalogFacetGroup } from "../shared/CatalogFacetRail";

/** Deletes a custom server by ID and reverts to embedded if active. */
async function deleteCustomServer(domain: "llm" | "tts", serverId: string): Promise<void> {
  const { commitChanges, draftSettings: draft, updateDraft } = useSettingsStore.getState();
  const list = (domain === "tts" ? draft?.tts.servers ?? draft?.providers?.tts.servers : draft?.llm.servers ?? draft?.providers?.llm.servers) ?? [];
  console.info(`[Providers] deleteCustomServer domain=${domain} id=${serverId} prevCount=${list.length}`);
  updateDraft(domain, "servers", list.filter((s: { id: string }) => s.id !== serverId));
  const isActive = (domain === "tts" ? draft?.tts.active.provider : draft?.llm.active.provider) === serverId;
  if (isActive) {
    const fallback = (domain === "tts" ? draft?.tts.last_selected.embedded : draft?.llm.last_selected.embedded) ?? "";
    updateDraft(domain, "active", activeSelection("embedded", fallback));
  }
  await commitChanges({ domain, key: "servers" });
  if (isActive) await commitChanges({ domain, key: "active" });
}

/** Deletes a cloud provider key and reverts to embedded if active. */
async function deleteCloudProvider(domain: "llm" | "tts", providerId: string): Promise<void> {
  const { commitChanges, draftSettings: draft, settings: committed, updateDraft } = useSettingsStore.getState();
  const isRealtime = draft?.interaction.pipeline_mode === "realtime";
  if (isRealtime) {
    const currentKeys = { ...(draft?.realtime?.api_keys ?? {}) };
    delete currentKeys[providerId];
    updateDraft("realtime", "api_keys", currentKeys);
    if (providerId === "gemini_live") {
      const cloudKeys = getEffectiveCloudKeyMap(draft, committed);
      delete cloudKeys.gemini;
      updateDraft("llm", "cloud_keys", cloudKeys);
    }
    await commitChanges({ domain: "realtime", key: "api_keys" });
    return;
  }
  if (domain === "tts") {
    type Rt = { tts?: Record<string, Record<string, unknown>>; providers?: { tts?: Record<string, Record<string, unknown>> } };
    const rt = draft as Rt | null;
    const base = (rt?.tts?.[providerId] ?? rt?.providers?.tts?.[providerId] ?? {}) as Record<string, unknown>;
    const current = { ...base, key: "" };
    (updateDraft as (a: string, b: string, c: unknown) => void)("tts", providerId, current);
    const isActive = draft?.tts.active.category === "cloud" && draft?.tts.active.provider === providerId;
    if (isActive) {
      const fallback = draft?.tts.last_selected.embedded ?? "";
      updateDraft("tts", "active", activeSelection("embedded", fallback));
    }
    await (commitChanges as (x: { domain: string; key: string }) => Promise<void>)({ domain: "tts", key: providerId });
    if (isActive) await commitChanges({ domain: "tts", key: "active" });
  } else {
    const currentKeys = getEffectiveCloudKeyMap(draft, committed);
    delete currentKeys[providerId];
    updateDraft("llm", "cloud_keys", currentKeys);
    const isActive = draft?.llm.active.category === "cloud" && draft?.llm.active.provider === providerId;
    if (isActive) {
      const fallback = draft?.llm.last_selected.embedded ?? "";
      updateDraft("llm", "active", activeSelection("embedded", fallback));
    }
    await commitChanges({ domain: "llm", key: "cloud_keys" });
    if (isActive) await commitChanges({ domain: "llm", key: "active" });
  }
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

/** Dashed skeleton card for creating a custom server. Shared with every
 *  add-provider surface through `AddCard`; this wrapper only owns the copy. */
const CustomProviderCreationCard = memo(function CustomProviderCreationCard({
  category,
  onClick,
}: {
  category: "STT" | "LLM" | "TTS";
  onClick: () => void;
}) {
  const copy = PROVIDERS_CONFIG_DESK_COPY;
  const isTts = category === "TTS";
  return (
    <AddCard
      variant="tile"
      title={isTts ? copy.creationCard.addVoiceServer : copy.creationCard.addCustomServer}
      description={
        isTts
          ? copy.creationCard.voiceServerDescription
          : copy.creationCard.customServerDescription
      }
      footnote={copy.creationCard.selfHostedFootnote}
      onClick={onClick}
    />
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
  // One default at every entry point: an unconnected provider stays reachable
  // through its own row, but the list opens on what is ready to use.
  const [filterCategory, setFilterCategory] = useState<ProviderFilterCategory>("configured");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);
  const [isCreatingCustomServer, setIsCreatingCustomServer] = useState(false);
  const [editingServer, setEditingServer] = useState<LlmCustomServer | TtsCustomServer | null>(null);
  const [confirmDeleteProviderId, setConfirmDeleteProviderId] = useState<string | null>(null);

  const draftSettings = useSettingsStore(useShallow((s) => s.draftSettings));
  const committedSettings = useSettingsStore(useShallow((s) => s.settings));
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const isTts = category === "TTS";
  const isRealtime = category === "LLM" && draftSettings?.interaction.pipeline_mode === "realtime";

  const realtimeProviders: CloudProvider[] = useMemo(() => [
    {
      id: "gemini_live",
      name: "Gemini Live",
      tagline: "Google's direct live speech model with search grounding",
      url: "https://aistudio.google.com/apikey",
      keyPlaceholder: "AIzaSy...",
      isConfigured: Boolean(
        draftSettings?.realtime?.api_keys?.["gemini_live"] ||
        draftSettings?.providers?.realtime?.api_keys?.["gemini_live"] ||
        committedSettings?.providers?.realtime?.api_keys?.["gemini_live"] ||
        draftSettings?.llm?.cloud_keys?.["gemini"] ||
        draftSettings?.providers?.llm?.cloud_keys?.["gemini"] ||
        committedSettings?.providers?.llm?.cloud_keys?.["gemini"]
      ),
    },
    {
      id: "deepgram_voice_agent",
      name: "Deepgram Agent",
      tagline: "Deepgram's real-time conversational voice agent",
      url: "https://console.deepgram.com/",
      keyPlaceholder: "Token...",
      isConfigured: Boolean(
        draftSettings?.realtime?.api_keys?.["deepgram_voice_agent"] ||
        draftSettings?.providers?.realtime?.api_keys?.["deepgram_voice_agent"] ||
        committedSettings?.providers?.realtime?.api_keys?.["deepgram_voice_agent"]
      ),
    },
  ], [draftSettings, committedSettings]);

  const rawCloudProviders = useCatalogStore((s) => s.cloudProviders);
  const storeCloudProviders = isRealtime ? realtimeProviders : rawCloudProviders;
  const storeTtsProviders = useCatalogStore((s) => s.ttsProviders);

  const customServers = useMemo(() => {
    return (isTts ? draftSettings?.tts.servers ?? draftSettings?.providers?.tts.servers : draftSettings?.llm.servers ?? draftSettings?.providers?.llm.servers) ?? [];
  }, [isTts, draftSettings]);

  const activeCategoryType = useMemo(() => {
    const domain = category.toLowerCase() as "stt" | "tts" | "llm";
    return draftSettings?.[domain].active.category ?? "embedded";
  }, [category, draftSettings]);

  const isLocalSelected = activeCategoryType === "embedded";
  const activeCloudId = useMemo(() => {
    if (activeCategoryType !== "cloud") return null;
    return activeCloudProviderId ?? (isTts ? draftSettings?.tts.active.provider : draftSettings?.llm.active.provider) ?? null;
  }, [activeCategoryType, activeCloudProviderId, isTts, draftSettings]);

  const activeServerId = activeCategoryType === "server" ? (isTts ? draftSettings?.tts.active.provider : draftSettings?.llm.active.provider) ?? null : null;

  const handleSelectServer = useCallback((server: LlmCustomServer | TtsCustomServer) => {
    useCatalogStore.getState().clearRemoteModelsError();
    const committedActive = isTts
      ? committedSettings?.tts?.active ?? null
      : committedSettings?.llm?.active ?? null;
    if (isTts) {
      updateDraft(
        "tts",
        "active",
        activeSelection(
          "server",
          resolveProviderModel({ category: "server", provider: server.id }, committedActive),
          server.id
        )
      );
      requestSpeechTab("tts");
    } else {
      const llmServ = server as LlmCustomServer;
      updateDraft(
        "llm",
        "active",
        activeSelection(
          "server",
          resolveProviderModel(
            { category: "server", provider: llmServ.id },
            committedActive,
            llmServ.model || ""
          ),
          llmServ.id
        )
      );
    }
  }, [isTts, committedSettings, updateDraft]);

  const handleDeleteServer = useCallback(async (serverId: string) => {
    await deleteCustomServer(isTts ? "tts" : "llm", serverId);
  }, [isTts]);

  const handleDeleteCloudProvider = useCallback(async (providerId: string) => {
    if (editingProviderId === providerId) {
      onCancelInlineKey();
    }
    await deleteCloudProvider(isTts ? "tts" : "llm", providerId);
  }, [isTts, editingProviderId, onCancelInlineKey]);

  const handleSelectLocalProvider = useCallback(() => {
    useCatalogStore.getState().clearRemoteModelsError();
    if (onSelectLocal) {
      onSelectLocal();
    } else {
      const domain = category.toLowerCase() as "stt" | "llm" | "tts";
      const fallback = draftSettings?.[domain].last_selected.embedded ?? "";
      updateDraft(
        domain,
        "active",
        activeSelection(
          "embedded",
          resolveProviderModel(
            { category: "embedded", provider: null },
            committedSettings?.[domain]?.active ?? null,
            fallback
          )
        )
      );
      if (domain === "tts" || domain === "stt") requestSpeechTab(domain);
    }
  }, [category, committedSettings, draftSettings, onSelectLocal, updateDraft]);

  const { containerRef: scrollContainerRef, lenisRef } = useLenisScrollContainer<HTMLDivElement>();
  const committedActiveProviderId = isTts
    ? committedSettings?.tts.active.provider
    : committedSettings?.llm.active.provider;
  const committedCategory = isTts
    ? committedSettings?.tts.active.category
    : committedSettings?.llm.active.category;
  const isCommittedLocal = committedCategory === "embedded";

  const orderFingerprint = useMemo(
    () => `${committedActiveProviderId ?? ""}|${isCommittedLocal ? "local" : ""}|${pinnedProviderId ?? ""}|${storeCloudProviders.map((p) => getCommittedProviderKey(p.id)?.trim() ? "1" : "0").join("")}`,
    [committedActiveProviderId, isCommittedLocal, pinnedProviderId, getCommittedProviderKey, storeCloudProviders]
  );
  useScrollTopOnChange(scrollContainerRef, orderFingerprint, lenisRef);

  const configuredCount = useMemo(() => storeCloudProviders.filter((p) => Boolean(p.isConfigured) || Boolean(getProviderKey(p.id)?.trim())).length, [getProviderKey, storeCloudProviders]);
  const unconfiguredCount = useMemo(() => storeCloudProviders.filter((p) => !p.isConfigured && !getProviderKey(p.id)?.trim()).length, [getProviderKey, storeCloudProviders]);
  const ttsAvailableCount = useMemo(() => storeTtsProviders.filter((p) => p.isConfigured || p.isFree || Boolean(getProviderKey(p.id)?.trim())).length, [getProviderKey, storeTtsProviders]);

  const sourceFilters = useMemo(
    () => [
      { id: "all" as const, label: "All Providers", count: 1 + (isTts ? storeTtsProviders.length : storeCloudProviders.length) + customServers.length, Icon: Globe },
      { id: "configured" as const, label: "Available", count: 1 + (isTts ? ttsAvailableCount : configuredCount) + customServers.length, Icon: Key },
      { id: "custom" as const, label: "Custom Servers", count: customServers.length, Icon: Server },
      { id: "cloud" as const, label: isTts ? "Cloud Voices" : "Cloud Providers", count: isTts ? storeTtsProviders.length : storeCloudProviders.length, Icon: Cloud },
    ],
    [isTts, customServers.length, ttsAvailableCount, configuredCount, storeTtsProviders.length, storeCloudProviders.length]
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
    if (filterCategory === "custom") return [];
    const list = filterProviders(
      storeTtsProviders.map((p) => ({ ...p, url: p.url ?? null })),
      filterCategory === "all" ? "all" : filterCategory,
      deferredSearch,
      { getProviderKey }
    );
    return promoteActive(
      [...list].sort((a, b) => sort === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" })),
      committedSettings?.tts.active.provider
    );
  }, [deferredSearch, filterCategory, getProviderKey, isTts, sort, committedSettings?.tts.active.provider, storeTtsProviders]);

  const modalFilteredProviders = useMemo(() => {
    if (filterCategory === "custom") return [];
    const list = filterProviders(
      storeCloudProviders,
      filterCategory === "guide" ? "all" : filterCategory,
      deferredSearch,
      { getProviderKey }
    );

    return promoteActive(
      [...list].sort((a, b) => {
        const aKey = Boolean(getCommittedProviderKey(a.id)?.trim()), bKey = Boolean(getCommittedProviderKey(b.id)?.trim());
        if (aKey !== bKey) return aKey ? -1 : 1;
        return sort === "az" ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
      }),
      committedSettings?.llm.active.provider
    );
  }, [deferredSearch, filterCategory, getProviderKey, getCommittedProviderKey, sort, committedSettings?.llm.active.provider, storeCloudProviders]);

  const matchesLocal = useMemo(() => {
    const q = deferredSearch.trim().toLowerCase();
    return !q || "built-in on-device local neural whisper gguf kokoro vox offline".includes(q);
  }, [deferredSearch]);

  const showLocalCard = !isRealtime && (filterCategory === "all" || filterCategory === "configured") && matchesLocal;
  const showCustomSection = !isRealtime && (filterCategory === "all" || filterCategory === "configured" || filterCategory === "custom");
  const showCustomCreationCard = showCustomSection && (!deferredSearch.trim() || "custom server self-hosted endpoint".includes(deferredSearch.trim().toLowerCase()));

  const filteredCustomServers = useMemo(() => {
    if (!showCustomSection) return [];
    const q = deferredSearch.trim().toLowerCase();
    return promoteActive(
      !q ? [...customServers] : customServers.filter((s) => (s.name && s.name.toLowerCase().includes(q)) || (s.url && s.url.toLowerCase().includes(q)) || (s.id && s.id.toLowerCase().includes(q))),
      isTts ? committedSettings?.tts.active.provider : committedSettings?.llm.active.provider
    );
  }, [showCustomSection, deferredSearch, customServers, isTts, committedSettings?.tts.active.provider, committedSettings?.llm.active.provider]);

  const localMeta = PROVIDERS_CONFIG_DESK_COPY.localMeta[category];

  const localCard = showLocalCard ? (
    <SelectableCard
      key="local-provider-card" id="local-provider" name={localMeta.name} tagline={localMeta.tagline} logo={null}
      isSelected={isLocalSelected} onSelect={handleSelectLocalProvider}
      actionSlot={
        <div className="flex items-center justify-between text-[11px] font-mono w-full">
          <span className="text-[rgb(var(--foreground-muted))]/70">{PROVIDERS_CONFIG_DESK_COPY.badges.localHardware}</span>
          <span className="text-[rgb(var(--accent))] font-medium flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
            {isLocalSelected ? PROVIDERS_CONFIG_DESK_COPY.status.active : PROVIDERS_CONFIG_DESK_COPY.badges.offlineReady}
          </span>
        </div>
      }
    />
  ) : null;

  const { connectedCloudCards, unconnectedCloudCards } = useMemo(() => {
    const connected: Array<{ key: string; node: React.ReactNode }> = [];
    const unconnected: Array<{ key: string; node: React.ReactNode }> = [];

    if (isTts) {
      ttsFilteredProviders.forEach((entry) => {
        const isSelected =
          activeCloudId === entry.id ||
          (entry.id === "edge_speech" &&
            ((activeCloudId?.includes("edge") ?? false) ||
              (draftSettings?.tts.active.model.includes("edge") ?? false)));

        const node = (
          <SelectableCard
            key={entry.id}
            id={entry.id}
            name={entry.name}
            tagline={entry.tagline}
            logo={entry.logo}
            isSelected={isSelected}
            isComingSoon={entry.isComingSoon}
            onSelect={() => {
              if (entry.isComingSoon) return;
              useCatalogStore.getState().clearRemoteModelsError();
              const ttsModel =
                entry.id === "edge_speech" ? entry.id.replace("speech", "tts") : entry.id;
              updateDraft("tts", "active", activeSelection("cloud", ttsModel, entry.id));
              requestSpeechTab("tts");
              onSelectProvider({
                id: entry.id,
                name: entry.name,
                url: entry.url || "",
                popular: true,
                keyPlaceholder: "API Key (Optional)...",
              });
            }}
            actionSlot={
              <div className="flex items-center justify-between text-[11px] font-mono w-full">
                <span className="text-[rgb(var(--foreground-muted))]/70">
                  {entry.isFree
                    ? PROVIDERS_CONFIG_DESK_COPY.badges.keylessCloud
                    : PROVIDERS_CONFIG_DESK_COPY.badges.cloudVoice}
                </span>
                <span
                  className={cn(
                    "font-medium flex items-center gap-1.5",
                    entry.isComingSoon
                      ? "text-[rgb(var(--foreground-muted))]/60 font-normal"
                      : "text-[rgb(var(--accent))]"
                  )}
                >
                  {!entry.isComingSoon && <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />}
                  {entry.isComingSoon
                    ? PROVIDERS_CONFIG_DESK_COPY.status.inDevelopment
                    : isSelected
                      ? PROVIDERS_CONFIG_DESK_COPY.status.active
                      : PROVIDERS_CONFIG_DESK_COPY.status.ready}
                </span>
              </div>
            }
          />
        );

        if (entry.isFree || !entry.isComingSoon) {
          connected.push({ key: `cloud:${entry.id}`, node });
        } else {
          unconnected.push({ key: `cloud:${entry.id}`, node });
        }
      });
    } else {
      modalFilteredProviders.forEach((provider) => {
        const isSelected = activeCloudId === provider.id;
        const savedKey = getProviderKey(provider.id);
        const isEditing = editingProviderId === provider.id;
        const hasKey = Boolean(savedKey && savedKey.trim().length > 0);

        const node = (
          <SelectableCard
            key={provider.id}
            id={provider.id}
            name={provider.name}
            tagline={provider.tagline || "OpenAI-compatible inference gateway."}
            logo={provider.logo ?? null}
            isSelected={isSelected}
            onSelect={() => onSelectProvider(provider)}
            cornerSlot={
              onNavigateToModels && hasKey ? (
                <Tooltip label={LANGUAGE_CARD_COPY.browseModels} side="top" asChild>
                  <button
                    type="button"
                    onClick={() => {
                      onSelectProvider(provider);
                      onNavigateToModels(provider);
                    }}
                    className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer"
                    aria-label={`${LANGUAGE_CARD_COPY.browseModels}: ${provider.name}`}
                  >
                    <ArrowRight size={13} strokeWidth={2} />
                  </button>
                </Tooltip>
              ) : undefined
            }
            actionSlot={
              isEditing ? (
                <div className="flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 w-full bg-transparent animate-fade-in">
                  <input
                    type="password"
                    autoFocus
                    value={editingKeyValue}
                    onChange={(e) => setEditingKeyValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") onSaveInlineKey(provider.id);
                      if (e.key === "Escape") onCancelInlineKey();
                    }}
                    placeholder={provider.keyPlaceholder}
                    className="w-full bg-transparent text-[11px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30"
                  />
                  <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.saveKey} side="top" asChild>
                    <button
                      type="button"
                      onClick={() => onSaveInlineKey(provider.id)}
                      aria-label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.saveKey}
                      className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0 bg-transparent cursor-pointer"
                    >
                      <Check size={13} strokeWidth={2.2} />
                    </button>
                  </Tooltip>
                  <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.cancelKey} side="top" asChild>
                    <button
                      type="button"
                      onClick={onCancelInlineKey}
                      aria-label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.cancelKey}
                      className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 bg-transparent cursor-pointer"
                    >
                      <X size={13} />
                    </button>
                  </Tooltip>
                </div>
              ) : (
                <div className="flex items-center justify-between text-[11px] font-mono w-full">
                  <span className="text-[rgb(var(--foreground-muted))]/70">
                    {PROVIDERS_CONFIG_DESK_COPY.badges.cloudGateway}
                  </span>
                  {hasKey ? (
                    confirmDeleteProviderId === provider.id ? (
                      <div className="flex items-center gap-1">
                        <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider} side="top" asChild>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteCloudProvider(provider.id);
                              setConfirmDeleteProviderId(null);
                            }}
                            className="p-1 rounded-md bg-[rgba(var(--danger),0.18)] text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.3)] transition-colors cursor-pointer"
                            aria-label="Confirm delete"
                          >
                            <Check size={12} strokeWidth={2.75} />
                          </button>
                        </Tooltip>
                        <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              setConfirmDeleteProviderId(null);
                            }}
                            className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                            aria-label="Cancel delete"
                          >
                            <X size={12} strokeWidth={2.5} />
                          </button>
                        </Tooltip>
                      </div>
                    ) : (
                      <div className="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                        <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey} side="top" asChild>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              onStartInlineEdit(provider.id, savedKey ?? "");
                            }}
                            className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-all cursor-pointer"
                            aria-label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey}
                          >
                            <Pencil size={13} />
                          </button>
                        </Tooltip>
                        <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider} side="top" asChild>
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              setConfirmDeleteProviderId(provider.id);
                            }}
                            className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))] hover:bg-[rgba(var(--error),0.1)] transition-all cursor-pointer"
                            aria-label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider}
                          >
                            <Trash2 size={13} />
                          </button>
                        </Tooltip>
                      </div>
                    )
                  ) : (
                    <button
                      type="button"
                      onClick={() => onStartInlineEdit(provider.id, "")}
                      className="text-[rgb(var(--accent))] hover:underline cursor-pointer font-medium flex items-center gap-1"
                    >
                      {PROVIDERS_CONFIG_DESK_COPY.llm.cloud.connectKey}
                    </button>
                  )}
                </div>
              )
            }
          />
        );

        if (hasKey) {
          connected.push({ key: `cloud:${provider.id}`, node });
        } else {
          unconnected.push({ key: `cloud:${provider.id}`, node });
        }
      });
    }

    return { connectedCloudCards: connected, unconnectedCloudCards: unconnected };
  }, [
    isTts,
    ttsFilteredProviders,
    modalFilteredProviders,
    activeCloudId,
    draftSettings?.tts.active.model,
    updateDraft,
    onSelectProvider,
    editingProviderId,
    editingKeyValue,
    onSaveInlineKey,
    onCancelInlineKey,
    onNavigateToModels,
    getProviderKey,
    confirmDeleteProviderId,
    handleDeleteCloudProvider,
    onStartInlineEdit,
    setEditingKeyValue,
  ]);

  const customCards = filteredCustomServers.map((server) => {
    if (editingServer?.id === server.id) {
      return {
        key: `server:${server.id}`,
        node: (
          <CustomServerFormCard
            key={server.id}
            category={category}
            initial={server}
            onClose={() => setEditingServer(null)}
            onServerSaved={() => {}}
          />
        ),
      };
    }
    const isSelected = activeCategoryType === "server" && activeServerId === server.id;
    return {
      key: `server:${server.id}`,
      node: (
        <SelectableCard
          key={server.id || server.url}
          id={server.id}
          name={server.name || "Custom Server"}
          tagline={server.url}
          logo={null}
          providerName={server.name}
          isSelected={isSelected}
          onSelect={() => handleSelectServer(server)}
          cornerSlot={
            onNavigateToModels && category === "LLM" ? (
              <Tooltip label={LANGUAGE_CARD_COPY.browseModels} side="top" asChild>
                <button
                  type="button"
                  onClick={() => {
                    handleSelectServer(server);
                    onNavigateToModels({
                      id: server.id,
                      name: server.name || "Custom Server",
                      url: server.url,
                      popular: false,
                      keyPlaceholder: "",
                    });
                  }}
                  className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer"
                  aria-label={`${LANGUAGE_CARD_COPY.browseModels}: ${server.name || "Custom Server"}`}
                >
                  <ArrowRight size={13} strokeWidth={2} />
                </button>
              </Tooltip>
            ) : undefined
          }
          actionSlot={
            <div className="flex items-center justify-between text-[11px] font-mono w-full">
              <span className="text-[rgb(var(--foreground-muted))]/70">
                {PROVIDERS_CONFIG_DESK_COPY.badges.selfHosted}
              </span>
              {confirmDeleteProviderId === server.id ? (
                <div className="flex items-center gap-1">
                  <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteServer} side="top" asChild>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteServer(server.id);
                        setConfirmDeleteProviderId(null);
                      }}
                      className="p-1 rounded-md bg-[rgba(var(--danger),0.18)] text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.3)] transition-colors cursor-pointer"
                      aria-label="Confirm delete"
                    >
                      <Check size={12} strokeWidth={2.75} />
                    </button>
                  </Tooltip>
                  <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setConfirmDeleteProviderId(null);
                      }}
                      className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                      aria-label="Cancel delete"
                    >
                      <X size={12} strokeWidth={2.5} />
                    </button>
                  </Tooltip>
                </div>
              ) : (
                <div className="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                  <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.editServer} side="top">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setEditingServer(server);
                        setIsCreatingCustomServer(false);
                      }}
                      className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-all cursor-pointer"
                      aria-label={PROVIDERS_CONFIG_DESK_COPY.actions.editServer}
                    >
                      <Pencil size={13} />
                    </button>
                  </Tooltip>
                  <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteServer} side="top">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setConfirmDeleteProviderId(server.id);
                      }}
                      className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))] hover:bg-[rgba(var(--error),0.1)] transition-all cursor-pointer"
                      aria-label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteServer}
                    >
                      <Trash2 size={13} />
                    </button>
                  </Tooltip>
                </div>
              )}
            </div>
          }
        />
      ),
    };
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

  const { readyElements, restElements } = useMemo(() => {
    let savedNode: { key: string; node: React.ReactNode } | null = null;

    if (isCommittedLocal && localCard) {
      savedNode = { key: "local-provider-card", node: localCard };
    } else if (committedCategory === "server" && committedActiveProviderId) {
      const idx = customCards.findIndex((c) => c.key === `server:${committedActiveProviderId}`);
      if (idx !== -1) {
        savedNode = customCards[idx];
      }
    } else if (committedCategory === "cloud" && committedActiveProviderId) {
      const connIdx = connectedCloudCards.findIndex((c) => c.key === `cloud:${committedActiveProviderId}`);
      if (connIdx !== -1) {
        savedNode = connectedCloudCards[connIdx];
      } else {
        const unconnIdx = unconnectedCloudCards.findIndex((c) => c.key === `cloud:${committedActiveProviderId}`);
        if (unconnIdx !== -1) {
          savedNode = unconnectedCloudCards[unconnIdx];
        }
      }
    }

    const orderedConnectedClouds = connectedCloudCards
      .filter((c) => c.key !== savedNode?.key)
      .map((c) => c.node);

    const orderedCustomServers = customCards
      .filter((c) => c.key !== savedNode?.key)
      .map((c) => c.node);

    const orderedLocal = localCard && savedNode?.key !== "local-provider-card" ? [localCard] : [];

    const ready: React.ReactNode[] = [];
    if (savedNode) {
      ready.push(savedNode.node);
    }
    ready.push(...orderedConnectedClouds);
    ready.push(...orderedCustomServers);
    ready.push(...orderedLocal);
    if (showCustomSection && (isCreatingCustomServer || (showCustomCreationCard && !editingServer))) {
      ready.push(creationCard);
    }

    const rest = unconnectedCloudCards
      .filter((c) => c.key !== savedNode?.key)
      .map((c) => c.node);

    return { readyElements: ready, restElements: rest };
  }, [
    isCommittedLocal,
    localCard,
    committedCategory,
    committedActiveProviderId,
    customCards,
    connectedCloudCards,
    unconnectedCloudCards,
    showCustomSection,
    isCreatingCustomServer,
    showCustomCreationCard,
    editingServer,
    creationCard,
  ]);

  const currentItemCount = (showLocalCard ? 1 : 0) + (isTts ? ttsFilteredProviders.length : modalFilteredProviders.length) + filteredCustomServers.length;
  const totalItemCount = (isTts ? storeTtsProviders.length : storeCloudProviders.length) + customServers.length + 1;

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
        label: PROVIDERS_CONFIG_DESK_COPY.filterLabel,
        open: isFilterDropdownOpen,
        activeCount: filterCategory === "all" ? 0 : 1,
        onToggle: () => setIsFilterDropdownOpen((v) => !v),
        onClose: () => setIsFilterDropdownOpen(false),
        menu: (
          <CardFilterMenu
            title={PROVIDERS_CONFIG_DESK_COPY.filterMenuTitle}
            resetLabel={filterCategory !== "all" ? PROVIDERS_CONFIG_DESK_COPY.reset : undefined}
            onReset={() => setFilterCategory("all")}
          >
            {providerFilters.map((cat) => (
              <CardFilterItem
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
          </CardFilterMenu>
        ),
      }}
    />
  );

  const emptyStateNode = (
    <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
      <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">{LLM_CATALOG_COPY.providerModalEmptyTitle}</p>
      <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
        {LLM_CATALOG_COPY.providerModalEmptyHint}
      </p>
      <button
        type="button"
        onClick={() => {
          setModalSearch("");
          setFilterCategory("all");
        }}
        className="mt-2 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
      >
        {LLM_CATALOG_COPY.resetFilters}
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
        {readyElements}
        {readyElements.length > 0 && restElements.length > 0 && (
          <div className="col-span-full my-2 h-px w-full bg-[rgba(var(--foreground),0.08)]" />
        )}
        {restElements}
      </div>
    </CatalogModalShell>
  );
});

ProviderModal.displayName = "ProviderModal";
