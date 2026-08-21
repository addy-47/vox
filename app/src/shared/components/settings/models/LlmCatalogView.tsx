import { memo, useState, useMemo, useRef, useEffect } from "react";
import { useSettingsStore, type LlmModelInfo, type ModelCapabilities, type LlmProviderConfig } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { Loader2, Network, RefreshCw, AlertCircle, Sparkles, Check, Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { fzfMultiTermScore } from "@/shared/lib/fuzzy";

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
  customModelStatus: 'idle' | 'checking' | 'valid' | 'invalid';
  handleValidateCustomModel: () => void;
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
  customModelStatus,
  handleValidateCustomModel,
}: LlmCatalogViewProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Search State for Remote Catalog
  const [searchQuery, setSearchQuery] = useState("");
  const [isSearching, setIsSearching] = useState(false);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (isSearching && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearching]);

  // Filtered Remote Models with fzf-style fuzzy matching and ranking
  const filteredRemoteModels = useMemo(() => {
    const trimmed = searchQuery.trim();
    if (!trimmed) return remoteModels;

    const terms = trimmed.split(/\s+/).filter(Boolean);
    if (terms.length === 0) return remoteModels;

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

    return scored.map((item) => item.model);
  }, [remoteModels, searchQuery]);

  // Remote / OpenAI-Compat Server Catalog
  if (isRemoteLlm) {
    const remoteUrl = provider && "base_url" in provider ? provider.base_url : undefined;
    const selectedModelId = provider && "model" in provider ? provider.model : undefined;

    return (
      <div className="space-y-3 w-full animate-fade-in">
        {/* Connected Server Header with Search Bar */}
        <div className="flex items-center justify-between gap-2 min-h-[38px] pb-1 border-b border-[rgba(var(--foreground),0.04)]">
          {!isSearching && !searchQuery ? (
            <>
              <div className="flex flex-col min-w-0">
                <span className="font-bold text-[rgb(var(--foreground))] text-[13px] flex items-center gap-1.5 truncate">
                  <Network size={15} className="text-[rgb(var(--accent))] shrink-0" />
                  <span>Connected Server</span>
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] font-mono truncate max-w-[200px] sm:max-w-[280px]">
                  {remoteUrl || "No server configured"}
                </span>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                {loadingRemoteModels ? (
                  <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1 px-2 py-1 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20">
                    <RefreshCw size={12} className="animate-spin" /> Fetching...
                  </span>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => setIsSearching(true)}
                      className="p-1.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] hover:border-[rgb(var(--accent))]/40 hover:bg-[rgba(var(--accent),0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all cursor-pointer shadow-sm flex items-center justify-center"
                      title="Search models (fzf fuzzy filter)"
                      aria-label="Search models"
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
          ) : (
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
                  placeholder="Fuzzy search models by name, ID, family, or quant..."
                  className="w-full bg-transparent border-none outline-none text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 font-sans"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => {
                      setSearchQuery("");
                      searchInputRef.current?.focus();
                    }}
                    className="p-0.5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                    title="Clear search text"
                  >
                    <X size={13} />
                  </button>
                )}
              </div>

              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] shrink-0 font-medium px-1">
                {filteredRemoteModels.length}/{remoteModels.length}
              </span>

              <button
                type="button"
                onClick={() => {
                  setIsSearching(false);
                  setSearchQuery("");
                }}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] transition-colors cursor-pointer shrink-0"
                title="Close search"
              >
                <X size={15} />
              </button>
            </div>
          )}
        </div>

        {remoteModelsError && (
          <div className="text-[12px] font-bold text-red-400/90 bg-red-400/5 border border-red-400/20 rounded-xl px-3 py-2 flex items-center gap-2">
            <AlertCircle size={16} className="shrink-0 text-red-400" />
            <span>{remoteModelsError}</span>
          </div>
        )}

        {/* Remote Models 2-Column Grid */}
        <div
          className={cn(
            "grid gap-2.5 pr-1",
            layoutMode === "small"
              ? "grid-cols-1 max-h-none overflow-y-visible"
              : "grid-cols-1 sm:grid-cols-2 max-h-[240px] overflow-y-auto custom-scrollbar"
          )}
        >
          {remoteModels.length === 0 ? (
            <div className="col-span-full text-center py-8 text-[12px] text-[rgb(var(--foreground-muted))]/70 space-y-1">
              <p className="font-semibold text-[rgb(var(--foreground))]/80">No remote models loaded</p>
              <p className="text-[11px]">Ensure the remote server is online and configured in the Interaction Card.</p>
            </div>
          ) : filteredRemoteModels.length === 0 ? (
            <div className="col-span-full text-center py-8 text-[12px] text-[rgb(var(--foreground-muted))]/70 space-y-2">
              <p className="font-semibold text-[rgb(var(--foreground))]/80">No models matching &ldquo;{searchQuery}&rdquo;</p>
              <button
                type="button"
                onClick={() => setSearchQuery("")}
                className="px-3 py-1 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
              >
                Clear Search
              </button>
            </div>
          ) : (
            filteredRemoteModels.map((model) => {
              const isSelected = selectedModelId === model.id;
              const probed = probingMap[model.id]?.capabilities || model.capabilities;
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
                <button
                  key={model.id}
                  type="button"
                  onClick={() => {
                    if (provider && "base_url" in provider) {
                      updateDraft("llm", "provider", { ...provider, model: model.id });
                    }
                  }}
                  className={cn(
                    "group w-full text-left p-3.5 rounded-xl border transition-all duration-200 relative overflow-hidden cursor-pointer",
                    isSelected
                      ? "bg-[rgba(var(--accent),0.07)] border-[rgb(var(--accent))] shadow-[0_0_16px_rgba(var(--accent),0.12)] ring-1 ring-[rgb(var(--accent))]/40"
                      : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.03)]",
                    isGpu && !isSelected ? "border-purple-500/30" : ""
                  )}
                >
                  {/* Top Row: Title + Tags + Selection Indicator */}
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
                        {model.family && (
                          <span className="text-[10.5px] font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.1)] leading-none">
                            {model.family}
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

                    {/* Right Top: GPU/Hardware Tag + Selection Circle */}
                    <div className="flex items-center gap-2 shrink-0">
                      {isGpu ? (
                        <Tooltip label={probed?.gpu_status || "GPU Offloaded"}>
                          <span className="text-[10.5px] font-bold font-mono px-2 py-0.5 rounded-full bg-purple-500/15 text-purple-300 border border-purple-500/30 flex items-center gap-1">
                            🚀 GPU {probed?.vram_bytes ? `${(probed.vram_bytes / (1024 * 1024)).toFixed(0)}MB` : ""}
                          </span>
                        </Tooltip>
                      ) : probed?.server_has_gpu ? (
                        <Tooltip label="Server has GPU hardware, but model is running in CPU mode">
                          <span className="text-[10.5px] font-bold font-mono px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/30 flex items-center gap-1">
                            ⚠️ CPU Mode
                          </span>
                        </Tooltip>
                      ) : null}

                      <div className={cn(
                        "w-5 h-5 rounded-full flex items-center justify-center transition-all duration-200",
                        isSelected
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm ring-2 ring-[rgb(var(--accent))]/30"
                          : "border border-[rgba(var(--foreground),0.18)] group-hover:border-[rgb(var(--accent))]/60 group-hover:scale-105"
                      )}>
                        {isSelected && <Check size={12} strokeWidth={3} />}
                      </div>
                    </div>
                  </div>

                  {/* Bottom Row: Capabilities & Metrics (Spanning Left to Right) */}
                  <div className="flex items-center justify-between gap-2 mt-2.5 pt-2 border-t border-[rgba(var(--foreground),0.04)]">
                    {/* Left: Capability badges */}
                    <div className="flex items-center gap-1.5 flex-wrap min-w-0">
                      {isTesting ? (
                        <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1.5 py-0.5">
                          <Loader2 size={12} className="animate-spin" />
                          <span>Testing capabilities...</span>
                        </span>
                      ) : probed ? (
                        <>
                          {probed.supports_tools && (
                            <Tooltip label="Supports tool calling / function calling">
                              <span className="text-[10.5px] font-bold px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 flex items-center gap-1">
                                🛠️ Tools
                              </span>
                            </Tooltip>
                          )}
                          {probed.supports_latin && (
                            <Tooltip label="Supports Latin Script (English)">
                              <span className="text-[10.5px] font-mono font-bold px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                                EN
                              </span>
                            </Tooltip>
                          )}
                          {probed.supports_devanagari && (
                            <Tooltip label="Supports Devanagari Script (Hindi / Hinglish)">
                              <span className="text-[10.5px] font-mono font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                                DEV
                              </span>
                            </Tooltip>
                          )}
                          {!probed.supports_tools && !probed.supports_latin && !probed.supports_devanagari && (
                            <span className="text-[10.5px] text-[rgb(var(--foreground-muted))]/60 font-mono">Standard LLM</span>
                          )}
                        </>
                      ) : (
                        <span className="text-[11px] text-[rgb(var(--foreground-muted))]/50 italic">Capabilities not yet probed</span>
                      )}
                    </div>

                    {/* Right: Context Window, TPS & Probe action */}
                    <div className="flex items-center gap-2 shrink-0">
                      {probed?.context_window ? (
                        <Tooltip label={`Context Window: ${probed.context_window.toLocaleString()} tokens`}>
                          <span className="text-[11px] font-mono px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground))]/85 border border-[rgba(var(--foreground),0.06)] flex items-center gap-1 font-medium">
                            <span>🧠</span>
                            <span>
                              {probed.context_window >= 1000000
                                ? `${(probed.context_window / 1000000).toFixed(1)}M`
                                : `${Math.round(probed.context_window / 1024)}k`}
                            </span>
                          </span>
                        </Tooltip>
                      ) : probed ? (
                        <Tooltip label="Context window is managed by the remote endpoint (no client clamp)">
                          <span className="text-[10.5px] font-mono px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--foreground),0.05)]">
                            Managed
                          </span>
                        </Tooltip>
                      ) : null}

                      {probed?.tps && (
                        <Tooltip label={`Response Speed: ${probed.tps.toFixed(1)} tokens/sec`}>
                          <span className="text-[11px] font-mono font-bold text-emerald-400 px-1.5 py-0.5 rounded bg-emerald-500/10 border border-emerald-500/20 flex items-center gap-1">
                            <span>⚡</span>
                            <span>{probed.tps.toFixed(0)} tps</span>
                          </span>
                        </Tooltip>
                      )}

                      {!probed && !isTesting && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleProbeCapabilities(model.id);
                          }}
                          className="text-[11px] font-bold text-[rgb(var(--accent))] px-2.5 py-0.5 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 hover:bg-[rgb(var(--accent))]/20 hover:border-[rgb(var(--accent))]/40 transition-all flex items-center gap-1 cursor-pointer"
                          title="Run quick capability test (tools, speed, scripts)"
                        >
                          <Sparkles size={11} />
                          <span>Test</span>
                        </button>
                      )}
                    </div>
                  </div>
                </button>
              );
            })
          )}
        </div>

        {/* Custom Model ID field */}
        <div className="mt-3 pt-3 border-t border-[rgba(var(--foreground),0.06)] space-y-2">
          <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))]/80 uppercase tracking-wider block">
            Use Custom Model ID
          </span>
          <div className="flex gap-2">
            <div className="flex-1 border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
              <input
                type="text"
                value={customModelId}
                onChange={(e) => {
                  setCustomModelId(e.target.value);
                }}
                placeholder="e.g. gemini-2.5-pro"
                className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
              />
            </div>
            <button
              type="button"
              onClick={handleValidateCustomModel}
              disabled={!customModelId.trim() || customModelStatus === "checking"}
              className={cn(
                "px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all border shrink-0 cursor-pointer",
                customModelStatus === "checking" && "bg-[rgba(var(--foreground),0.05)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]",
                customModelStatus === "valid" && "bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20",
                customModelStatus === "invalid" && "bg-amber-500/10 border-amber-500/20 text-amber-400 hover:bg-amber-500/20",
                customModelStatus === "idle" && "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] border-[rgba(var(--accent),0.2)] hover:scale-[1.02] active:scale-95"
              )}
            >
              {customModelStatus === "checking" && "Checking..."}
              {customModelStatus === "valid" && "Valid ✓"}
              {customModelStatus === "invalid" && "Not Listed ⚠"}
              {customModelStatus === "idle" && "Validate & Use"}
            </button>
          </div>
          {customModelStatus === "invalid" && (
            <div className="text-[11px] text-amber-400/80 leading-normal flex items-start gap-1">
              <span>⚠</span>
              <span>Model ID not in standard server list. Selected in draft anyway, but verify spelling.</span>
            </div>
          )}
          {customModelStatus === "valid" && (
            <div className="text-[11px] text-emerald-400/80 leading-normal flex items-start gap-1">
              <span>✓</span>
              <span>Model verified successfully! Selected and ready to save.</span>
            </div>
          )}
        </div>
      </div>
    );
  }

  // Model Tab: Local GGUF Model Grid
  return (
    <div className={cn("grid gap-2.5", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
      {(modelCatalog?.llm || []).map((model) => {
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
            onSelect={() => updateDraft("llm", "model", model.id)}
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
