import { memo, useState, useMemo, useEffect, useCallback, useDeferredValue } from "react";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { Cpu, Gauge, Heart, Wrench, Zap } from "lucide-react";
import { CLOUD_PROVIDERS } from "@/data/providersCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { useFavoriteModels } from "@/shared/hooks/useFavoriteModels";
import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";
import { REMOTE_MODEL_PAGE_SIZE } from "../models/catalog/catalogProbeUtils";
import { RemoteModelCard } from "../models/catalog/RemoteModelCard";
import { CatalogHeader, CatalogFilterMenu, CatalogFilterItem } from "./CatalogHeader";

export interface ModelModalProps {
  remoteModels: LlmModelInfo[];
  selectedModelId?: string;
  /** Committed (saved) model id driving list order; selection highlight stays on the draft id. */
  pinnedModelId?: string;
  probingMap: Record<string, { status: "idle" | "testing" | "success" | "error"; capabilities?: ModelCapabilities; error?: string }>;
  capabilitiesCache?: Record<string, ModelCapabilities>;
  onSelectModel: (id: string) => void;
  onProbeCapabilities: (id?: string) => void;
}

type ModelCategory = "all" | "favorites" | "benchmarked" | "tools" | "fast";

export const ModelModal = memo(({
  remoteModels, selectedModelId, pinnedModelId, probingMap, capabilitiesCache,
  onSelectModel, onProbeCapabilities,
}: ModelModalProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [filterCategory, setFilterCategory] = useState<ModelCategory>("all");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);

  const { containerRef: scrollContainerRef, lenisRef, scrollToTop } = useLenisScrollContainer<HTMLDivElement>();
  useScrollTopOnChange(scrollContainerRef, selectedModelId || pinnedModelId, lenisRef);

  useEffect(() => {
    scrollToTop();
  }, [deferredSearch, filterCategory, sort, scrollToTop]);

  const handleCopyId = useCallback((id: string) => {
    navigator.clipboard.writeText(id);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  }, []);

  const { favoriteModelIds, toggleFavorite, isFavorite } = useFavoriteModels();

  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const llmServers = useSettingsStore((s) => s.draftSettings?.providers?.llm?.servers);
  const activeMode = llmSettings?.active?.category ?? "cloud";
  const activeProviderName = useMemo(() => {
    if (activeMode === "cloud") return CLOUD_PROVIDERS.find((p) => p.id === llmSettings?.active?.provider)?.name || "Cloud";
    if (activeMode === "server") return llmServers?.find((s) => s.id === llmSettings?.active?.provider)?.name || "Custom Server";
    return "Embedded";
  }, [activeMode, llmSettings?.active?.provider, llmServers]);

  const resolveProbed = useCallback((m: LlmModelInfo) => {
    return probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
  }, [probingMap, capabilitiesCache]);

  const favoritesCount = useMemo(() => remoteModels.filter((m) => favoriteModelIds.has(m.id)).length, [remoteModels, favoriteModelIds]);
  const benchmarkedCount = useMemo(() => remoteModels.filter((m) => (resolveProbed(m)?.tps ?? 0) > 0).length, [remoteModels, resolveProbed]);
  const toolsCount = useMemo(() => remoteModels.filter((m) => resolveProbed(m)?.supports_tools === "supported").length, [remoteModels, resolveProbed]);
  const fastCount = useMemo(() => remoteModels.filter((m) => (resolveProbed(m)?.tps ?? 0) >= 30).length, [remoteModels, resolveProbed]);

  const categoryFilters = useMemo(() => [
    { id: "all" as const, label: "All Models", count: remoteModels.length, Icon: Cpu },
    { id: "favorites" as const, label: "Favorites", count: favoritesCount, Icon: Heart },
    { id: "benchmarked" as const, label: "Benchmarked", count: benchmarkedCount, Icon: Zap },
    { id: "tools" as const, label: "Tools Support", count: toolsCount, Icon: Wrench },
    { id: "fast" as const, label: "Fast (>30 TPS)", count: fastCount, Icon: Gauge },
  ], [remoteModels.length, favoritesCount, benchmarkedCount, toolsCount, fastCount]);

  const modalFilteredModels = useMemo(() => {
    let list = remoteModels;
    const q = deferredSearch.trim().toLowerCase();
    if (q) {
      list = list.filter((m) => m.id.toLowerCase().includes(q) || (m.name && m.name.toLowerCase().includes(q)) || (m.family && m.family.toLowerCase().includes(q)) || (m.quantization && m.quantization.toLowerCase().includes(q)));
    }
    if (filterCategory === "favorites") list = list.filter((m) => favoriteModelIds.has(m.id));
    else if (filterCategory === "benchmarked") list = list.filter((m) => (resolveProbed(m)?.tps ?? 0) > 0);
    else if (filterCategory === "tools") list = list.filter((m) => resolveProbed(m)?.supports_tools === "supported");
    else if (filterCategory === "fast") list = list.filter((m) => (resolveProbed(m)?.tps ?? 0) >= 30);

    const sorted = [...list].sort((a, b) => {
      const an = a.name || a.id, bn = b.name || b.id;
      return sort === "az" ? an.localeCompare(bn, undefined, { sensitivity: "base" }) : bn.localeCompare(an, undefined, { sensitivity: "base" });
    });

    const activeModelId = selectedModelId || pinnedModelId;
    if (activeModelId) {
      const idx = sorted.findIndex((m) => m.id === activeModelId);
      if (idx > 0) {
        const [selected] = sorted.splice(idx, 1);
        sorted.unshift(selected);
      }
    }
    return sorted;
  }, [remoteModels, deferredSearch, filterCategory, resolveProbed, selectedModelId, pinnedModelId, sort, favoriteModelIds]);

  const [visibleModalModelCount, setVisibleModalModelCount] = useState(REMOTE_MODEL_PAGE_SIZE);
  useEffect(() => {
    setVisibleModalModelCount(REMOTE_MODEL_PAGE_SIZE);
  }, [modalSearch, deferredSearch, filterCategory, selectedModelId, pinnedModelId, remoteModels]);

  const visibleModalModels = useMemo(() => modalFilteredModels.slice(0, visibleModalModelCount), [modalFilteredModels, visibleModalModelCount]);
  const modalHiddenCount = modalFilteredModels.length - visibleModalModels.length;

  return (
    <div className="flex flex-col h-full min-h-0 gap-3">
      <CatalogHeader
        icon={<Cpu size={14} className="text-[rgb(var(--accent))]" />}
        title="Remote Models"
        status={
          <span className="text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 flex items-center gap-1.5 ml-1">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse shrink-0" />
            <span className="capitalize">{activeMode} · {activeProviderName}</span>
          </span>
        }
        search={{
          open: isSearchOpen,
          value: modalSearch,
          placeholder: "Search models...",
          onChange: setModalSearch,
          onToggle: () => setIsSearchOpen((v) => !v),
          onClose: () => { setIsSearchOpen(false); setModalSearch(""); },
        }}
        sort={{ order: sort, onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")) }}
        filter={{
          label: "Filter models",
          open: isFilterDropdownOpen,
          activeCount: filterCategory === "all" ? 0 : 1,
          onToggle: () => setIsFilterDropdownOpen((v) => !v),
          onClose: () => setIsFilterDropdownOpen(false),
          menu: (
            <CatalogFilterMenu
              title="Model Filters"
              resetLabel={filterCategory !== "all" ? "Reset" : undefined}
              onReset={() => setFilterCategory("all")}
            >
              {categoryFilters.map((cat) => (
                <CatalogFilterItem
                  key={cat.id}
                  icon={cat.Icon}
                  label={cat.label}
                  count={cat.count}
                  active={filterCategory === cat.id}
                  onSelect={() => { setFilterCategory(cat.id); setIsFilterDropdownOpen(false); }}
                />
              ))}
            </CatalogFilterMenu>
          ),
        }}
      />

      <div ref={scrollContainerRef} className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2">
        <div className="min-h-full w-full">
          {modalFilteredModels.length === 0 ? (
            <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
              <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No models found</p>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">No available models match your current search & filter criteria.</p>
              <button
                type="button"
                onClick={() => { setModalSearch(""); setFilterCategory("all"); }}
                className="mt-2 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
              >
                Reset Filters
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
              {visibleModalModels.map((model) => {
                const isSelected = selectedModelId === model.id;
                const probed = resolveProbed(model);
                const isTesting = probingMap[model.id]?.status === "testing";
                const probeError = probingMap[model.id]?.status === "error" ? probingMap[model.id]?.error : undefined;

                return (
                  <RemoteModelCard
                    key={model.id}
                    model={model}
                    isSelected={isSelected}
                    probed={probed}
                    isTesting={isTesting}
                    probeError={probeError}
                    isFavorite={isFavorite(model.id)}
                    onSelect={onSelectModel}
                    onProbe={onProbeCapabilities}
                    onCopy={handleCopyId}
                    onToggleFavorite={toggleFavorite}
                    isCopied={copiedId === model.id}
                  />
                );
              })}
              {modalHiddenCount > 0 && (
                <div className="col-span-full flex items-center justify-center pt-1">
                  <button
                    type="button"
                    onClick={() => setVisibleModalModelCount((c) => c + REMOTE_MODEL_PAGE_SIZE)}
                    className="px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-colors cursor-pointer"
                  >
                    {`Show ${modalHiddenCount} more`}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

ModelModal.displayName = "ModelModal";
