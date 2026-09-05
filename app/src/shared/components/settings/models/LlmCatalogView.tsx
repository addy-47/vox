import { memo, useState, useMemo, useRef, useEffect } from "react";
import { useSettingsStore, type LlmModelInfo, type ModelCapabilities, type LlmProviderConfig } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { Loader2, Network, RefreshCw, AlertCircle, Sparkles, Search, X, Plus } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { fzfMultiTermScore } from "@/shared/lib/fuzzy";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";

export interface LlmCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  selectedLlmId: string;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  startDownload: (id: string) => void;
  handleDeleteModelGroup: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
  isRemoteLlm: boolean;
  provider?: LlmProviderConfig;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  handleProbeCapabilities: (id?: string) => void;
  customModelId: string;
  setCustomModelId: (id: string) => void;
  customModelStatus?: 'idle' | 'checking' | 'valid' | 'invalid';
  handleValidateCustomModel?: () => void;
}

export const LlmCatalogView = memo(({
  layoutMode,
  selectedLlmId,
  modelPresence,
  downloadStatuses,
  confirmDeleteId,
  setConfirmDeleteId,
  startDownload,
  handleDeleteModelGroup,
  isGroupRequired,
  isRemoteLlm,
  provider,
  remoteModels,
  loadingRemoteModels,
  remoteModelsError,
  probingMap,
  handleProbeCapabilities,
  customModelId,
  setCustomModelId,
  customModelStatus: _customModelStatus,
  handleValidateCustomModel: _handleValidateCustomModel,
}: LlmCatalogViewProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Search State for Remote Catalog
  const [searchQuery, setSearchQuery] = useState("");
  const [isSearching, setIsSearching] = useState(false);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  // Custom Model ID expandable bar
  const [isCustomInputOpen, setIsCustomInputOpen] = useState(false);
  const customInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (isSearching && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearching]);

  useEffect(() => {
    if (isCustomInputOpen && customInputRef.current) {
      customInputRef.current.focus();
    }
  }, [isCustomInputOpen]);

  const selectedModelId = provider && "model" in provider ? provider.model : undefined;

  // Filtered Remote Models with fzf-style fuzzy matching and selected model prioritized first
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

    // Pinned: The selected model is placed first in the list
    if (selectedModelId) {
      const selectedIndex = models.findIndex((m) => m.id === selectedModelId);
      if (selectedIndex > 0) {
        const copy = [...models];
        const [selected] = copy.splice(selectedIndex, 1);
        copy.unshift(selected);
        return copy;
      }
    }

    return models;
  }, [remoteModels, searchQuery, selectedModelId]);

  const handleApplyCustomModel = () => {
    const modelId = customModelId.trim();
    if (!modelId) return;
    const draft = useSettingsStore.getState().draftSettings;
    const activeLlm = draft?.llm?.active || "embedded";
    if (activeLlm === "server" && draft?.llm?.server) {
      updateDraft("llm", "server", { ...draft.llm.server, model: modelId });
    } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
      updateDraft("llm", "cloud", { ...draft.llm.cloud, model: modelId });
    } else if (activeLlm === "embedded" && draft?.llm?.embedded) {
      updateDraft("llm", "embedded", { ...draft.llm.embedded, model: modelId });
    }
    setIsCustomInputOpen(false);
  };

  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);

  // Model Tab: Local GGUF Model Grid (Pinned: selected model first)
  const sortedLocalModels = useMemo(() => {
    const list = [...(modelCatalog?.llm || [])];
    if (selectedLlmId) {
      const idx = list.findIndex((m) => m.id === selectedLlmId);
      if (idx > 0) {
        const [selected] = list.splice(idx, 1);
        list.unshift(selected);
      }
    }
    return list;
  }, [modelCatalog?.llm, selectedLlmId]);

  // Remote / OpenAI-Compat Server Catalog
  if (isRemoteLlm) {
    const remoteUrl = provider && "base_url" in provider ? provider.base_url : undefined;

    return (
      <div className="w-full h-full flex flex-col min-h-0 space-y-2 animate-fade-in">
        {/* Connected Server Header with Search Bar / Custom Model Input (Fixed/Sticky at top) */}
        <div className="flex items-center justify-between gap-2 min-h-[34px] pb-1 border-b border-[rgba(var(--foreground),0.04)] shrink-0">
          {isCustomInputOpen ? (
            <div className="flex items-center gap-2 w-full animate-fade-in">
              <div className="flex-1 flex items-center gap-2 px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.35)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
                <Plus size={14} className="text-[rgb(var(--accent))] shrink-0" />
                <input
                  ref={customInputRef}
                  type="text"
                  value={customModelId}
                  onChange={(e) => setCustomModelId(e.target.value)}
                  placeholder={LLM_CATALOG_COPY.customModelPlaceholder}
                  className="w-full bg-transparent border-none outline-none text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
                  onKeyDown={(e) => {
                    if (e.key === "Escape") {
                      setIsCustomInputOpen(false);
                    } else if (e.key === "Enter" && customModelId.trim()) {
                      handleApplyCustomModel();
                    }
                  }}
                />
                {customModelId && (
                  <button
                    type="button"
                    onClick={() => setCustomModelId("")}
                    className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
                  >
                    <X size={13} />
                  </button>
                )}
              </div>
              <button
                type="button"
                disabled={!customModelId.trim()}
                onClick={handleApplyCustomModel}
                className="px-3 py-1 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider hover:brightness-110 active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed transition-all cursor-pointer shrink-0"
              >
                {LLM_CATALOG_COPY.use}
              </button>
              <button
                type="button"
                onClick={() => setIsCustomInputOpen(false)}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] transition-all cursor-pointer shrink-0"
              >
                <X size={15} />
              </button>
            </div>
          ) : isSearching || searchQuery ? (
            <div className="flex items-center gap-2 w-full animate-fade-in">
              <div className="flex-1 flex items-center gap-2 px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.35)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
                <Search size={14} className="text-[rgb(var(--accent))] shrink-0" />
                <input
                  ref={searchInputRef}
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") {
                      if (searchQuery) {
                        setSearchQuery("");
                      } else {
                        setIsSearching(false);
                      }
                    } else if (e.key === "Enter") {
                      searchInputRef.current?.blur();
                    }
                  }}
                  placeholder={LLM_CATALOG_COPY.searchPlaceholder}
                  className="w-full bg-transparent border-none outline-none text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
                  >
                    <X size={13} />
                  </button>
                )}
              </div>
              <button
                type="button"
                onClick={() => {
                  setIsSearching(false);
                  setSearchQuery("");
                }}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] transition-all cursor-pointer shrink-0"
              >
                <X size={15} />
              </button>
            </div>
          ) : (
            <>
              <div className="flex flex-col min-w-0">
                <span className="font-bold text-[rgb(var(--foreground))] text-[13px] flex items-center gap-1.5 truncate">
                  <Network size={15} className="text-[rgb(var(--accent))] shrink-0" />
                  <span>{LLM_CATALOG_COPY.connectedServer}</span>
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-mono truncate max-w-[200px] sm:max-w-[280px]">
                  {remoteUrl || LLM_CATALOG_COPY.noServer}
                </span>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                {loadingRemoteModels ? (
                  <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1 px-2 py-1 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20">
                    <RefreshCw size={12} className="animate-spin" /> {LLM_CATALOG_COPY.fetching}
                  </span>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => {
                        setIsCustomInputOpen(true);
                        setIsSearching(false);
                      }}
                      className="p-1.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] hover:border-[rgb(var(--accent))]/40 hover:bg-[rgba(var(--accent),0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all cursor-pointer shadow-sm flex items-center justify-center"
                      title={LLM_CATALOG_COPY.customModelTitle}
                      aria-label={LLM_CATALOG_COPY.customModelAria}
                    >
                      <Plus size={15} />
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setIsSearching(true);
                        setIsCustomInputOpen(false);
                      }}
                      className="p-1.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] hover:border-[rgb(var(--accent))]/40 hover:bg-[rgba(var(--accent),0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all cursor-pointer shadow-sm flex items-center justify-center"
                      title={LLM_CATALOG_COPY.searchTitle}
                      aria-label={LLM_CATALOG_COPY.searchAria}
                    >
                      <Search size={15} />
                    </button>
                    <span className="px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] text-[11px] font-mono font-medium">
                      {remoteModels.length} models
                    </span>
                  </>
                )}
              </div>
            </>
          )}
        </div>

        {remoteModelsError && (
          <div className="text-[12px] font-bold text-red-400/90 bg-red-400/5 border border-red-400/20 rounded-xl px-3 py-2 flex items-center gap-2 shrink-0">
            <AlertCircle size={16} className="shrink-0 text-red-400" />
            <span>{remoteModelsError}</span>
          </div>
        )}

        {/* Remote Models 2-Column Grid (Only Scrollable Area) */}
        <div
          className={cn(
            "grid auto-rows-max content-start gap-2 pr-1 flex-1 min-h-0 overflow-y-auto custom-scrollbar",
            layoutMode === "small" ? "grid-cols-1 max-h-[235px]" : "grid-cols-1 sm:grid-cols-2"
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
                className="px-3 py-1 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
              >
                {LLM_CATALOG_COPY.clearSearch}
              </button>
            </div>
          ) : (
            filteredRemoteModels.map((model) => {
              const isSelected = selectedModelId === model.id;
              const probed =
                probingMap[model.id]?.capabilities ||
                model.capabilities ||
                capabilitiesCache?.[`open_ai_compat:${model.id}`] ||
                capabilitiesCache?.[`server:${model.id}`] ||
                capabilitiesCache?.[`cloud:${model.id}`] ||
                capabilitiesCache?.[`embedded:${model.id}`] ||
                capabilitiesCache?.[model.id];
              const isTesting = probingMap[model.id]?.status === "testing";
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
                    const draft = useSettingsStore.getState().draftSettings;
                    const activeLlm = draft?.llm?.active || "embedded";
                    if (activeLlm === "server" && draft?.llm?.server) {
                      updateDraft("llm", "server", { ...draft.llm.server, model: model.id });
                    } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
                      updateDraft("llm", "cloud", { ...draft.llm.cloud, model: model.id });
                    } else if (activeLlm === "embedded" && draft?.llm?.embedded) {
                      updateDraft("llm", "embedded", { ...draft.llm.embedded, model: model.id });
                    }
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      const draft = useSettingsStore.getState().draftSettings;
                      const activeLlm = draft?.llm?.active || "embedded";
                      if (activeLlm === "server" && draft?.llm?.server) {
                        updateDraft("llm", "server", { ...draft.llm.server, model: model.id });
                      } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
                        updateDraft("llm", "cloud", { ...draft.llm.cloud, model: model.id });
                      } else if (activeLlm === "embedded" && draft?.llm?.embedded) {
                        updateDraft("llm", "embedded", { ...draft.llm.embedded, model: model.id });
                      }
                    }
                  }}
                  className={cn(
                    "group w-full text-left p-3 rounded-xl border transition-all duration-200 relative shrink-0 cursor-pointer min-h-[64px] flex flex-col justify-between hover:z-20",
                    isSelected
                      ? "bg-[rgba(var(--accent),0.07)] border-[rgb(var(--accent))] shadow-[0_0_16px_rgba(var(--accent),0.12)] ring-1 ring-[rgb(var(--accent))]/40"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.03)]",
                    isGpu && !isSelected ? "border-purple-500/30" : ""
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
                      </div>

                      {/* Optional Subtitle (Only shown if model.name is genuinely different from model.id, or to display size) */}
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

                    {/* Right Top: Reset / Re-run Probe Icon */}
                    <div className="flex items-center gap-1.5 shrink-0">
                      {probed && !isTesting && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleProbeCapabilities(model.id);
                          }}
                          className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer"
                          title={LLM_CATALOG_COPY.rerunBenchmark}
                        >
                          <RefreshCw size={12} />
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Bottom Row: Unified Capabilities Trigger (Left) & Family Badge or Benchmark (Right) */}
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
                          className="p-3 min-w-[220px] whitespace-normal text-left z-50 border border-[rgba(var(--foreground),0.14)] bg-[rgb(var(--card))]/98 shadow-2xl backdrop-blur-2xl"
                          label={
                            <div className="space-y-1.5 text-[11px] font-sans">
                              <div className="font-bold text-[rgb(var(--foreground))] border-b border-[rgba(var(--foreground),0.08)] pb-1 flex items-center justify-between">
                                <span>{LLM_CATALOG_COPY.modelCapabilities}</span>
                                {isGpu ? (
                                  <span className="text-purple-400 font-mono text-[10px] font-bold">{LLM_CATALOG_COPY.gpuBadge}</span>
                                ) : probed?.server_has_gpu ? (
                                  <span className="text-amber-400 font-mono text-[10px] font-bold">{LLM_CATALOG_COPY.cpuBadge}</span>
                                ) : null}
                              </div>
                              <div className="space-y-1 font-mono text-[10.5px]">
                                {probed.tps != null && probed.tps > 0 && (
                                  <div className="flex justify-between gap-3">
                                    <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.speed}</span>
                                    <span className="text-emerald-400 font-bold">⚡ {probed.tps.toFixed(1)} tps</span>
                                  </div>
                                )}
                                <div className="flex justify-between gap-3">
                                    <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.context}</span>
                                  <span className="text-[rgb(var(--foreground))]">
                                    {probed.context_window
                                      ? probed.context_window >= 1000000
                                        ? `${(probed.context_window / 1000000).toFixed(1)}M tokens`
                                        : `${Math.round(probed.context_window / 1024)}k tokens`
                                      : LLM_CATALOG_COPY.managed}
                                  </span>
                                </div>
                                {probed.vram_bytes ? (
                                  <div className="flex justify-between gap-3">
                                    <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.vram}</span>
                                    <span className="text-purple-300">{(probed.vram_bytes / (1024 * 1024)).toFixed(0)} MB</span>
                                  </div>
                                ) : null}
                                <div className="flex justify-between gap-3">
                                    <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.tools}</span>
                                  <span className={probed.supports_tools ? "text-blue-400 font-bold" : "text-[rgb(var(--foreground-muted))]/60"}>
                                    {probed.supports_tools ? LLM_CATALOG_COPY.toolsSupported : LLM_CATALOG_COPY.toolsNone}
                                  </span>
                                </div>
                                <div className="flex justify-between gap-3">
                                    <span className="text-[rgb(var(--foreground-muted))]">{LLM_CATALOG_COPY.languages}</span>
                                  <span className="text-[rgb(var(--foreground))] font-bold">
                                    {[
                                      probed.supports_latin && "EN",
                                      probed.supports_devanagari && "HIN",
                                    ].filter(Boolean).join(", ") || LLM_CATALOG_COPY.languageStandard}
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
                        <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/50 font-mono">{LLM_CATALOG_COPY.notBenchmarked}</span>
                      )}
                    </div>

                    {/* Right: Family Badge (when probed) OR Benchmark Button (when unprobed) */}
                    <div className="flex items-center gap-1.5 shrink-0">
                      {probed && model.family && (
                        <span className="text-[10.5px] font-bold px-2 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.15)] leading-none">
                          {model.family}
                        </span>
                      )}

                      {!probed && !isTesting && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleProbeCapabilities(model.id);
                          }}
                          className="text-[11px] font-bold text-[rgb(var(--accent))] px-2.5 py-0.5 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 transition-all flex items-center gap-1 cursor-pointer"
                          title={LLM_CATALOG_COPY.runBenchmark}
                        >
                          <Sparkles size={11} />
                          <span>{LLM_CATALOG_COPY.benchmark}</span>
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "grid gap-2.5 flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 h-full",
        sortedLocalModels.length <= 2
          ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
          : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
      )}
    >
      {sortedLocalModels.map((model) => {
        const isSelected = selectedLlmId === model.id;
        const isDownloaded = modelPresence[model.id];
        const status = downloadStatuses[model.id];

        return (
          <SubModelCard
            key={model.id}
            id={model.id}
            name={model.name}
            description={model.description || ""}
            parameters={model.parameters || ""}
            ramUsage={model.ram_usage}
            tradeoffs={model.tradeoffs}
            isDownloaded={isDownloaded}
            isActive={isSelected}
            isRequired={isGroupRequired(model.id)}
            layoutMode={layoutMode}
            onSelect={() => {
              const draft = useSettingsStore.getState().draftSettings;
              if (draft?.llm?.embedded) {
                updateDraft("llm", "embedded", { ...draft.llm.embedded, model: model.id });
              }
            }}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={status}
            startDownload={() => startDownload(model.id)}
            deleteModel={() => handleDeleteModelGroup(model.id)}
            showTooltip={false}
          />
        );
      })}
    </div>
  );
});

LlmCatalogView.displayName = "LlmCatalogView";
