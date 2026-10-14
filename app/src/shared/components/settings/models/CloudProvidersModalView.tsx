import { memo, useState, useMemo, useCallback, useDeferredValue, useRef, useEffect } from "react";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { CLOUD_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import {
  ArrowDownAZ,
  ArrowUpZA,
  ArrowRight,
  Check,
  Filter,
  Globe,
  Key,
  KeyRound,
  Pencil,
  Search,
  Sparkles,
  X,
} from "lucide-react";
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
        "relative flex flex-col justify-between p-4 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu",
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
  /** Committed (saved) active provider + key lookup driving list order; selection highlight stays on draft ids. */
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
}

export const CloudProvidersModalView = memo(({
  activeCloudProviderId,
  getProviderKey,
  pinnedProviderId,
  getCommittedProviderKey,
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
  const [sort, setSort] = useState<"az" | "za">("az");
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);
  const filterDropdownRef = useRef<HTMLDivElement>(null);

  // Close compact filter dropdown on outside click or Escape
  useEffect(() => {
    if (!isFilterDropdownOpen) return;
    const handleDown = (e: MouseEvent | TouchEvent) => {
      if (filterDropdownRef.current && !filterDropdownRef.current.contains(e.target as Node)) {
        setIsFilterDropdownOpen(false);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsFilterDropdownOpen(false);
    };
    window.addEventListener("mousedown", handleDown);
    window.addEventListener("touchstart", handleDown);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("mousedown", handleDown);
      window.removeEventListener("touchstart", handleDown);
      window.removeEventListener("keydown", handleKey);
    };
  }, [isFilterDropdownOpen]);

  const { containerRef: scrollContainerRef, lenisRef } =
    useLenisScrollContainer<HTMLDivElement>();

  // Save-gated reorder glide: fires when a save changes the committed order.
  const orderFingerprint = useMemo(
    () =>
      `${pinnedProviderId ?? ""}|${CLOUD_PROVIDERS.map((p) =>
        getCommittedProviderKey(p.id)?.trim() ? "1" : "0"
      ).join("")}`,
    [pinnedProviderId, getCommittedProviderKey]
  );
  useScrollTopOnChange(scrollContainerRef, orderFingerprint, lenisRef);

  const configuredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => Boolean(getProviderKey(p.id)?.trim())).length;
  }, [getProviderKey]);

  const popularCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => p.popular).length;
  }, []);

  const unconfiguredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => !getProviderKey(p.id)?.trim()).length;
  }, [getProviderKey]);

  const providerFilters = useMemo(
    () => [
      { id: "all" as const, label: "All Providers", count: CLOUD_PROVIDERS.length, Icon: Globe },
      { id: "configured" as const, label: "Configured", count: configuredCount, Icon: Key },
      { id: "popular" as const, label: "Popular", count: popularCount, Icon: Sparkles },
      { id: "unconfigured" as const, label: "Needs Key", count: unconfiguredCount, Icon: KeyRound },
    ],
    [configuredCount, popularCount, unconfiguredCount]
  );

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
      const aKey = Boolean(getCommittedProviderKey(a.id)?.trim());
      const bKey = Boolean(getCommittedProviderKey(b.id)?.trim());
      if (aKey && !bKey) return -1;
      if (!aKey && bKey) return 1;
      return sort === "az"
        ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
        : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
    });
  }, [deferredSearch, filterCategory, getProviderKey, getCommittedProviderKey, sort]);

  return (
    <div className="flex flex-col h-full min-h-0 gap-3.5">
      {/* Top Toolbar: Search + Filter Dropdown (compact) + Sort Icon + Count */}
      <div className="flex items-center justify-between gap-2.5 shrink-0 flex-wrap">
        {/* Mobile/Compact Filter Dropdown Trigger */}
        <div ref={filterDropdownRef} className="relative md:hidden shrink-0">
          <button
            type="button"
            onClick={() => setIsFilterDropdownOpen((v) => !v)}
            className={cn(
              "p-2 rounded-xl border transition-all cursor-pointer flex items-center gap-1.5",
              isFilterDropdownOpen || filterCategory !== "all"
                ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.35)] shadow-xs"
                : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))]"
            )}
            aria-label="Toggle provider filters"
            aria-expanded={isFilterDropdownOpen}
          >
            <Filter size={15} />
            {filterCategory !== "all" && (
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
            )}
          </button>

          {/* Compact Filter Menu Popover */}
          {isFilterDropdownOpen && (
            <div className="absolute left-0 top-full mt-2 z-50 w-64 rounded-xl bg-[rgba(var(--card),0.96)] border border-[rgba(var(--foreground),0.12)] shadow-2xl p-3 flex flex-col gap-1.5 animate-fade-in">
              <div className="flex items-center justify-between pb-1.5 border-b border-[rgba(var(--foreground),0.06)] px-1">
                <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">Filters</span>
                {filterCategory !== "all" && (
                  <button
                    type="button"
                    onClick={() => {
                      setFilterCategory("all");
                      setIsFilterDropdownOpen(false);
                    }}
                    className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    Reset
                  </button>
                )}
              </div>
              {providerFilters.map((cat) => (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => {
                    setFilterCategory(cat.id);
                    setIsFilterDropdownOpen(false);
                  }}
                  className={cn(
                    "w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl text-[12px] transition-all cursor-pointer font-medium",
                    filterCategory === cat.id
                      ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  <div className="flex items-center gap-2 truncate">
                    <cat.Icon size={13} className={cn("shrink-0", filterCategory === cat.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                    <span className="truncate">{cat.label}</span>
                  </div>
                  <span className={cn(
                    "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                    filterCategory === cat.id
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                      : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
                  )}>
                    {cat.count}
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Search Input */}
        <div className="flex-1 min-w-[140px] sm:min-w-[180px] flex items-center gap-2 px-3.5 py-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
          <Search size={14} className="text-[rgb(var(--foreground-muted))] shrink-0" />
          <input
            type="text"
            value={modalSearch}
            onChange={(e) => setModalSearch(e.target.value)}
            placeholder="Search providers..."
            aria-label="Search providers"
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

        {/* Sort Button */}
        <button
          type="button"
          onClick={() => setSort((s) => (s === "az" ? "za" : "az"))}
          className="p-2 rounded-xl text-[12px] font-medium bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] hover:border-[rgb(var(--accent))]/40 hover:text-[rgb(var(--accent))] flex items-center justify-center cursor-pointer shrink-0 transition-all"
          title={sort === "az" ? "Sort: A–Z" : "Sort: Z–A"}
          aria-label="Toggle alphabetical sort order"
        >
          {sort === "az" ? (
            <ArrowDownAZ size={15} className="text-[rgb(var(--accent))]" />
          ) : (
            <ArrowUpZA size={15} className="text-[rgb(var(--accent))]" />
          )}
        </button>

        {/* Count Pill */}
        <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0 select-none">
          <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredProviders.length}</span> of {CLOUD_PROVIDERS.length} providers</span>
          <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredProviders.length}</span>/{CLOUD_PROVIDERS.length}</span>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 flex gap-4 overflow-hidden">
        {/* Facet Sidebar on Desktop */}
        <div className="w-[190px] shrink-0 hidden md:flex flex-col gap-1 overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5">
          {providerFilters.map((cat) => (
            <button
              key={cat.id}
              type="button"
              onClick={() => setFilterCategory(cat.id)}
              className={cn(
                "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[12.5px] transition-all cursor-pointer font-medium",
                filterCategory === cat.id
                  ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
              )}
            >
              <div className="flex items-center gap-2.5 truncate">
                <cat.Icon size={14} className={cn("shrink-0", filterCategory === cat.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                <span className="truncate">{cat.label}</span>
              </div>
              <span className={cn(
                "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                filterCategory === cat.id
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
              )}>
                {cat.count}
              </span>
            </button>
          ))}
        </div>
        <div
          ref={scrollContainerRef}
          className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2"
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
            <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
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
  const committedSettings = useSettingsStore((s) => s.settings);
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

  // Save-gated ordering: the sort follows committed keys so selecting only
  // re-highlights; the reorder (with smooth scroll) lands on save.
  const pinnedProviderId = useMemo(() => {
    const pName = committedSettings?.llm?.cloud?.provider_name;
    if (!pName) return undefined;
    const found = CLOUD_PROVIDERS.find((p) => p.name.toLowerCase() === pName.toLowerCase());
    return found ? found.id : undefined;
  }, [committedSettings?.llm?.cloud?.provider_name]);

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
    [committedSettings?.llm?.cloud_keys, committedSettings?.llm?.cloud?.api_key, committedSettings?.llm?.cloud?.provider_name]
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
    pinnedProviderId,
    getCommittedProviderKey,
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
