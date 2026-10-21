import {
  memo,
  useState,
  useMemo,
  useCallback,
  useRef,
} from "react";
import {
  ServerCog,
  Cloud,
  AlertCircle,
  AlertTriangle,
  Loader2,
  Sparkles,
  Cpu,
  Gauge,
  Heart,
  Wrench,
  Zap,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { resolveCapabilities as resolveModelCapabilities } from "@/shared/lib/catalogCapabilities";
import { filterModels } from "@/shared/lib/catalogFilter";
import { useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { useFavoriteModels } from "@/shared/hooks/useFavoriteModels";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";
import {
  useSettingsStore,
  type LlmModelInfo,
  type ModelCapabilities,
  type LlmProviderConfig,
} from "@/store/settingsStore";
import { useShallow } from "zustand/react/shallow";
import { useCatalogStore } from "@/store/catalogStore";
import { ProviderModal } from "./ProviderModal";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import { ModelModal } from "./ModelModal";
import {
  CardFilterItem,
  CardFilterMenu,
  CardHeader,
  type CardSortOrder,
} from "../shared/CardHeader";
import { CatalogReveal } from "../shared/CatalogReveal";
import { LlmTuningControls } from "./LlmTuningControls";
import {
  provenanceLabel,
  hasProbeWarning,
  probeWarningLabel,
} from "./catalogProbeUtils";
import { getSettingsInlineListMaxHeightClass } from "../shared/settingsLayout";

export interface RemoteCatalogSectionProps {
  layoutMode?: "full-max" | "full-min" | "small";
  provider?: LlmProviderConfig;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  handleProbeCapabilities: (id?: string) => void;
  /** Returns to the provider picker when the section is hosted inside a card. */
  onBack?: () => void;
}

export const RemoteCatalogSection = memo(({
  layoutMode,
  provider,
  remoteModels,
  loadingRemoteModels,
  remoteModelsError,
  probingMap,
  handleProbeCapabilities,
  onBack,
}: RemoteCatalogSectionProps) => {
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const capabilitiesCache = useCatalogStore((s) => s.capabilitiesCache);
  const cloudProviders = useCatalogStore((s) => s.cloudProviders);
  const committedSettings = useSettingsStore(useShallow((s) => s.settings));

  // Modal View: "models" | "providers"
  const [catalogModalView, setCatalogModalView] = useState<"models" | "providers">("models");
  // The providers view lists *cloud* providers. For embedded/server there is
  // nothing to navigate to, so the breadcrumb stays hidden and the modal is
  // always the models catalog.
  const showProvidersView = provider?.kind === "cloud";
  const activeModalView = showProvidersView ? catalogModalView : "models";
  const cloudProvidersModalProps = useProviderSelection({
    onNavigateToModels: () => setCatalogModalView("models"),
  });

  // Search State for Remote Catalog
  const [searchQuery, setSearchQuery] = useState("");
  const [sort, setSort] = useState<CardSortOrder>("az");
  const [modelFilter, setModelFilter] = useState<
    "all" | "favorites" | "benchmarked" | "tools" | "fast"
  >("all");
  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const { favoriteModelIds } = useFavoriteModels();

  /** Resolved probe capabilities for a model (live probe → model payload →
   * capability cache). Single source for the filter predicates below. */
  const resolveCapabilities = useCallback(
    (model: LlmModelInfo): ModelCapabilities | undefined =>
      resolveModelCapabilities(model, probingMap, capabilitiesCache),
    [probingMap, capabilitiesCache]
  );

  const resetModelFilter = useCallback(() => setModelFilter("all"), []);

  // Facet counts for the header filter popover (mirrors the expand modal).
  const modelFilterCounts = useMemo(() => {
    let favorites = 0;
    let benchmarked = 0;
    let tools = 0;
    let fast = 0;
    for (const model of remoteModels) {
      if (favoriteModelIds.has(model.id)) favorites += 1;
      const probed = resolveCapabilities(model);
      if (probed?.tps != null && probed.tps > 0) benchmarked += 1;
      if (probed?.supports_tools === "supported") tools += 1;
      if (probed?.tps != null && probed.tps >= 30) fast += 1;
    }
    return {
      all: remoteModels.length,
      favorites,
      benchmarked,
      tools,
      fast,
    };
  }, [remoteModels, favoriteModelIds, resolveCapabilities]);

  const selectedModelId = provider && "model" in provider ? provider.model : undefined;

  // Save-gated pinning: list order follows the COMMITTED model, so selecting
  // only re-highlights. The reorder (with smooth scroll) lands on save.
  const committedRemoteModelId = useMemo(() => {
    return committedSettings?.llm?.active?.model || undefined;
  }, [committedSettings?.llm?.active?.model]);

  // Filtered Remote Models with fzf-style fuzzy matching, active facets, and
  // name order. The committed model stays pinned to the top.
  const filteredRemoteModels = useMemo(() => {
    const models = filterModels(remoteModels, {
      category: modelFilter,
      favoriteIds: favoriteModelIds,
      resolveProbed: resolveCapabilities,
      fuzzy: true,
    }, searchQuery);

    // The sort is the list order. It used to be computed and thrown away,
    // which left the header toggle inert.
    const sorted = [...models];
    sorted.sort((a, b) => {
      const an = a.name || a.id;
      const bn = b.name || b.id;
      const cmp = an.localeCompare(bn, undefined, { sensitivity: "base" });
      return sort === "az" ? cmp : -cmp;
    });

    // Active card primacy: only the committed model pins to the top, and it
    // pins inside the sorted order.
    const activeModelId = committedRemoteModelId;
    if (activeModelId) {
      const selectedIndex = sorted.findIndex((m) => m.id === activeModelId);
      if (selectedIndex > 0) {
        const [selected] = sorted.splice(selectedIndex, 1);
        sorted.unshift(selected);
      }
    }

    return sorted;
  }, [
    remoteModels,
    searchQuery,
    modelFilter,
    sort,
    favoriteModelIds,
    resolveCapabilities,
    committedRemoteModelId,
  ]);

  const [isSearchOpen, setIsSearchOpen] = useState(false);

  const handleSelectModel = useCallback((modelId: string) => {
    const active = useSettingsStore.getState().draftSettings?.llm?.active;
    if (active && (active.category === "server" || active.category === "cloud")) {
      updateDraft("llm", "active", { ...active, model: modelId });
    }
  }, [updateDraft]);

  // Native scrolling. One Lenis instance per mounted catalog added wheel
  // interception to a list this short, and the card already sits above a
  // backdrop-blurred surface; native scroll keeps the frame budget flat.
  const inlineRowsRef = useRef<HTMLDivElement | null>(null);
  useScrollTopOnChange(inlineRowsRef, selectedModelId || committedRemoteModelId);

  const RemoteModelRows = () => (
    <CatalogReveal
      className="flex-1 min-h-0"
      scrollRef={inlineRowsRef}
      featherHeight={80}
      scrollerClassName="h-full overflow-y-auto pr-1 custom-scrollbar [scrollbar-gutter:stable]"
    >
      <div
        className={cn(
          "grid auto-rows-max content-start gap-2 w-full",
          layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
        )}
      >
      {remoteModels.length === 0 && loadingRemoteModels ? (
        <div className="col-span-full flex flex-col gap-2" aria-hidden="true">
          {Array.from({ length: 4 }).map((_, idx) => (
            <div
              key={`model-row-skeleton-${idx}`}
              className="flex min-h-[64px] w-full animate-pulse items-center gap-2.5 rounded-xl border border-[rgba(var(--foreground),0.07)] bg-[rgba(var(--foreground),0.035)] py-2.5 pl-2.5 pr-2.5"
            >
              <div className="h-9 w-9 shrink-0 rounded-xl bg-[rgba(var(--foreground),0.06)]" />
              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <div className="h-3 w-32 rounded bg-[rgba(var(--foreground),0.07)]" />
                <div className="h-2.5 w-20 rounded bg-[rgba(var(--foreground),0.04)]" />
              </div>
            </div>
          ))}
        </div>
      ) : remoteModels.length === 0 ? (
        <div className="col-span-full text-center py-8 text-[12px] text-[rgb(var(--foreground-muted))]/70 space-y-1">
          <p className="font-semibold text-[rgb(var(--foreground))]/80">{LLM_CATALOG_COPY.emptyTitle}</p>
          <p className="text-[11px]">{LLM_CATALOG_COPY.emptyHint}</p>
        </div>
      ) : filteredRemoteModels.length === 0 ? (
        <div className="col-span-full text-center py-8 text-[12px] text-[rgb(var(--foreground-muted))]/70 space-y-2">
          <p className="font-semibold text-[rgb(var(--foreground))]/80">{LLM_CATALOG_COPY.noModelsMatch} &ldquo;{searchQuery}&rdquo;</p>
          <button
            type="button"
            onClick={() => setSearchQuery("")}
            className="px-3 py-1 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
          >
            {LLM_CATALOG_COPY.clearSearch}
          </button>
        </div>
      ) : (
        filteredRemoteModels.map((model) => {
          const isSelected = selectedModelId === model.id;
          const probed = resolveCapabilities(model);
          const isTesting = probingMap[model.id]?.status === "testing";
          const inlineProbeError = probingMap[model.id]?.status === "error" ? probingMap[model.id]?.error : undefined;
          const isGpu = probed?.is_gpu_accelerated;

          // Check if name is essentially a duplicate of the raw ID
          const isIdDuplicateOfName =
            !model.name ||
            model.name.toLowerCase().replace(/[\s\-_/]/g, "") ===
              model.id.toLowerCase().replace(/[\s\-_/]/g, "");

          // Format clean short name and org prefix
          let org = "";
          let shortName = model.name || model.id;
          if (model.id.includes("/")) {
            const parts = model.id.split("/");
            org = parts[0];
            shortName = parts.slice(1).join("/");
          }

          return (
            <div
              key={model.id}
              role="button"
              tabIndex={0}
              onClick={() => handleSelectModel(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleSelectModel(model.id);
                }
              }}
              className={cn(
                "group w-full text-left p-3 rounded-xl border transition-all duration-200 relative shrink-0 cursor-pointer min-h-[64px] flex flex-col justify-between hover:z-20",
                isSelected
                  ? "bg-[rgba(var(--card),0.5)] border-[rgb(var(--accent))] shadow-[0_0_16px_rgba(var(--accent),0.16)] ring-1 ring-[rgb(var(--accent))]/40"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.03)]",
                isGpu && !isSelected ? "border-[rgba(var(--notif-models),0.35)]" : ""
              )}
            >
              {/* Top Row: Title + Quantization + Reset Icon on Top Right */}
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    {isIdDuplicateOfName ? (
                      <span className="font-bold text-[13.5px] leading-snug tracking-tight truncate font-sans">
                        {org && (
                          <span className="text-[rgb(var(--foreground-muted))]/75 font-mono text-[12px] font-normal mr-0.5">
                            {org}/
                          </span>
                        )}
                        <span className={isSelected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"}>
                          {shortName}
                        </span>
                      </span>
                    ) : (
                      <span className={cn(
                        "font-bold text-[13.5px] leading-snug tracking-tight truncate font-sans",
                        isSelected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                      )}>
                        {model.name}
                      </span>
                    )}

                    {model.quantization && (
                      <span className="text-[10.5px] font-bold font-mono px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.05)] leading-none">
                        {model.quantization}
                      </span>
                    )}

                    {hasProbeWarning(inlineProbeError, probed) && (
                      <Tooltip
                        side="top"
                        align="start"
                        className="p-3 w-[300px] max-w-[340px] whitespace-normal text-left border border-[rgba(var(--foreground),0.14)] bg-[rgb(var(--card))]/98 shadow-lg"
                        label={probeWarningLabel(inlineProbeError, probed)}
                      >
                        <span
                          className="inline-flex items-center shrink-0 cursor-help p-1 -m-1"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <AlertTriangle size={12} className="text-[rgb(var(--warn-soft))]" />
                        </span>
                      </Tooltip>
                    )}
                  </div>

                  {/* Optional Subtitle */}
                  {(!isIdDuplicateOfName || (model.size_bytes !== null && model.size_bytes !== undefined)) && (
                    <div className="flex items-center gap-2 text-[11.5px] text-[rgb(var(--foreground-muted))] mt-1">
                      {!isIdDuplicateOfName && (
                        <span className="font-mono truncate select-all">{model.id}</span>
                      )}
                      {model.size_bytes !== null && model.size_bytes !== undefined && (
                        <>
                          {!isIdDuplicateOfName && <span className="opacity-40">•</span>}
                          <span className="font-mono shrink-0">{(model.size_bytes / (1024 * 1024 * 1024)).toFixed(1)} GB</span>
                        </>
                      )}
                    </div>
                  )}
                </div>

                {/* Right Top: Family Badge */}
                <div className="flex items-center gap-1.5 shrink-0">
                  {model.family && (
                    <span className="text-[10.5px] font-bold px-2 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.15)] leading-none">
                      {model.family}
                    </span>
                  )}
                </div>
              </div>

              {/* Bottom Row: Unified Capabilities Trigger (Left) & Benchmark / Re-probe Action (Right) */}
              <div className="flex items-center justify-between gap-2 mt-2 pt-2 border-t border-[rgba(var(--foreground),0.04)] text-[11px]">
                {/* Left: Unified Capabilities with Info Tooltip on Hover */}
                <div className="flex items-center gap-1 min-w-0">
                  {isTesting ? (
                    <span className="font-bold text-[rgb(var(--accent))] flex items-center gap-1.5 py-0.5">
                      <Loader2 size={12} className="animate-spin shrink-0" />
                      <span className="truncate">{LLM_CATALOG_COPY.benchmarking}</span>
                    </span>
                  ) : probed ? (
                    <Tooltip
                      side="top"
                      align="start"
                      mobileTapEnabled
                      className="p-3 w-[210px] whitespace-normal text-left z-50 border border-[rgba(var(--foreground),0.14)] bg-[rgb(var(--card))]/98 shadow-lg"
                      label={
                        <div className="space-y-1.5 text-[11px] font-sans w-full">
                          <div className="font-bold text-[rgb(var(--foreground))] border-b border-[rgba(var(--foreground),0.08)] pb-1 flex items-center justify-between gap-2">
                            <span>{LLM_CATALOG_COPY.modelCapabilities}</span>
                            {isGpu ? (
                              <span className="text-[rgb(var(--notif-models))] font-mono text-[10px] font-bold shrink-0">{LLM_CATALOG_COPY.gpuBadge}</span>
                            ) : probed?.server_has_gpu ? (
                              <span className="text-[rgb(var(--warn-soft))] font-mono text-[10px] font-bold shrink-0">{LLM_CATALOG_COPY.cpuBadge}</span>
                            ) : null}
                          </div>
                          <div className="space-y-1 font-mono text-[10.5px] w-full">
                            {probed.tps != null && probed.tps > 0 && (
                              <div className="flex items-center justify-between gap-3">
                                <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.speed}</span>
                                <span className="text-[rgb(var(--notif-models))] font-bold shrink-0">⚡ {probed.tps.toFixed(1)} tps</span>
                              </div>
                            )}
                            <div className="flex items-center justify-between gap-3">
                              <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.context}</span>
                              <span className="text-[rgb(var(--foreground))] shrink-0">
                                {probed.context_window
                                  ? probed.context_window >= 1000000
                                    ? `${(probed.context_window / 1000000).toFixed(1)}M tokens`
                                    : `${Math.round(probed.context_window / 1024)}k tokens`
                                  : LLM_CATALOG_COPY.managed}
                              </span>
                            </div>
                            {probed.vram_bytes ? (
                              <div className="flex items-center justify-between gap-3">
                                <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.vram}</span>
                                <span className="text-[rgb(var(--notif-models))] shrink-0">{(probed.vram_bytes / (1024 * 1024)).toFixed(0)} MB</span>
                              </div>
                            ) : null}
                            <div className="flex items-center justify-between gap-3">
                              <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.tools}</span>
                              <span className={cn("shrink-0", probed.supports_tools === "supported" ? "text-[rgb(var(--notif-models))] font-bold" : "text-[rgb(var(--foreground-muted))]/60")}>
                                {probed.supports_tools === "supported"
                                  ? LLM_CATALOG_COPY.toolsSupported
                                  : probed.supports_tools === "unsupported"
                                    ? LLM_CATALOG_COPY.toolsNone
                                    : LLM_CATALOG_COPY.toolsUnknown}
                              </span>
                            </div>
                            <div className="flex items-center justify-between gap-3">
                              <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.languages}</span>
                              <span className="text-[rgb(var(--foreground))] font-bold shrink-0">
                                {[
                                  probed.supports_latin === "supported" && "EN",
                                  probed.supports_devanagari === "supported" && "HIN",
                                ].filter(Boolean).join(", ") || LLM_CATALOG_COPY.languagesUnknown}
                              </span>
                            </div>
                            <div className="flex items-center justify-between gap-3">
                              <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.sourceRow}</span>
                              <span className="text-[rgb(var(--foreground))] shrink-0">
                                {provenanceLabel(probed.provenance)}
                              </span>
                            </div>
                          </div>
                        </div>
                      }
                    >
                      <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] transition-all cursor-help">
                        <Sparkles size={11} className="text-[rgb(var(--accent))] shrink-0" />
                        <span>{LLM_CATALOG_COPY.capabilities}</span>
                      </div>
                    </Tooltip>
                  ) : null}
                </div>

                {/* Right Bottom: Re-probe / Benchmark Button */}
                <div className="flex items-center gap-1.5 shrink-0">
                  {!isTesting && (
                    <Tooltip label={probed ? LLM_CATALOG_COPY.rerunBenchmark : LLM_CATALOG_COPY.runBenchmark} side="top" asChild>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleProbeCapabilities(model.id);
                        }}
                        className="text-[11px] font-bold text-[rgb(var(--accent))] px-2.5 py-0.5 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 transition-all flex items-center gap-1 cursor-pointer"
                      >
                        <Sparkles size={11} />
                        <span>{probed ? LLM_CATALOG_COPY.reprobe : LLM_CATALOG_COPY.benchmark}</span>
                      </button>
                    </Tooltip>
                  )}
                </div>
              </div>
            </div>
          );
        })
      )}
      </div>
    </CatalogReveal>
  );

  return (
    <div className="w-full h-full flex flex-col min-h-0 space-y-2 animate-fade-in">
      {/* Standardized catalog header (CardHeader) */}
      <CardHeader
        icon={
          provider?.kind === "cloud" ? (
            <Cloud size={14} className="text-[rgb(var(--accent))]" />
          ) : (
            <ServerCog size={14} className="text-[rgb(var(--accent))]" />
          )
        }
        title={provider?.provider_name || (provider?.kind === "cloud" ? "Cloud models" : "Server models")}
        onBack={onBack}
        action={<LlmTuningControls />}
        filter={{
          label: LLM_CATALOG_COPY.filterAria,
          open: isFilterOpen,
          activeCount: modelFilter === "all" ? 0 : 1,
          onToggle: () => setIsFilterOpen((v) => !v),
          onClose: () => setIsFilterOpen(false),
          menu: (
            <CardFilterMenu
              title={LLM_CATALOG_COPY.filtersTitle}
              resetLabel={
                modelFilter === "all" ? undefined : LLM_CATALOG_COPY.resetFilters
              }
              onReset={resetModelFilter}
            >
              {(
                [
                  {
                    id: "all",
                    label: LLM_CATALOG_COPY.filterAll,
                    count: modelFilterCounts.all,
                    Icon: Cpu,
                  },
                  {
                    id: "favorites",
                    label: LLM_CATALOG_COPY.filterFavorites,
                    count: modelFilterCounts.favorites,
                    Icon: Heart,
                  },
                  {
                    id: "benchmarked",
                    label: LLM_CATALOG_COPY.filterBenchmarked,
                    count: modelFilterCounts.benchmarked,
                    Icon: Zap,
                  },
                  {
                    id: "tools",
                    label: LLM_CATALOG_COPY.filterTools,
                    count: modelFilterCounts.tools,
                    Icon: Wrench,
                  },
                  {
                    id: "fast",
                    label: LLM_CATALOG_COPY.filterFast,
                    count: modelFilterCounts.fast,
                    Icon: Gauge,
                  },
                ] as const
              ).map((item) => (
                <CardFilterItem
                  key={item.id}
                  icon={item.Icon}
                  label={item.label}
                  count={item.count}
                  active={modelFilter === item.id}
                  onSelect={() => {
                    setModelFilter(item.id);
                    setIsFilterOpen(false);
                  }}
                />
              ))}
            </CardFilterMenu>
          ),
        }}
        sort={{
          order: sort,
          onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
        }}
        search={{
          open: isSearchOpen,
          value: searchQuery,
          placeholder: LLM_CATALOG_COPY.searchPlaceholder,
          onChange: setSearchQuery,
          onToggle: () => setIsSearchOpen((v) => !v),
          onClose: () => {
            setSearchQuery("");
            setIsSearchOpen(false);
          },
        }}
      />

      {/* Errors surface inline. Loading is carried by skeleton rows in the
          list, so the header never changes width mid-fetch. */}
      {remoteModelsError && (
        <div className="flex items-center gap-2 shrink-0">
          <Tooltip
            label={
              typeof remoteModelsError === "string" &&
              !remoteModelsError.includes("[object")
                ? remoteModelsError
                : LLM_CATALOG_COPY.serverErrorTooltip
            }
            side="bottom"
            mobileTapEnabled
          >
            <span className="inline-flex items-center text-[rgb(var(--warn-soft))] shrink-0 cursor-help">
              <AlertCircle size={14} />
            </span>
          </Tooltip>
        </div>
      )}


      {/* Remote Models 2-Column Grid — inline at card height, full-bleed in the expand modal */}
      <ExpandableList
        className="flex-1 min-h-0"
        domainId="language"
        triggerPlacement="floating"
        inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
        onBack={showProvidersView ? () => setCatalogModalView(activeModalView === "models" ? "providers" : "models") : undefined}
        icon={activeModalView === "models" ? <ServerCog size={16} className="text-[rgb(var(--accent))]" /> : <Cloud size={16} className="text-[rgb(var(--accent))]" />}
        title={
          <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
            {activeModalView === "models" ? LLM_CATALOG_COPY.catalogTitle : "Cloud Providers"}
          </span>
        }
        subtitle={
          activeModalView === "models" ? (
            <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
              {remoteModels.length} {LLM_CATALOG_COPY.modelCountLabel}
            </span>
          ) : (
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
              Connectors · {cloudProviders.length}
            </span>
          )
        }
        expandLabel={LLM_CATALOG_COPY.expandLabel}
        ariaLabel={LLM_CATALOG_COPY.catalogAriaLabel}
        modalSubtitle={
          activeModalView === "models"
            ? "Explore, probe, and select remote models for conversation."
            : "Configure external AI model endpoints and authentication keys."
        }
        headerActions={<SettingsCommitControls domainId="language" layoutMode={layoutMode} />}
        modalContent={
          activeModalView === "models" ? (
            <ModelModal
              remoteModels={remoteModels}
              selectedModelId={selectedModelId}
              pinnedModelId={committedRemoteModelId}
              probingMap={probingMap}
              capabilitiesCache={capabilitiesCache}
              onSelectModel={handleSelectModel}
              onProbeCapabilities={handleProbeCapabilities}
            />
          ) : (
            <ProviderModal {...cloudProvidersModalProps} />
          )
        }
      >
        {RemoteModelRows()}
      </ExpandableList>
    </div>
  );
});

RemoteCatalogSection.displayName = "RemoteCatalogSection";
