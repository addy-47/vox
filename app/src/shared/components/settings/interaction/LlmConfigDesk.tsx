import { useState, useEffect, memo, useCallback, useMemo } from "react";
import { useSettingsStore, LlmProviderConfig } from "@/store/settingsStore";
import { CLOUD_PROVIDERS, CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { checkLlmProviderHealth } from "@/services/settingsService";
import {
  Brain, Cloud, Network, Volume2, Sparkles, Mic,
  AlertCircle, ArrowLeft, Server, Search, Check, X, ArrowUpDown, Pencil
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField, ExpandableList, UnderlineInput } from "@/shared/ui";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { ProviderTier } from "./ProviderSelectorView";
import { RemoteModelsModalView } from "@/shared/components/settings/models/LlmCatalogView";
import { CloudProvidersModalView } from "@/shared/components/settings/models/CloudProvidersModalView";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";

interface LlmConfigDeskProps {
  activeCategory: "STT" | "LLM" | "TTS";
  activePill: ProviderTier;
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
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [modelsError, setModelsError] = useState<string | null>(null);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [detectedDialect, setDetectedDialect] = useState<string | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("asc");
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState("");
  const [modalView, setModalView] = useState<"providers" | "models">("providers");

  const activeLlmProvider = draftSettings?.llm?.active || "embedded";
  const currentRemoteConfig =
    activeLlmProvider === "server"
      ? draftSettings?.llm?.server
      : activeLlmProvider === "cloud"
      ? draftSettings?.llm?.cloud
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
  } = useRemoteLlmProbing(currentProvider, "llm", activePill === "cloud" || activePill === "server");


  const activeCloudProviderId = useMemo(() => {
    const cloudName = (draftSettings?.llm?.cloud?.provider_name || "").toLowerCase();
    const cloudUrl = draftSettings?.llm?.cloud?.base_url || "";
    const match = CLOUD_PROVIDERS.find(
      (p) =>
        p.id.toLowerCase() === cloudName ||
        p.name.toLowerCase() === cloudName ||
        (cloudUrl && p.url && cloudUrl.startsWith(p.url))
    );
    return match?.id || "openai";
  }, [draftSettings?.llm?.cloud?.base_url, draftSettings?.llm?.cloud?.provider_name]);

  const cloudKeys = useMemo(
    () => draftSettings?.llm?.cloud_keys ?? {},
    [draftSettings?.llm?.cloud_keys]
  );

  /** Resolves the stored API key for a cloud provider: explicit keyring entry
   * first, live draft key when it is the active provider. Single source for
   * the sort comparator and the list renderer below. */
  const getProviderKey = useCallback(
    (providerId: string) =>
      cloudKeys[providerId] ||
      (activeCloudProviderId === providerId ? draftSettings?.llm?.cloud?.api_key : "") ||
      "",
    [cloudKeys, activeCloudProviderId, draftSettings?.llm?.cloud?.api_key]
  );

  /** Committed (saved) active provider: list order follows this, so selecting
   * only re-highlights. The reorder (with smooth scroll) lands on save. */
  const committedSettings = useSettingsStore((s) => s.settings);
  const committedActiveCloudProviderId = useMemo(() => {
    const cloudName = (committedSettings?.llm?.cloud?.provider_name || "").toLowerCase();
    const cloudUrl = committedSettings?.llm?.cloud?.base_url || "";
    const match = CLOUD_PROVIDERS.find(
      (p) =>
        p.id.toLowerCase() === cloudName ||
        p.name.toLowerCase() === cloudName ||
        (cloudUrl && p.url && cloudUrl.startsWith(p.url))
    );
    return match?.id || "openai";
  }, [committedSettings?.llm?.cloud?.base_url, committedSettings?.llm?.cloud?.provider_name]);

  const getCommittedProviderKey = useCallback(
    (providerId: string) =>
      committedSettings?.llm?.cloud_keys?.[providerId] ||
      (committedActiveCloudProviderId === providerId ? committedSettings?.llm?.cloud?.api_key : "") ||
      "",
    [committedSettings?.llm?.cloud_keys, committedActiveCloudProviderId, committedSettings?.llm?.cloud?.api_key]
  );

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
    return [...list].sort((a, b) => {
      const aKey = Boolean(getCommittedProviderKey(a.id)?.trim());
      const bKey = Boolean(getCommittedProviderKey(b.id)?.trim());

      // Connected / configured providers appear at top
      if (aKey && !bKey) return -1;
      if (!aKey && bKey) return 1;

      // Within connected or unconnected group, sort alphabetically
      const cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      return sortOrder === "asc" ? cmp : -cmp;
    });
    }, [searchQuery, sortOrder, getCommittedProviderKey]);

  const { containerRef: providerGridRef, lenisRef: providerLenisRef } =
    useLenisScrollContainer<HTMLDivElement>();
  // Save-gated reorder glide: fires when a save changes the committed order.
  const providerOrderFingerprint = useMemo(
    () =>
      `${committedActiveCloudProviderId}|${CLOUD_PROVIDERS.map((p) =>
        getCommittedProviderKey(p.id)?.trim() ? "1" : "0"
      ).join("")}`,
    [committedActiveCloudProviderId, getCommittedProviderKey]
  );
  useScrollTopOnChange(providerGridRef, providerOrderFingerprint, providerLenisRef);

  const url = currentRemoteConfig?.base_url ?? "";
  const apiKey = currentRemoteConfig?.api_key ?? "";

  const remoteTtsEndpoint = draftSettings?.tts?.chatterbox_remote?.endpoint ?? "";
  const remoteTtsPath = draftSettings?.tts?.chatterbox_remote?.remote_path ?? "";

  const handleRemoteTtsEndpointChange = useCallback(
    (val: string) => {
      updateDraft("tts", "chatterbox_remote", {
        ...draftSettings?.tts?.chatterbox_remote,
        endpoint: val || "http://127.0.0.1:7860",
      });
    },
    [draftSettings?.tts?.chatterbox_remote, updateDraft]
  );

  const handleRemoteTtsPathChange = useCallback(
    (val: string) => {
      updateDraft("tts", "chatterbox_remote", {
        ...draftSettings?.tts?.chatterbox_remote,
        remote_path: val || "~/.vox",
      });
    },
    [draftSettings?.tts?.chatterbox_remote, updateDraft]
  );

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
    activeLlmProvider,
    updateDraft,
  ]);

  const handleUrlChange = useCallback(
    (val: string) => {
      if (activeLlmProvider === "server") {
        updateDraft("llm", "server", {
          ...draftSettings?.llm?.server,
          base_url: val || "http://127.0.0.1:11434",
        });
      } else if (activeLlmProvider === "cloud") {
        updateDraft("llm", "cloud", {
          ...draftSettings?.llm?.cloud,
          base_url: val || CLOUD_PROVIDERS[0].url,
        });
      }
    },
    [activeLlmProvider, draftSettings?.llm?.server, draftSettings?.llm?.cloud, updateDraft]
  );

  const handleApiKeyChange = useCallback(
    (key: string) => {
      if (activeLlmProvider === "server") {
        updateDraft("llm", "server", {
          ...draftSettings?.llm?.server,
          api_key: key || null,
        });
      } else if (activeLlmProvider === "cloud") {
        updateDraft("llm", "cloud", {
          ...draftSettings?.llm?.cloud,
          api_key: key || null,
        });
      }
    },
    [activeLlmProvider, draftSettings?.llm?.server, draftSettings?.llm?.cloud, updateDraft]
  );

  const handleSelectCloudProvider = useCallback(
    (provider: CloudProvider) => {
      const keys = draftSettings?.llm?.cloud_keys ?? {};
      const savedKey = keys[provider.id] || "";

      updateDraft("llm", "cloud", {
        ...draftSettings?.llm?.cloud,
        base_url: provider.url,
        provider_name: provider.name,
        api_key: savedKey || null,
      });

      if (draftSettings?.llm?.active !== "cloud") {
        updateDraft("llm", "active", "cloud");
      }
    },
    [draftSettings?.llm?.cloud, draftSettings?.llm?.cloud_keys, draftSettings?.llm?.active, updateDraft]
  );

  const handleStartInlineEdit = useCallback(
    (providerId: string, currentKey: string) => {
      setEditingProviderId(providerId);
      setEditingKeyValue(currentKey);
    },
    []
  );

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
  }, []);

  const openSearch = useCallback(() => setIsSearchOpen(true), []);
  const closeSearch = useCallback(() => {
    setIsSearchOpen(false);
    setSearchQuery("");
  }, []);

  const handleSaveInlineKey = useCallback(
    (providerId: string) => {
      const trimmed = editingKeyValue.trim();
      const currentKeys = { ...(draftSettings?.llm?.cloud_keys ?? {}) };
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
        draftSettings?.llm?.cloud?.provider_name?.toLowerCase() === targetProvider.name.toLowerCase();

      if (isCurrentlyActive) {
        updateDraft("llm", "cloud", {
          ...draftSettings?.llm?.cloud,
          base_url: targetProvider.url,
          provider_name: targetProvider.name,
          api_key: trimmed || null,
        });
      }

      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [editingKeyValue, draftSettings?.llm?.cloud_keys, draftSettings?.llm?.cloud, activeCloudProviderId, updateDraft]
  );

  if (!draftSettings) return null;

  /** Inline render helper (NOT a component): defined per-render, so it must be
   * invoked as a function — rendering as <ProviderSearchField /> would remount
   * the input (and reset scroll/focus) on every parent render. */
  const ProviderSearchField = ({ autoFocus = false }: { autoFocus?: boolean }) => (
    <div className="flex items-center gap-1.5 w-full min-w-0 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in">
      <Search size={12} className="text-[rgb(var(--accent))] shrink-0" />
      <input
        type="text"
        autoFocus={autoFocus}
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") closeSearch();
        }}
        placeholder={INTERACTION_CONFIG_DESK_COPY.llm.cloud.searchPlaceholder}
        aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.searchPlaceholder}
        className="flex-1 min-w-0 bg-transparent text-[11px] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
      />
      <button
        type="button"
        onClick={closeSearch}
        className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0"
        title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.closeSearch}
        aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.closeSearch}
      >
        <X size={12} />
      </button>
    </div>
  );

  /** The provider grid. Height-driven so the same element serves both the
   * constrained inline slot and the full-height expand modal. */
  /** Inline render helper (NOT a component): invoke as {ProviderRows()} so list
   * DOM (and its scroll offset) survives parent re-renders instead of remounting. */
  const ProviderRows = () => (
    <div ref={providerGridRef} className="h-full overflow-y-auto pr-1 grid grid-cols-1 sm:grid-cols-2 gap-2 content-start custom-scrollbar">
      {filteredProviders.length === 0 ? (
        <div className="col-span-full flex flex-col items-center justify-center gap-2 py-10 text-center">
          <p className="text-[12px] font-semibold text-[rgb(var(--foreground))]/80">
            {INTERACTION_CONFIG_DESK_COPY.llm.cloud.noProvidersMatch}
          </p>
          <button
            type="button"
            onClick={closeSearch}
            className="px-3 py-1 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
          >
            {INTERACTION_CONFIG_DESK_COPY.llm.cloud.clearSearch}
          </button>
        </div>
      ) : (
        filteredProviders.map((provider) => {
          const isSelected = activeCloudProviderId === provider.id;
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
                "group flex items-center justify-between gap-2 px-2.5 py-2 rounded-lg transition-all cursor-pointer select-none border min-h-[38px]",
                isSelected
                  ? "bg-[rgba(var(--accent),0.08)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.05)] hover:border-[rgba(var(--accent),0.15)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
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
                    title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey}
                    aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey}
                    className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0"
                  >
                    <Check size={13} />
                  </button>
                  <button
                    type="button"
                    onClick={handleCancelInlineKey}
                    title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey}
                    aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey}
                    className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0"
                  >
                    <X size={13} />
                  </button>
                </div>
              ) : (
                <>
                  <div className="flex items-center gap-2 min-w-0">
                    <span
                      className={cn(
                        "w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 transition-colors",
                        isSelected
                          ? "border-[rgb(var(--accent))]"
                          : "border-[rgba(var(--foreground),0.25)] group-hover:border-[rgba(var(--foreground),0.4)]"
                      )}
                    >
                      {isSelected && (
                        <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-scale-in" />
                      )}
                    </span>
                    <span className={cn(
                      "text-[13px] truncate leading-none",
                      isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium"
                    )}>
                      {provider.name}
                    </span>
                  </div>

                  <div className="shrink-0 flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                    {hasKey ? (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, savedKey)}
                        className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                        title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.editKey}
                        aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.editKey}
                      >
                        <Pencil size={12} />
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, "")}
                        className="text-[10.5px] font-medium text-[rgb(var(--accent))] hover:underline transition-all"
                      >
                        {INTERACTION_CONFIG_DESK_COPY.llm.cloud.connect}
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          );
        })
      )}
    </div>
  );


  // ── Standard Level-2 Header ──────────────────────────────────────────────
  // Left: breadcrumb back button + stage/modality title
  // Right: status indicator pill / badge
  const renderHeader = (
    icon: React.ReactNode,
    title: string,
    badge?: React.ReactNode
  ) => {
    const defaultBadge = () => {
      const baseStatus = checkingHealth
        ? INTERACTION_CONFIG_DESK_COPY.status.testing
        : isHealthy === false
        ? INTERACTION_CONFIG_DESK_COPY.status.offline
        : isHealthy === true
        ? INTERACTION_CONFIG_DESK_COPY.status.online
        : INTERACTION_CONFIG_DESK_COPY.status.active;

      const statusText =
        isHealthy === true && detectedDialect && activeLlmProvider === "server"
          ? `${detectedDialect} • ${baseStatus}`
          : baseStatus;

      return (
        <span className="text-[10px] font-mono font-bold tracking-wider text-[rgb(var(--accent))] uppercase">
          {statusText}
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
              aria-label={INTERACTION_CONFIG_DESK_COPY.status.backToProviders}
            >
              <ArrowLeft size={12} strokeWidth={2.5} className="group-hover:-translate-x-0.5 transition-transform" />
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

  /** Static description panel: header + centered copy. The six embedded/cloud
   * blurbs below share this exact layout; only icon, title, and text vary. */
  const renderBlurb = (
    icon: React.ReactNode,
    title: string,
    description: string,
    badge?: React.ReactNode
  ) => (
    <div className="flex flex-col justify-between h-full gap-2 animate-fade-in">
      {renderHeader(icon, title, badge)}
      <div className="flex-1 flex items-center p-3 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.06)]">
        <p className="text-[11.5px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/85 leading-relaxed font-medium">
          {description}
        </p>
      </div>
    </div>
  );

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
      {isModular && activeCategory === "STT" && activePill === "embedded" && (
        renderBlurb(
          <Mic size={14} className="text-[rgb(var(--accent))]" />,
          copy.stt.embedded.title,
          copy.stt.embedded.description
        )
      )}

      {isModular && activeCategory === "STT" && activePill === "server" && (
        renderBlurb(
          <Server size={14} className="text-[rgb(var(--accent))]" />,
          copy.stt.server.title,
          copy.stt.server.description,
          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
            {copy.stt.server.badge}
          </span>
        )
      )}

      {isModular && activeCategory === "STT" && activePill === "cloud" && (
        renderBlurb(
          <Cloud size={14} className="text-[rgb(var(--accent))]" />,
          copy.stt.cloud.title,
          copy.stt.cloud.description,
          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
            {copy.stt.cloud.badge}
          </span>
        )
      )}

      {/* ─── SECTION 2: LLM CATEGORY ─── */}
      {isModular && activeCategory === "LLM" && activePill === "embedded" && (
        renderBlurb(
          <Brain size={14} className="text-[rgb(var(--accent))]" />,
          copy.llm.embedded.title,
          copy.llm.embedded.description
        )
      )}

      {isModular && activeCategory === "LLM" && activePill === "server" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          {renderHeader(<Network size={14} className="text-[rgb(var(--accent))]" />, copy.llm.server.title)}

          <div
            className={cn(
              "grid gap-3 items-end flex-1 pb-0.5",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
            )}
          >
            <UnderlineInput
              label={copy.llm.server.urlLabel}
              value={url}
              onChange={(e) => handleUrlChange(e.target.value)}
              placeholder={copy.llm.server.urlPlaceholder}
            />
            <ApiKeyField
              label={copy.llm.server.apiKeyLabel}
              value={apiKey}
              onChange={handleApiKeyChange}
              placeholder={copy.llm.server.apiKeyPlaceholder}
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
        <div className="flex flex-col gap-2.5 h-full min-h-0 animate-fade-in">
          {renderHeader(
            <Cloud size={14} className="text-[rgb(var(--accent))]" />,
            copy.llm.cloud.title,
            <div className="flex items-center gap-1 shrink-0">
              <button
                type="button"
                onClick={() => setSortOrder((prev) => (prev === "asc" ? "desc" : "asc"))}
                className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                title={sortOrder === "asc" ? copy.llm.cloud.sortAsc : copy.llm.cloud.sortDesc}
                aria-label={sortOrder === "asc" ? copy.llm.cloud.sortAsc : copy.llm.cloud.sortDesc}
              >
                <ArrowUpDown size={13} strokeWidth={1.75} />
              </button>
              <button
                type="button"
                onClick={() => (isSearchOpen ? closeSearch() : openSearch())}
                className={cn(
                  "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer",
                  isSearchOpen || searchQuery
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]"
                )}
                title={copy.llm.cloud.searchPlaceholder}
                aria-label={copy.llm.cloud.searchPlaceholder}
              >
                <Search size={13} strokeWidth={1.75} />
              </button>
            </div>
          )}

          {/* 2-Column Provider List — inline at card height, full-bleed in the expand modal */}
          <ExpandableList
            className="flex-1 min-h-0"
            triggerPlacement="floating"
            inlineMaxHeightClass="max-h-[160px] sm:max-h-[180px]"
            icon={modalView === "models" ? <Network size={16} className="text-[rgb(var(--accent))]" /> : <Cloud size={16} className="text-[rgb(var(--accent))]" />}
            onBack={modalView === "models" ? () => setModalView("providers") : undefined}
            title={
              <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
                {modalView === "models" ? "Available Models" : copy.llm.cloud.title}
              </span>
            }
            subtitle={
              modalView === "models" ? (
                <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
                  {remoteModels.length} models
                </span>
              ) : (
                <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
                  {copy.llm.cloud.providerLabel} · {filteredProviders.length}
                </span>
              )
            }
            expandLabel={copy.llm.cloud.expandLabel}
            ariaLabel={copy.llm.cloud.expandAriaLabel}
            headerActions={
              <div className="flex items-center gap-2 min-w-0">
                {modalView === "models" ? null : (isSearchOpen || searchQuery ? <div className="w-[200px] sm:w-[260px]">{ProviderSearchField({})}</div> : null)}
                <SettingsCommitControls domainId="models" />
              </div>
            }
            modalContent={
              modalView === "models" ? (
                <RemoteModelsModalView
                  remoteModels={remoteModels}
                  selectedModelId={draftSettings?.llm?.cloud?.model}
                  pinnedModelId={committedSettings?.llm?.cloud?.model}
                  probingMap={probingMap}
                  capabilitiesCache={useSettingsStore.getState().capabilitiesCache}
                  onSelectModel={(modelId) => {
                    const cur = draftSettings?.llm?.cloud;
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
                />
              )
            }
          >
            {ProviderRows()}
          </ExpandableList>

          {isSearchOpen && (
            <div className="shrink-0">
              {ProviderSearchField({ autoFocus: true })}
            </div>
          )}

          {modelsError && (
            <span className="text-[10px] text-[rgb(var(--accent))] flex items-center gap-1 ml-0.5 shrink-0 font-mono">
              <AlertCircle size={11} /> {modelsError}
            </span>
          )}
        </div>
      )}

      {/* ─── SECTION 3: TTS CATEGORY ─── */}
      {isModular && activeCategory === "TTS" && activePill === "embedded" && (
        renderBlurb(
          <Volume2 size={14} className="text-[rgb(var(--accent))]" />,
          copy.tts.embedded.title,
          copy.tts.embedded.description
        )
      )}

      {isModular && activeCategory === "TTS" && activePill === "server" && (
        <div className="flex flex-col gap-2 h-full justify-between animate-fade-in">
          {renderHeader(<Network size={14} className="text-[rgb(var(--accent))]" />, copy.tts.server.title)}

          <div className="grid grid-cols-2 gap-3 pb-0.5">
            <UnderlineInput
              label={copy.tts.server.urlLabel}
              value={remoteTtsEndpoint}
              onChange={(e) => handleRemoteTtsEndpointChange(e.target.value)}
              placeholder={copy.tts.server.urlPlaceholder}
            />
            <UnderlineInput
              label={copy.tts.server.pathLabel}
              value={remoteTtsPath}
              onChange={(e) => handleRemoteTtsPathChange(e.target.value)}
              placeholder={copy.tts.server.pathPlaceholder}
            />
          </div>
        </div>
      )}

      {isModular && activeCategory === "TTS" && activePill === "cloud" && (
        renderBlurb(
          <Sparkles size={14} className="text-[rgb(var(--accent))]" />,
          copy.tts.cloud.title,
          copy.tts.cloud.description,
          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase font-mono border border-[rgb(var(--accent))]/20">
            {copy.tts.cloud.badge}
          </span>
        )
      )}
    </div>
  );
});

LlmConfigDesk.displayName = "LlmConfigDesk";
