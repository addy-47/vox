import { memo } from "react";
import { useSettingsStore, type LlmModelInfo, type ModelCapabilities } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { Loader2, Network, RefreshCw, AlertCircle, Sparkles, Check } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

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
  provider: any;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  handleProbeCapabilities: (id?: string) => void;
  customModelId: string;
  setCustomModelId: (id: string) => void;
  customModelStatus: 'idle' | 'checking' | 'valid' | 'invalid';
  handleValidateCustomModel: () => void;
  activeCategoryTab?: "model" | "settings";
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
  activeCategoryTab = "model",
}: LlmCatalogViewProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const llmSettings = useSettingsStore((s) => s.draftSettings?.llm);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!llmSettings) return null;

  // Settings Tab: Memory Context Tokens & Processor Threads
  if (activeCategoryTab === "settings") {
    const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
    const maxSafe = Math.max(2, totalCores - 2);
    const threadPresets = (() => {
      const base = [2, 4];
      if (maxSafe > 4 && maxSafe !== totalCores) return [...base, maxSafe, totalCores];
      if (maxSafe > 4) return [...base, maxSafe];
      return base;
    })();

    return (
      <div className="space-y-4 p-1">
        {/* Memory Context Tokens */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">
              Memory Context Tokens
            </span>
            <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">
              {llmSettings.ctx_size}
            </span>
          </div>
          <div className="flex gap-1">
            {[512, 1024, 2048, 4096, 8192].map((val) => (
              <button
                key={val}
                onClick={() => updateDraft("llm", "ctx_size", val)}
                className={cn(
                  "flex-1 py-1 rounded-lg text-[12px] font-bold uppercase tracking-wider transition-all duration-300",
                  llmSettings.ctx_size === val
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                    : "glass text-[rgb(var(--foreground))] border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20 hover:text-[rgb(var(--accent))]"
                )}
              >
                {val < 1024 ? val : `${val / 1024}k`}
              </button>
            ))}
          </div>
        </div>

        {/* Processor Threads */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">
              Processor Threads
            </span>
            <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">
              {llmSettings.threads}
            </span>
          </div>
          <div className="flex gap-1">
            {threadPresets.map((val) => (
              <button
                key={val}
                onClick={() => updateDraft("llm", "threads", val)}
                className={cn(
                  "flex-1 py-1 rounded-lg text-[12px] font-bold uppercase tracking-wider transition-all duration-300",
                  llmSettings.threads === val
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                    : "glass text-[rgb(var(--foreground))] border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20 hover:text-[rgb(var(--accent))]"
                )}
              >
                {val}
                {val === maxSafe && val !== totalCores ? " (max)" : ""}
                {val === totalCores && val !== maxSafe ? " (all)" : ""}
              </button>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Model Tab: Remote / OpenAI-Compat Server Catalog
  if (isRemoteLlm) {
    return (
      <div className="space-y-3 p-3 rounded-2xl bg-[rgba(var(--foreground),0.015)] border border-[rgba(var(--foreground),0.02)] hover:border-[rgba(var(--accent),0.1)] transition-all duration-300 w-full animate-fade-in">
        {/* Connected Server Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-bold text-[rgb(var(--foreground))]/90 text-[13px] flex items-center gap-1.5">
              <Network size={16} className="text-[rgb(var(--accent))]" /> Connected Server
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 font-mono mt-0.5">
              {provider?.base_url || "No server configured"}
            </span>
          </div>
          {loadingRemoteModels ? (
            <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1">
              <RefreshCw size={14} className="animate-spin" /> Fetching...
            </span>
          ) : (
            <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))]/60">
              {remoteModels.length} models available
            </span>
          )}
        </div>

        {remoteModelsError && (
          <div className="text-[12px] font-bold text-red-400/80 bg-red-400/5 border border-red-400/15 rounded-xl px-3 py-2 flex items-center gap-2">
            <AlertCircle size={16} />
            <span>{remoteModelsError}</span>
          </div>
        )}

        {/* Remote Models List */}
        <div
          className={cn(
            "grid grid-cols-1 gap-2 pr-1",
            layoutMode === "small" ? "max-h-none overflow-y-visible" : "max-h-[220px] overflow-y-auto custom-scrollbar"
          )}
        >
          {remoteModels.length === 0 ? (
            <div className="text-center py-6 text-[12px] text-[rgb(var(--foreground-muted))]/70">
              No remote models loaded. Ensure the server is online and configured in the Interaction Card.
            </div>
          ) : (
            remoteModels.map((model) => {
              const isSelected = provider?.model === model.id;
              const probed = probingMap[model.id]?.capabilities || model.capabilities;
              const isTesting = probingMap[model.id]?.status === "testing";
              const isGpu = probed?.is_gpu_accelerated;

              return (
                <button
                  key={model.id}
                  type="button"
                  onClick={() => {
                    updateDraft("llm", "provider", {
                      ...provider,
                      model: model.id,
                    });
                    if (!probed && !isTesting) {
                      handleProbeCapabilities(model.id);
                    }
                  }}
                  className={cn(
                    "w-full text-left p-3 rounded-xl border transition-all duration-300 flex items-center justify-between gap-3 relative overflow-hidden cursor-pointer",
                    isGpu ? "border-purple-500/50 shadow-[0_0_12px_rgba(168,85,247,0.2)]" : "",
                    isSelected
                      ? "bg-[rgba(var(--accent),0.05)] border-[rgb(var(--accent))]"
                      : "bg-[rgba(var(--foreground),0.01)] border-[rgba(var(--foreground),0.04)] hover:border-[rgba(var(--accent),0.2)]"
                  )}
                >
                  <div className="flex-1 space-y-1.5 min-w-0">
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <span className="font-bold text-[rgb(var(--foreground))]/90 text-[12px] truncate">
                        {model.name}
                      </span>
                      {model.quantization && (
                        <span className="text-[11px] font-bold font-mono px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground))]/70 border border-[rgba(var(--foreground),0.04)] leading-none">
                          {model.quantization}
                        </span>
                      )}
                      {model.family && (
                        <span className="text-[11px] font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.08)] leading-none">
                          {model.family}
                        </span>
                      )}
                      {isGpu ? (
                        <Tooltip label={probed?.gpu_status || "GPU Offloaded"}>
                          <span className="text-[11px] font-bold font-mono px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-300 border border-purple-500/30 leading-none flex items-center gap-1">
                            🚀 GPU {probed?.vram_bytes ? `(${(probed.vram_bytes / (1024 * 1024)).toFixed(0)}MB)` : ""}
                          </span>
                        </Tooltip>
                      ) : probed?.server_has_gpu ? (
                        <Tooltip label="Server has GPU hardware, but model is running in CPU mode">
                          <span className="text-[11px] font-bold font-mono px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 leading-none flex items-center gap-1">
                            ⚠️ GPU Server (CPU)
                          </span>
                        </Tooltip>
                      ) : null}
                    </div>

                    <div className="flex items-center gap-2 text-[11px] text-[rgb(var(--foreground-muted))]/70">
                      <span className="font-mono truncate">{model.id}</span>
                      {model.size_bytes !== null && model.size_bytes !== undefined && (
                        <>
                          <span>•</span>
                          <span>{(model.size_bytes / (1024 * 1024 * 1024)).toFixed(2)} GB</span>
                        </>
                      )}
                    </div>

                    {/* Capability Badges & Readouts */}
                    <div className="flex items-center gap-1.5 flex-wrap pt-0.5">
                      {isTesting ? (
                        <span className="text-[11px] font-bold text-[rgb(var(--accent))] flex items-center gap-1">
                          <Loader2 size={10} className="animate-spin" />
                          Testing capabilities...
                        </span>
                      ) : probed ? (
                        <>
                          {probed.supports_tools && (
                            <Tooltip label="Can use tools">
                              <span className="text-[11px] font-bold px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 flex items-center gap-1">
                                🛠️ Tools
                              </span>
                            </Tooltip>
                          )}
                          {probed.supports_latin && (
                            <Tooltip label="Latin Script (EN)">
                              <span className="text-[11px] font-mono font-bold px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                                EN
                              </span>
                            </Tooltip>
                          )}
                          {probed.supports_devanagari && (
                            <Tooltip label="Devanagari Script (Hindi/Hinglish)">
                              <span className="text-[11px] font-mono font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                                DEV
                              </span>
                            </Tooltip>
                          )}
                          {probed.context_window && (
                            <Tooltip label="Memory size">
                              <span className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-zinc-800/60 text-zinc-300 border border-zinc-700/50">
                                🧠{" "}
                                {probed.context_window >= 1000000
                                  ? `${(probed.context_window / 1000000).toFixed(1)}M ctx`
                                  : `${Math.round(probed.context_window / 1024)}k ctx`}
                              </span>
                            </Tooltip>
                          )}
                          {probed.tps && (
                            <Tooltip label="Response speed">
                              <span className="text-[11px] font-mono text-emerald-400 font-bold">
                                ⚡ {probed.tps.toFixed(1)} tps
                              </span>
                            </Tooltip>
                          )}
                        </>
                      ) : (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleProbeCapabilities(model.id);
                          }}
                          className="text-[11px] font-bold text-[rgb(var(--accent))] hover:underline flex items-center gap-1"
                        >
                          <Sparkles size={10} /> Test Capabilities
                        </button>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center gap-1.5 shrink-0 ml-auto">
                    {isSelected && (
                      <div className="w-5 h-5 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] flex items-center justify-center">
                        <Check size={16} strokeWidth={3} />
                      </div>
                    )}
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
                "px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all border shrink-0",
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
