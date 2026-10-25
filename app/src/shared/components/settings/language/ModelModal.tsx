import { memo, useState, useMemo, useEffect, useCallback, useDeferredValue, useRef } from "react";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { promoteActive } from "@/shared/lib/activeSelection";
import { resolveCapabilities } from "@/shared/lib/catalogCapabilities";
import { filterModels } from "@/shared/lib/catalogFilter";
import { copyToClipboard } from "@/shared/lib/clipboard";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";
import { Cpu, Gauge, Heart, Wrench, Zap } from "lucide-react";
import { EmptyState } from "@/shared/components/common";
import { useFavoriteModels } from "@/shared/hooks/useFavoriteModels";
import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";
import { RemoteModelCard } from "./RemoteModelCard";
import { CardFilterMenu, CardFilterItem } from "../shared/CardHeader";
import { CatalogModalToolbar } from "../shared/CatalogModalToolbar";
import { CatalogModalShell } from "../shared/CatalogModalShell";
import { CatalogFacetRail, type CatalogFacetGroup } from "../shared/CatalogFacetRail";

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
  const [filterCategory, setFilterCategory] = useState<ModelCategory>("all");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);

  const { containerRef: scrollContainerRef, lenisRef, scrollToTop } = useLenisScrollContainer<HTMLDivElement>();
  useScrollTopOnChange(scrollContainerRef, selectedModelId || pinnedModelId, lenisRef);

  useEffect(() => {
    scrollToTop();
  }, [deferredSearch, filterCategory, sort, scrollToTop]);

  const copyTimer = useRef<number | null>(null);
  useEffect(() => () => {
    if (copyTimer.current) window.clearTimeout(copyTimer.current);
  }, []);

  const handleCopyId = useCallback((id: string) => {
    void copyToClipboard(id).then((ok) => {
      if (!ok) return;
      setCopiedId(id);
      if (copyTimer.current) window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopiedId(null), 1800);
    });
  }, []);

  const { favoriteModelIds, toggleFavorite, isFavorite } = useFavoriteModels();

  const resolveProbed = useCallback((m: LlmModelInfo) => {
    return resolveCapabilities(m, probingMap, capabilitiesCache);
  }, [probingMap, capabilitiesCache]);

  const { favoritesCount, benchmarkedCount, toolsCount, fastCount } = useMemo(() => {
    let favorites = 0;
    let benchmarked = 0;
    let tools = 0;
    let fast = 0;
    for (const m of remoteModels) {
      if (favoriteModelIds.has(m.id)) favorites += 1;
      const probed = resolveProbed(m);
      if (probed?.tps != null && probed.tps > 0) benchmarked += 1;
      if (probed?.supports_tools === "supported") tools += 1;
      if (probed?.tps != null && probed.tps >= 30) fast += 1;
    }
    return {
      favoritesCount: favorites,
      benchmarkedCount: benchmarked,
      toolsCount: tools,
      fastCount: fast,
    };
  }, [remoteModels, favoriteModelIds, resolveProbed]);

  const categoryFilters = useMemo(() => [
    { id: "all" as const, label: LLM_CATALOG_COPY.filterAll, count: remoteModels.length, Icon: Cpu },
    { id: "favorites" as const, label: LLM_CATALOG_COPY.filterFavorites, count: favoritesCount, Icon: Heart },
    { id: "benchmarked" as const, label: LLM_CATALOG_COPY.filterBenchmarked, count: benchmarkedCount, Icon: Zap },
    { id: "tools" as const, label: LLM_CATALOG_COPY.filterTools, count: toolsCount, Icon: Wrench },
    { id: "fast" as const, label: LLM_CATALOG_COPY.filterFast, count: fastCount, Icon: Gauge },
  ], [remoteModels.length, favoritesCount, benchmarkedCount, toolsCount, fastCount]);

  const facetGroups = useMemo<CatalogFacetGroup[]>(() => [
    {
      title: LLM_CATALOG_COPY.filtersTitle,
      items: categoryFilters,
    },
  ], [categoryFilters]);

  const modalFilteredModels = useMemo(() => {
    const filtered = filterModels(remoteModels, {
      category: filterCategory,
      favoriteIds: favoriteModelIds,
      resolveProbed,
      fuzzy: true,
    }, deferredSearch);

    return promoteActive(
      [...filtered].sort((a, b) => {
        const an = a.name || a.id, bn = b.name || b.id;
        return sort === "az" ? an.localeCompare(bn, undefined, { sensitivity: "base" }) : bn.localeCompare(an, undefined, { sensitivity: "base" });
      }),
      pinnedModelId
    );
  }, [remoteModels, deferredSearch, filterCategory, resolveProbed, pinnedModelId, sort, favoriteModelIds]);

  const toolbarNode = (
    <CatalogModalToolbar
      search={{
        value: modalSearch,
        placeholder: LLM_CATALOG_COPY.modalSearchPlaceholder,
        onChange: setModalSearch,
      }}
      sort={{
        order: sort,
        onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
      }}
      itemCount={{
        current: modalFilteredModels.length,
        total: remoteModels.length,
        entityName: "models",
      }}
      filter={{
        label: "Filter models",
        open: isFilterDropdownOpen,
        activeCount: filterCategory === "all" ? 0 : 1,
        onToggle: () => setIsFilterDropdownOpen((v) => !v),
        onClose: () => setIsFilterDropdownOpen(false),
        menu: (
          <CardFilterMenu
            title={LLM_CATALOG_COPY.filterTitle}
            resetLabel={filterCategory !== "all" ? LLM_CATALOG_COPY.filterReset : undefined}
            onReset={() => setFilterCategory("all")}
          >
            {categoryFilters.map((cat) => (
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
    <EmptyState
      icon={Cpu}
      title={LLM_CATALOG_COPY.modalEmptyTitle}
      description={LLM_CATALOG_COPY.modalEmptyHint}
      action={
        <button
          type="button"
          onClick={() => {
            setModalSearch("");
            setFilterCategory("all");
          }}
          className="px-3.5 py-1.5 rounded-xl text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
        >
          {LLM_CATALOG_COPY.resetFilters}
        </button>
      }
      className="py-12 border-dashed border-[rgba(var(--accent),0.15)] bg-transparent"
    />
  );

  return (
    <CatalogModalShell
      toolbar={toolbarNode}
      facetRail={
        <CatalogFacetRail
          groups={facetGroups}
          selectedId={filterCategory}
          onSelect={(id) => setFilterCategory(id as ModelCategory)}
        />
      }
      scrollRef={scrollContainerRef}
      isEmpty={modalFilteredModels.length === 0}
      emptyState={emptyStateNode}
    >
      <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
        {modalFilteredModels.map((model) => {
          const isSelected = selectedModelId === model.id;
          const probed = resolveProbed(model);
          const probeState = probingMap[model.id];
          const isTesting = probeState?.status === "testing";
          const probeError = probeState?.status === "error" ? probeState.error : undefined;

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
      </div>
    </CatalogModalShell>
  );
});

ModelModal.displayName = "ModelModal";
