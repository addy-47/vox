import { memo, useState, useRef, useEffect, useMemo, useCallback, useDeferredValue } from "react";
import Lenis from "lenis";
import { CLOUD_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { Search, X, Check, Pencil, ArrowRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface CloudProviderCardProps {
  provider: CloudProvider;
  isSelected: boolean;
  savedKey: string | undefined;
  isEditing: boolean;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  onSelect: (provider: CloudProvider) => void;
  onStartInlineEdit: (id: string, currentKey: string) => void;
  onCancelInlineKey: () => void;
  onSaveInlineKey: (id: string) => void;
  onNavigateToModels?: (provider: CloudProvider) => void;
}

export const CloudProviderCard = memo(({
  provider,
  isSelected,
  savedKey,
  isEditing,
  editingKeyValue,
  setEditingKeyValue,
  onSelect,
  onStartInlineEdit,
  onCancelInlineKey,
  onSaveInlineKey,
  onNavigateToModels,
}: CloudProviderCardProps) => {
  const hasKey = Boolean(savedKey && savedKey.trim().length > 0);

  return (
    <div
      onClick={() => {
        if (!isEditing) {
          onSelect(provider);
        }
      }}
      className={cn(
        "relative flex flex-col justify-between p-4 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu will-change-transform",
        isSelected
          ? "bg-[rgba(var(--accent),0.07)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.12)] ring-1 ring-[rgb(var(--accent))]/30"
          : "bg-[rgba(var(--card),0.5)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.85)]"
      )}
    >
      <div>
        {/* Top Row: Provider Name + Tagline + Radio Sphere */}
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <h4 className="font-display text-[15px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
              {provider.name}
            </h4>
            {provider.tagline && (
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2">
                {provider.tagline}
              </p>
            )}
          </div>

          {/* Minimal Radio Sphere Selection + Models Navigation Arrow */}
          <div className="flex items-center gap-1.5 shrink-0 ml-1">
            <div
              role="radio"
              aria-checked={isSelected}
              className="p-1 -m-1 cursor-pointer flex items-center justify-center shrink-0"
              onClick={(e) => {
                e.stopPropagation();
                onSelect(provider);
              }}
              title={isSelected ? "Active Provider" : "Select Provider"}
            >
              {isSelected ? (
                <div className="w-4 h-4 rounded-full border border-[rgb(var(--accent))] flex items-center justify-center bg-[rgb(var(--accent))]/15 shrink-0 shadow-sm">
                  <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
                </div>
              ) : (
                <div className="w-4 h-4 rounded-full border border-[rgba(var(--foreground),0.28)] hover:border-[rgba(var(--foreground),0.55)] transition-colors shrink-0" />
              )}
            </div>

            {onNavigateToModels && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect(provider);
                  onNavigateToModels(provider);
                }}
                className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer shrink-0"
                title="Browse models"
                aria-label="Browse models"
              >
                <ArrowRight size={13} strokeWidth={2} />
              </button>
            )}
          </div>
        </div>

        {/* Endpoint URL */}
        <div className="mt-3 p-2 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/75 truncate select-all">
          {provider.url}
        </div>

        {/* API Key Management */}
        <div className="mt-3" onClick={(e) => e.stopPropagation()}>
          {isEditing ? (
            <div className="flex items-center gap-1.5 p-1.5 rounded-lg border border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.05)]">
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
                className="w-full bg-transparent text-[11.5px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 px-1"
              />
              <button
                type="button"
                onClick={() => onSaveInlineKey(provider.id)}
                title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey}
                className="p-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 transition-all cursor-pointer shrink-0"
              >
                <Check size={12} strokeWidth={2.5} />
              </button>
              <button
                type="button"
                onClick={onCancelInlineKey}
                title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey}
                className="p-1 rounded text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
              >
                <X size={12} />
              </button>
            </div>
          ) : hasKey ? (
            <div className="flex items-center justify-between gap-2 p-1.5 rounded-lg bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] text-[11px] font-mono">
              <span className="text-[rgb(var(--foreground-muted))] truncate pl-1">
                ••••••••••••{savedKey?.slice(-4)}
              </span>
              <button
                type="button"
                onClick={() => onStartInlineEdit(provider.id, savedKey ?? "")}
                className="p-1 rounded hover:bg-white/[0.08] hover:text-[rgb(var(--accent))] transition-colors text-[rgb(var(--foreground-muted))] cursor-pointer shrink-0"
                title="Edit API Key"
              >
                <Pencil size={12} />
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => onStartInlineEdit(provider.id, "")}
              className="w-full py-1.5 rounded-lg border border-dashed border-[rgba(var(--accent),0.3)] text-[11px] font-mono text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.06)] hover:border-[rgb(var(--accent))] transition-all text-center cursor-pointer"
            >
              + Set API Key
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
CloudProviderCard.displayName = "CloudProviderCard";

export interface CloudProvidersModalViewProps {
  activeCloudProviderId: string;
  getProviderKey: (id: string) => string | undefined;
  onSelectProvider: (provider: CloudProvider) => void;
  editingProviderId: string | null;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  onStartInlineEdit: (id: string, currentKey: string) => void;
  onCancelInlineKey: () => void;
  onSaveInlineKey: (id: string) => void;
  onNavigateToModels?: (provider: CloudProvider) => void;
}

export const CloudProvidersModalView = memo(({
  activeCloudProviderId,
  getProviderKey,
  onSelectProvider,
  editingProviderId,
  editingKeyValue,
  setEditingKeyValue,
  onStartInlineEdit,
  onCancelInlineKey,
  onSaveInlineKey,
  onNavigateToModels,
}: CloudProvidersModalViewProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [filterCategory, setFilterCategory] = useState<"all" | "configured" | "popular" | "unconfigured">("all");
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return undefined;

    const lenis = new Lenis({
      wrapper: el,
      content: el,
      eventsTarget: el,
      smoothWheel: true,
      autoRaf: true,
      duration: 0.75,
    });

    return () => {
      lenis.destroy();
    };
  }, []);

  const configuredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => Boolean(getProviderKey(p.id)?.trim())).length;
  }, [getProviderKey]);

  const popularCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => p.popular).length;
  }, []);

  const unconfiguredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => !getProviderKey(p.id)?.trim()).length;
  }, [getProviderKey]);

  const modalFilteredProviders = useMemo(() => {
    let list = CLOUD_PROVIDERS;
    const q = deferredSearch.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.id.toLowerCase().includes(q) ||
          p.tagline?.toLowerCase().includes(q) ||
          p.url.toLowerCase().includes(q)
      );
    }

    if (filterCategory === "configured") {
      list = list.filter((p) => Boolean(getProviderKey(p.id)?.trim()));
    } else if (filterCategory === "popular") {
      list = list.filter((p) => p.popular);
    } else if (filterCategory === "unconfigured") {
      list = list.filter((p) => !getProviderKey(p.id)?.trim());
    }

    return [...list].sort((a, b) => {
      const aKey = Boolean(getProviderKey(a.id)?.trim());
      const bKey = Boolean(getProviderKey(b.id)?.trim());
      if (aKey && !bKey) return -1;
      if (!aKey && bKey) return 1;
      return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    });
  }, [deferredSearch, filterCategory, getProviderKey]);

  return (
    <div className="flex flex-col h-full min-h-0 gap-3">
      {/* Modal Toolbar: Search + Filter Chips Bar */}
      <div className="flex flex-col gap-2.5 shrink-0">
        <div className="flex items-center justify-between gap-3">
          <div className="flex-1 flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
            <Search size={14} className="text-[rgb(var(--foreground-muted))] shrink-0" />
            <input
              type="text"
              value={modalSearch}
              onChange={(e) => setModalSearch(e.target.value)}
              placeholder="Search cloud providers, model ecosystems, endpoints..."
              className="w-full bg-transparent border-none outline-none text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            {modalSearch && (
              <button
                type="button"
                onClick={() => setModalSearch("")}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
              >
                <X size={13} />
              </button>
            )}
          </div>

          <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0">
            <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredProviders.length}</span> of {CLOUD_PROVIDERS.length} providers</span>
            <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredProviders.length}</span>/{CLOUD_PROVIDERS.length}</span>
          </div>
        </div>

        {/* Filter Chips Bar */}
        <div className="flex items-center gap-1.5 flex-wrap">
          {(
            [
              { id: "all", label: `All (${CLOUD_PROVIDERS.length})` },
              { id: "configured", label: `Configured (${configuredCount})` },
              { id: "popular", label: `Popular (${popularCount})` },
              { id: "unconfigured", label: `Needs Key (${unconfiguredCount})` },
            ] as const
          ).map((chip) => (
            <button
              key={chip.id}
              type="button"
              onClick={() => setFilterCategory(chip.id)}
              className={cn(
                "px-2.5 py-1 rounded-md text-[11px] font-mono font-medium transition-all cursor-pointer border",
                filterCategory === chip.id
                  ? "bg-[rgb(var(--accent))]/15 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] shadow-sm"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.2)]"
              )}
            >
              {chip.label}
            </button>
          ))}
        </div>
      </div>

      {/* Contained Cards Scroll Area with Constant Inset Padding */}
      <div className="flex-1 min-h-0 rounded-2xl border border-[rgba(var(--foreground),0.07)] bg-[rgba(var(--foreground),0.015)] overflow-hidden flex flex-col relative">
        <div
          ref={scrollContainerRef}
          className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-3.5 sm:p-4"
        >
          {modalFilteredProviders.length === 0 ? (
            <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
              <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No providers found</p>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
                No cloud providers match your search query.
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
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3.5">
              {modalFilteredProviders.map((provider) => {
                const isSelected = activeCloudProviderId === provider.id;
                const savedKey = getProviderKey(provider.id);
                const isEditing = editingProviderId === provider.id;

                return (
                  <CloudProviderCard
                    key={provider.id}
                    provider={provider}
                    isSelected={isSelected}
                    savedKey={savedKey}
                    isEditing={isEditing}
                    editingKeyValue={editingKeyValue}
                    setEditingKeyValue={setEditingKeyValue}
                    onSelect={onSelectProvider}
                    onStartInlineEdit={onStartInlineEdit}
                    onCancelInlineKey={onCancelInlineKey}
                    onSaveInlineKey={onSaveInlineKey}
                    onNavigateToModels={onNavigateToModels}
                  />
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
CloudProvidersModalView.displayName = "CloudProvidersModalView";

export function useCloudProvidersModalState(options?: { onNavigateToModels?: (provider: CloudProvider) => void }) {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState<string>("");

  const activeCloudProviderId = useMemo(() => {
    const pName = draftSettings?.llm?.cloud?.provider_name;
    if (!pName) return "custom";
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : "custom";
  }, [draftSettings?.llm?.cloud?.provider_name]);

  const getProviderKey = useCallback(
    (providerId: string) => {
      const keys = draftSettings?.llm?.cloud_keys;
      if (keys && keys[providerId]) return keys[providerId];
      const target = CLOUD_PROVIDERS.find((p) => p.id === providerId);
      if (
        draftSettings?.llm?.cloud?.api_key &&
        target &&
        draftSettings.llm.cloud.provider_name?.toLowerCase() === target.name.toLowerCase()
      ) {
        return draftSettings.llm.cloud.api_key;
      }
      return undefined;
    },
    [draftSettings?.llm?.cloud_keys, draftSettings?.llm?.cloud?.api_key, draftSettings?.llm?.cloud?.provider_name]
  );

  const handleSelectProvider = useCallback(
    (provider: CloudProvider) => {
      const savedKey = getProviderKey(provider.id);
      updateDraft("llm", "cloud", {
        ...draftSettings?.llm?.cloud,
        base_url: provider.url,
        provider_name: provider.name,
        api_key: savedKey ?? null,
      });
    },
    [draftSettings?.llm?.cloud, getProviderKey, updateDraft]
  );

  const handleStartInlineEdit = useCallback((id: string, currentKey: string) => {
    setEditingProviderId(id);
    setEditingKeyValue(currentKey);
  }, []);

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
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

  return {
    activeCloudProviderId,
    getProviderKey,
    onSelectProvider: handleSelectProvider,
    editingProviderId,
    editingKeyValue,
    setEditingKeyValue,
    onStartInlineEdit: handleStartInlineEdit,
    onCancelInlineKey: handleCancelInlineKey,
    onSaveInlineKey: handleSaveInlineKey,
    onNavigateToModels: options?.onNavigateToModels,
  };
}
