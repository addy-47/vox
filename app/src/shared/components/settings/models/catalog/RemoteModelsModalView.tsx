import {
  memo,
  useState,
  useMemo,
  useEffect,
  useCallback,
  useDeferredValue,
  useRef,
} from "react";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import {
  ArrowDownAZ,
  ArrowUpZA,
  Cpu,
  Filter,
  Gauge,
  Heart,
  Search,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { useFavoriteModels } from "@/shared/hooks/useFavoriteModels";
import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";
import { REMOTE_MODEL_PAGE_SIZE } from "./catalogProbeUtils";
import { RemoteModelCard } from "./RemoteModelCard";

export interface RemoteModelsModalViewProps {
  remoteModels: LlmModelInfo[];
  selectedModelId?: string;
  /** Committed (saved) model id driving list order; selection highlight stays on the draft id. */
  pinnedModelId?: string;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  capabilitiesCache?: Record<string, ModelCapabilities>;
  onSelectModel: (id: string) => void;
  onProbeCapabilities: (id?: string) => void;
}

type ModelCategory = "all" | "favorites" | "benchmarked" | "tools" | "fast";

export const RemoteModelsModalView = memo(({
  remoteModels,
  selectedModelId,
  pinnedModelId,
  probingMap,
  capabilitiesCache,
  onSelectModel,
  onProbeCapabilities,
}: RemoteModelsModalViewProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [filterCategory, setFilterCategory] = useState<ModelCategory>("all");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [copiedId, setCopiedId] = useState<string | null>(null);
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

  // Canonical smooth scroll (SSOT hook — inertial wheel glide, native touch
  // momentum). The inner content wrapper is the Lenis content element.
  const { containerRef: scrollContainerRef, lenisRef, scrollToTop } =
    useLenisScrollContainer<HTMLDivElement>();

  useScrollTopOnChange(scrollContainerRef, pinnedModelId, lenisRef);

  useEffect(() => {
    scrollToTop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deferredSearch, filterCategory, sort]);

  const handleCopyId = useCallback((id: string) => {
    navigator.clipboard.writeText(id);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  }, []);

  const { favoriteModelIds, toggleFavorite, isFavorite } = useFavoriteModels();

  // Active LLM provider configuration for the bottom-left corner indicator
  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const activeMode = llmSettings?.active ?? "cloud";
  const activeProviderName = useMemo(() => {
    if (activeMode === "cloud") {
      return llmSettings?.cloud?.provider_name || "Cloud";
    }
    if (activeMode === "server") {
      return llmSettings?.server?.provider_name || "Custom Server";
    }
    return "Embedded";
  }, [activeMode, llmSettings]);

  const activeEndpointLabel = useMemo(() => {
    if (activeMode === "cloud") return "Remote API";
    if (activeMode === "server") {
      try {
        const url = new URL(llmSettings?.server?.base_url || "http://localhost:11434");
        return url.host || "Localhost";
      } catch {
        return llmSettings?.server?.base_url || "Remote Server";
      }
    }
    return "On-Device GGUF";
  }, [activeMode, llmSettings]);

  const favoritesCount = useMemo(() => {
    return remoteModels.filter((m) => favoriteModelIds.has(m.id)).length;
  }, [remoteModels, favoriteModelIds]);

  // Compute metrics counts for filter badges
  const benchmarkedCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.tps && probed.tps > 0;
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const toolsCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.supports_tools === "supported";
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const fastCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.tps && probed.tps >= 30;
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const categoryFilters = useMemo(
    () => [
      { id: "all" as const, label: "All Models", count: remoteModels.length, Icon: Cpu },
      { id: "favorites" as const, label: "Favorites", count: favoritesCount, Icon: Heart },
      { id: "benchmarked" as const, label: "Benchmarked", count: benchmarkedCount, Icon: Zap },
      { id: "tools" as const, label: "Tools Support", count: toolsCount, Icon: Wrench },
      { id: "fast" as const, label: "Fast (>30 TPS)", count: fastCount, Icon: Gauge },
    ],
    [remoteModels.length, favoritesCount, benchmarkedCount, toolsCount, fastCount]
  );

  const modalFilteredModels = useMemo(() => {
    let list = remoteModels;
    const query = deferredSearch.trim().toLowerCase();
    if (query) {
      list = list.filter((m) =>
        m.id.toLowerCase().includes(query) ||
        (m.name && m.name.toLowerCase().includes(query)) ||
        (m.family && m.family.toLowerCase().includes(query)) ||
        (m.quantization && m.quantization.toLowerCase().includes(query))
      );
    }

    if (filterCategory === "favorites") {
      list = list.filter((m) => favoriteModelIds.has(m.id));
    } else if (filterCategory === "benchmarked") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.tps && probed.tps > 0;
      });
    } else if (filterCategory === "tools") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.supports_tools === "supported";
      });
    } else if (filterCategory === "fast") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.tps && probed.tps >= 30;
      });
    }

    const sorted = [...list];
    if (sort === "az") {
      sorted.sort((a, b) => (a.name || a.id).localeCompare(b.name || b.id, undefined, { sensitivity: "base" }));
    } else {
      sorted.sort((a, b) => (b.name || b.id).localeCompare(a.name || a.id, undefined, { sensitivity: "base" }));
    }

    if (pinnedModelId) {
      const idx = sorted.findIndex((m) => m.id === pinnedModelId);
      if (idx > 0) {
        const [selected] = sorted.splice(idx, 1);
        sorted.unshift(selected);
      }
    }

    return sorted;
  }, [remoteModels, deferredSearch, filterCategory, probingMap, capabilitiesCache, pinnedModelId, sort, favoriteModelIds]);

  // Mounted window over the filtered list
  const [visibleModalModelCount, setVisibleModalModelCount] = useState(REMOTE_MODEL_PAGE_SIZE);
  useEffect(() => {
    setVisibleModalModelCount(REMOTE_MODEL_PAGE_SIZE);
  }, [modalSearch, deferredSearch, filterCategory, remoteModels]);

  const visibleModalModels = useMemo(
    () => modalFilteredModels.slice(0, visibleModalModelCount),
    [modalFilteredModels, visibleModalModelCount]
  );
  const modalHiddenCount = modalFilteredModels.length - visibleModalModels.length;

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
            aria-label="Toggle model filters"
            aria-expanded={isFilterDropdownOpen}
          >
            <Filter size={15} />
            {filterCategory !== "all" && (
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
            )}
          </button>

          {/* Compact Filter Menu Popover */}
          {isFilterDropdownOpen && (
            <div className="absolute left-0 top-full mt-2 z-50 w-64 rounded-xl bg-[rgba(var(--card),0.96)] backdrop-blur-xl border border-[rgba(var(--foreground),0.12)] shadow-2xl p-3 flex flex-col gap-1.5 animate-fade-in">
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
              {categoryFilters.map((cat) => (
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
            placeholder="Search models..."
            aria-label="Search models"
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
          <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredModels.length}</span> of {remoteModels.length} models</span>
          <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredModels.length}</span>/{remoteModels.length}</span>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 flex gap-4 overflow-hidden">
        {/* Facet Sidebar on Desktop with Active Connector Pin on Bottom Left */}
        <div className="w-[190px] shrink-0 hidden md:flex flex-col justify-between h-full overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5">
          <div className="flex flex-col gap-1">
            {categoryFilters.map((cat) => (
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

          {/* Bottom-Left Connector Indicator */}
          <div className="mt-4 pt-3 border-t border-[rgba(var(--foreground),0.07)] flex flex-col gap-1.5 px-1 select-none">
            <div className="flex items-center gap-2 p-2]">
              <div className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] animate-pulse shrink-0" />
              <div className="flex flex-col min-w-0">
                <span className="text-[11.5px] font-bold text-[rgb(var(--foreground))] capitalize truncate">
                  {activeMode} · {activeProviderName}
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
                  {activeEndpointLabel}
                </span>
              </div>
            </div>
          </div>
        </div>
        <div
          ref={scrollContainerRef}
          className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2"
        >
          {/* Stable Lenis content wrapper: keeps virtual height measurement
              correct across filter/empty-state swaps. */}
          <div className="min-h-full w-full">
          {modalFilteredModels.length === 0 ? (
            <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
              <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No models found</p>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
                No available models match your current search & filter criteria.
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
              {visibleModalModels.map((model) => {
                const isSelected = selectedModelId === model.id;
                const probed =
                  probingMap[model.id]?.capabilities ||
                  model.capabilities ||
                  capabilitiesCache?.[`server:${model.id}`] ||
                  capabilitiesCache?.[`cloud:${model.id}`] ||
                  capabilitiesCache?.[`embedded:${model.id}`] ||
                  capabilitiesCache?.[model.id];
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
                    onClick={() =>
                      setVisibleModalModelCount((c) => c + REMOTE_MODEL_PAGE_SIZE)
                    }
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
    </div>
  );
});

RemoteModelsModalView.displayName = "RemoteModelsModalView";
