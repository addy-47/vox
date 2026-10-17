import {
  memo,
  useState,
  useMemo,
  useEffect,
  useCallback,
} from "react";
import {
  Loader2,
  ServerCog,
  Cloud,
  RefreshCw,
  AlertCircle,
  AlertTriangle,
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
import { fzfMultiTermScore } from "@/shared/lib/fuzzy";
import { useLenisScrollContainer, useScrollTopOnChange } from "@/shared/hooks/useLenisScrollContainer";
import { useFavoriteModels } from "@/shared/hooks/useFavoriteModels";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";
import { CLOUD_PROVIDERS } from "@/data/providersCopy";
import {
  useSettingsStore,
  type LlmModelInfo,
  type ModelCapabilities,
  type LlmProviderConfig,
} from "@/store/settingsStore";
import { ProviderModal } from "../../providers/ProviderModal";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import { ModelModal } from "../../providers/ModelModal";
import {
  CatalogFilterItem,
  CatalogFilterMenu,
  CatalogHeader,
  type CatalogSortOrder,
} from "../../providers/CatalogHeader";
import {
  REMOTE_MODEL_PAGE_SIZE,
  provenanceLabel,
  hasProbeWarning,
  probeWarningLabel,
} from "./catalogProbeUtils";

export interface RemoteCatalogSectionProps {
  layoutMode?: "full-max" | "full-min" | "small";
  provider?: LlmProviderConfig;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  handleProbeCapabilities: (id?: string) => void;
}

export const RemoteCatalogSection = memo(({
  layoutMode,
  provider,
  remoteModels,
  loadingRemoteModels,
  remoteModelsError,
  probingMap,
  handleProbeCapabilities,
}: RemoteCatalogSectionProps) => {
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);
  const committedSettings = useSettingsStore((s) => s.settings);

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
  const [isSearching, setIsSearching] = useState(false);
  const [sort, setSort] = useState<CatalogSortOrder>("az");
  const [modelFilter, setModelFilter] = useState<
    "all" | "favorites" | "benchmarked" | "tools" | "fast"
  >("all");
  const [isFilterOpen, setIsFilterOpen] = useState(false);

  const { favoriteModelIds } = useFavoriteModels();

  /** Resolved probe capabilities for a model (live probe → model payload →
   * capability cache). Single source for the filter predicates below. */
  const resolveCapabilities = useCallback(
    (model: LlmModelInfo): ModelCapabilities | undefined =>
      probingMap[model.id]?.capabilities ||
      model.capabilities ||
      capabilitiesCache?.[`server:${model.id}`] ||
      capabilitiesCache?.[`cloud:${model.id}`] ||
      capabilitiesCache?.[`embedded:${model.id}`] ||
      capabilitiesCache?.[model.id],
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
    const trimmed = searchQuery.trim();
    let models = remoteModels;

    if (trimmed) {
      const terms = trimmed.split(/\s+/).filter(Boolean);
      if (terms.length > 0) {
        const scored: Array<{ model: LlmModelInfo; score: number }> = [];

        for (const model of remoteModels) {
          const candidateFields = [
            model.id,
            model.name,
            model.family || "",
            model.quantization || "",
          ];
          const score = fzfMultiTermScore(terms, candidateFields);
          if (score !== null) {
            scored.push({ model, score });
          }
        }

        // Sort descending by match score so the best fuzzy match is at the top
        scored.sort((a, b) => b.score - a.score);
        models = scored.map((item) => item.model);
      }
    }

    if (modelFilter !== "all") {
      models = models.filter((model) => {
        if (modelFilter === "favorites") return favoriteModelIds.has(model.id);
        const probed = resolveCapabilities(model);
        if (modelFilter === "benchmarked") return probed?.tps != null && probed.tps > 0;
        if (modelFilter === "tools") return probed?.supports_tools === "supported";
        return probed?.tps != null && probed.tps >= 30;
      });
    }

    const sorted = [...models];
    sorted.sort((a, b) => {
      const an = a.name || a.id;
      const bn = b.name || b.id;
      const cmp = an.localeCompare(bn, undefined, { sensitivity: "base" });
      return sort === "az" ? cmp : -cmp;
    });
    models = sorted;

    // Active card primacy: committedRemoteModelId (or selectedModelId) is placed first in the list
    const activeModelId = committedRemoteModelId || selectedModelId;
    console.log("[RemoteCatalogSection] Model primacy:", {
      activeModelId,
      committedRemoteModelId,
      selectedModelId,
      totalModels: models.length,
      firstBeforePin: models[0]?.id,
    });
    if (activeModelId) {
      const selectedIndex = models.findIndex((m) => m.id === activeModelId);
      if (selectedIndex > 0) {
        const copy = [...models];
        const [selected] = copy.splice(selectedIndex, 1);
        copy.unshift(selected);
        console.log("[RemoteCatalogSection] Pinned model to index 0:", selected.id);
        return copy;
      }
    }

    return models;
  }, [
    remoteModels,
    searchQuery,
    modelFilter,
    sort,
    favoriteModelIds,
    resolveCapabilities,
    selectedModelId,
    committedRemoteModelId,
  ]);

  // Capped render window for the inline grid (see REMOTE_MODEL_PAGE_SIZE).
  const [visibleModelCount, setVisibleModelCount] = useState(REMOTE_MODEL_PAGE_SIZE);
  useEffect(() => {
    setVisibleModelCount(REMOTE_MODEL_PAGE_SIZE);
  }, [searchQuery, modelFilter, sort, selectedModelId, committedRemoteModelId, remoteModels]);

  const visibleInlineModels = useMemo(
    () => filteredRemoteModels.slice(0, visibleModelCount),
    [filteredRemoteModels, visibleModelCount]
  );
  const inlineHiddenCount = filteredRemoteModels.length - visibleInlineModels.length;

  const handleSelectModel = useCallback((modelId: string) => {
    const active = useSettingsStore.getState().draftSettings?.llm?.active;
    if (active && (active.category === "server" || active.category === "cloud")) {
      updateDraft("llm", "active", { ...active, model: modelId });
    }
  }, [updateDraft]);

  const { containerRef: inlineRowsRef, lenisRef: inlineLenisRef } =
    useLenisScrollContainer<HTMLDivElement>();
  useScrollTopOnChange(inlineRowsRef, selectedModelId || committedRemoteModelId, inlineLenisRef);

  const RemoteModelRows = () => (
    <div
      ref={inlineRowsRef}
      className="h-full overflow-y-auto pr-1 custom-scrollbar"
    >
      <div
        className={cn(
          "grid auto-rows-max content-start gap-2 w-full",
          layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
        )}
      >
      {remoteModels.length === 0 ? (
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
        visibleInlineModels.map((model) => {
          const isSelected = selectedModelId === model.id;
          const probed =
            probingMap[model.id]?.capabilities ||
            model.capabilities ||
            capabilitiesCache?.[`server:${model.id}`] ||
            capabilitiesCache?.[`cloud:${model.id}`] ||
            capabilitiesCache?.[`embedded:${model.id}`] ||
            capabilitiesCache?.[model.id];
          const isTesting = probingMap[model.id]?.status === "testing";
          const inlineProbeError = probingMap[model.id]?.status === "error" ? probingMap[model.id]?.error : undefined;
          const isGpu = probed?.is_gpu_accelerated;

          // Check if name is essentially a duplicate of the raw ID (e.g. "01 ai/yi large" vs "01-ai/yi-large")
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
              onClick={() => {
                const active = useSettingsStore.getState().draftSettings?.llm?.active;
                if (active && (active.category === "server" || active.category === "cloud")) {
                  updateDraft("llm", "active", { ...active, model: model.id });
                }
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  const active = useSettingsStore.getState().draftSettings?.llm?.active;
                  if (active && (active.category === "server" || active.category === "cloud")) {
                    updateDraft("llm", "active", { ...active, model: model.id });
                  }
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
                  ) : (
                    null
                  )}
                </div>

                {/* Right Bottom: Re-probe / Benchmark Button */}
                <div className="flex items-center gap-1.5 shrink-0">
                  {!isTesting && (
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleProbeCapabilities(model.id);
                      }}
                      className="text-[11px] font-bold text-[rgb(var(--accent))] px-2.5 py-0.5 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 transition-all flex items-center gap-1 cursor-pointer"
                      title={probed ? LLM_CATALOG_COPY.rerunBenchmark : LLM_CATALOG_COPY.runBenchmark}
                    >
                      <Sparkles size={11} />
                      <span>{probed ? LLM_CATALOG_COPY.reprobe : LLM_CATALOG_COPY.benchmark}</span>
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        })
      )}
      {inlineHiddenCount > 0 && (
        <div className="col-span-full flex items-center justify-center py-2 mt-1">
          <button
            type="button"
            onClick={() => {
              setVisibleModelCount((c) => c + REMOTE_MODEL_PAGE_SIZE);
              setTimeout(() => inlineLenisRef.current?.resize(), 50);
            }}
            className="px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-colors cursor-pointer"
          >
            {`Show ${inlineHiddenCount} more`}
          </button>
        </div>
      )}
      </div>
    </div>
  );

  return (
    <div className="w-full h-full flex flex-col min-h-0 space-y-2 animate-fade-in">
      {/* Standardized catalog header (design-spec §10.1) */}
      <CatalogHeader
        icon={<ServerCog size={14} className="text-[rgb(var(--accent))]" />}
        title="Connected Server"
        status={
          remoteModelsError ? (
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
          ) : loadingRemoteModels ? (
            <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1 px-2 py-1 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 shrink-0">
              <RefreshCw size={12} className="animate-spin" /> {LLM_CATALOG_COPY.fetching}
            </span>
          ) : undefined
        }
        filter={{
          label: LLM_CATALOG_COPY.filterAria,
          open: isFilterOpen,
          activeCount: modelFilter === "all" ? 0 : 1,
          onToggle: () => setIsFilterOpen((v) => !v),
          onClose: () => setIsFilterOpen(false),
          menu: (
            <CatalogFilterMenu
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
                <CatalogFilterItem
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
            </CatalogFilterMenu>
          ),
        }}
        sort={{
          order: sort,
          onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
        }}
        search={{
          open: isSearching,
          value: searchQuery,
          placeholder: LLM_CATALOG_COPY.searchPlaceholder,
          onChange: setSearchQuery,
          onToggle: () =>
            setIsSearching((v) => {
              if (v) setSearchQuery("");
              return !v;
            }),
          onClose: () => {
            setIsSearching(false);
            setSearchQuery("");
          },
        }}
      />


      {/* Remote Models 2-Column Grid — inline at card height, full-bleed in the expand modal */}
      <ExpandableList
        className="flex-1 min-h-0"
        domainId="models"
        triggerPlacement="floating"
        inlineMaxHeightClass={layoutMode === "small" ? "max-h-[220px]" : "max-h-[480px]"}
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
              Connectors · {CLOUD_PROVIDERS.length}
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
        headerActions={<SettingsCommitControls domainId="models" layoutMode={layoutMode} />}
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
