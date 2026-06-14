import { useState, useEffect, useCallback, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { 
  Brain, Volume2, Database, Trash2,
  Languages, Activity, Sparkles, Check, ArrowLeft,
  Download, RefreshCw, Info, AlertCircle, Network
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { RemoteModelInfo } from "@/store/settingsStore";

const PRICING_MAP: Record<string, string> = {
  "gpt-4o-mini": "$0.15 / $0.60",
  "gpt-4o": "$2.50 / $10.00",
  "gpt-4-turbo": "$10.00 / $30.00",
  "gemini-1.5-flash": "$0.075 / $0.30",
  "gemini-1.5-pro": "$1.25 / $5.00",
  "gemini-2.0-flash": "$0.075 / $0.30",
  "gemini-2.5-flash": "$0.075 / $0.30",
  "claude-3-5-sonnet": "$3.00 / $15.00",
  "claude-3-5-haiku": "$0.80 / $4.00",
  "claude-3-opus": "$15.00 / $75.00",
  "llama-3.3-70b": "$0.59 / $0.79",
  "llama3-8b": "$0.05 / $0.08",
  "mixtral-8x7b": "$0.24 / $0.24",
};

interface ModelStatus {
  step: 'idle' | 'downloading' | 'extracting' | 'verifying' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  bytesDownloaded: number;
  totalBytes: number;
  error?: string;
}

interface ModelEntry {
  id: string;
  path: string;
  size: number;
  required: boolean;
}

interface ModelGroup {
  id: string;
  name: string;
  category: string;
  version: string;
  files: ModelEntry[];
}

interface VoxManifest {
  models_version: string;
  release_notes?: string[];
  total_size_bytes: number;
  model_groups: ModelGroup[];
}

const pulseStyles = `
@keyframes premium-pulse-red {
  0%, 100% { border-color: rgba(239, 68, 68, 0.25); box-shadow: 0 0 4px rgba(239, 68, 68, 0.15); }
  50% { border-color: rgba(239, 68, 68, 0.75); box-shadow: 0 0 12px rgba(239, 68, 68, 0.4); }
}
@keyframes premium-pulse-purple {
  0%, 100% { border-color: rgba(168, 85, 247, 0.25); box-shadow: 0 0 4px rgba(168, 85, 247, 0.15); }
  50% { border-color: rgba(168, 85, 247, 0.75); box-shadow: 0 0 12px rgba(168, 85, 247, 0.4); }
}
.pulse-missing {
  animation: premium-pulse-red 2s infinite ease-in-out;
  border-width: 1px !important;
}
.pulse-update {
  animation: premium-pulse-purple 2s infinite ease-in-out;
  border-width: 1px !important;
}
.tooltip-container:hover .tooltip-content {
  display: block !important;
}
`;

interface SubModelCardProps {
  id: string;
  name: string;
  description: string;
  parameters: string;
  ramUsage?: string;
  tradeoffs?: string;
  isDownloaded: boolean;
  isActive: boolean;
  isRequired: boolean;
  layoutMode: "full-max" | "full-min" | "small";
  onSelect: () => void;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  downloadStatus?: { step: string; progress: number };
  startDownload: () => void;
  deleteModel: () => void;
  showTooltip?: boolean;
}

const SubModelCard: React.FC<SubModelCardProps> = ({
  id,
  name,
  description,
  parameters,
  ramUsage,
  tradeoffs,
  isDownloaded,
  isActive,
  isRequired,
  onSelect,
  confirmDeleteId,
  setConfirmDeleteId,
  downloadStatus,
  startDownload,
  deleteModel,
  showTooltip = false,
}) => {
  const isConfirmingDelete = confirmDeleteId === id;

  const renderAction = () => {
    if (!isDownloaded) {
      if (downloadStatus) {
        return (
          <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold shrink-0">
            {Math.round(downloadStatus.progress)}%
          </span>
        );
      }
      return (
        <button
          onClick={(e) => {
            e.stopPropagation();
            startDownload();
          }}
          className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shrink-0 hover:scale-[1.02] active:scale-95 transition-all"
        >
          Get
        </button>
      );
    }

    if (isRequired) return null;

    if (isConfirmingDelete) {
      return (
        <div className="flex items-center gap-1 transition-all duration-300 shrink-0">
          <span className="text-[10px] text-red-500 font-bold uppercase tracking-wider mr-0.5">Delete?</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              deleteModel();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-red-500/20 text-red-500 hover:bg-red-500/35 transition-colors border border-red-500/30 flex items-center justify-center"
            aria-label="Confirm Delete"
          >
            <Check size={14} className="font-bold" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--foreground))]/[0.05] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.08] transition-colors border border-[rgba(var(--border),0.1)] flex items-center justify-center"
            aria-label="Cancel"
          >
            <ArrowLeft size={14} />
          </button>
        </div>
      );
    }

    return (
      <button
        onClick={(e) => {
          e.stopPropagation();
          setConfirmDeleteId(id);
        }}
        className="p-1.5 rounded-lg bg-red-500/10 text-red-500 border border-red-500/20 hover:bg-red-500/20 hover:border-red-500/30 transition-colors shrink-0"
        aria-label="Delete weights"
      >
        <Trash2 size={16} />
      </button>
    );
  };

  const hasTooltip = showTooltip && !!(description || parameters || ramUsage || tradeoffs);

  return (
    <div
      onClick={() => {
        if (isDownloaded && !isActive) {
          onSelect();
        }
      }}
      className={cn(
        "p-4 rounded-lg border transition-all duration-300 flex flex-col justify-between gap-2.5 glass min-h-[105px]",
        isDownloaded && !isActive && "cursor-pointer hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.02)]",
        isActive && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
      )}
    >
      {/* Top Section */}
      <div className="space-y-0.5">
        <div className="flex items-start justify-between gap-2">
          <span className="text-[12px] font-bold text-[rgb(var(--foreground))] truncate max-w-[170px]" title={name}>
            {name}
          </span>
          
          {hasTooltip && (
            <div className="relative tooltip-container inline-block shrink-0 mt-0.5">
              <Info size={16} className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-help" />
              <div className="absolute right-full top-0 mr-2 hidden tooltip-content w-52 p-2.5 rounded-lg bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.25)] text-[11px] text-[rgb(var(--foreground-muted))]/80 shadow-2xl leading-normal z-50">
                <div className="space-y-1">
                  <div className="flex justify-between border-b border-[rgba(var(--accent),0.06)] pb-0.5 mb-1 font-bold">
                    <span className="text-[9px] text-[rgb(var(--accent))] uppercase tracking-wider">Specs</span>
                    <span className="font-mono text-[9px] text-[rgb(var(--foreground-muted))]/60">{parameters}</span>
                  </div>
                  {description && <div className="text-[10px] text-[rgb(var(--foreground))]/80 leading-normal mb-1">{description}</div>}
                  {ramUsage && (
                    <div className="text-[9px] text-[rgb(var(--foreground-muted))]/70 font-mono">
                      RAM: {ramUsage}
                    </div>
                  )}
                  {tradeoffs && (
                    <div className="text-[9px] text-[rgb(var(--foreground-muted))]/70 italic border-t border-[rgba(var(--accent),0.06)] pt-1 mt-1 leading-normal">
                      {tradeoffs}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Subtext metadata */}
        {description && (
          !showTooltip ? (
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal">
              {description}
              {ramUsage && ` · RAM: ${ramUsage}`}
              {parameters && ` · ${parameters}`}
            </p>
          ) : (
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal line-clamp-2">
              {description}
            </p>
          )
        )}
      </div>

      {/* Bottom Section */}
      <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.05)] h-6 shrink-0">
        <span className={cn(
          "text-[11px] font-bold uppercase tracking-wider",
          isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/70"
        )}>
          {isActive ? "Active" : "Ready"}
        </span>
        {renderAction()}
      </div>
    </div>
  );
};

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  void layoutMode;
  const { settings, draftSettings, updateDraft, modelCatalog } = useSettings();
  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "translit" | "llm" | "tts">("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [outdatedModels, setOutdatedModels] = useState<string[]>([]);
  const [manifest, setManifest] = useState<VoxManifest | null>(null);

  // Remote LLM models catalog live state
  const [remoteModels, setRemoteModels] = useState<RemoteModelInfo[]>([]);
  const [loadingRemoteModels, setLoadingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);

  // Fix: Base layout decisions on committed settings to prevent uncommitted leaks
  const savedProvider = settings?.llm?.provider;
  const isRemoteLlm = savedProvider?.kind === "open_ai_compat";
  const provider = (draftSettings?.llm?.provider?.kind === savedProvider?.kind)
    ? draftSettings?.llm?.provider
    : savedProvider;



  const [customModelId, setCustomModelId] = useState("");
  const [customModelStatus, setCustomModelStatus] = useState<'idle' | 'valid' | 'invalid' | 'checking'>('idle');

  const getFilteredModels = useCallback(() => {
    if (!provider || !provider.provider_name) return remoteModels;
    const name = provider.provider_name.toLowerCase();
    
    let filtered: RemoteModelInfo[] = [];
    if (remoteModels && remoteModels.length > 0) {
      if (name.includes("openai")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("gpt") && 
          !m.id.toLowerCase().includes("instruct") && 
          !m.id.toLowerCase().includes("embedding") && 
          !m.id.toLowerCase().includes("audio")
        );
      } else if (name.includes("gemini") || name.includes("google")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("gemini") && 
          !m.id.toLowerCase().includes("embedding")
        );
      } else if (name.includes("anthropic")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("claude")
        );
      } else if (name.includes("groq")) {
        filtered = remoteModels.filter(m => 
          (m.id.toLowerCase().includes("llama") || m.id.toLowerCase().includes("mixtral") || m.id.toLowerCase().includes("gemma")) && 
          !m.id.toLowerCase().includes("whisper")
        );
      } else {
        filtered = [...remoteModels];
      }

      // Sort: newer versions first (e.g. 2.5 > 2.0 > 1.5)
      filtered.sort((a, b) => {
        const aId = a.id.toLowerCase();
        const bId = b.id.toLowerCase();
        
        // Put experimental or preview models at the bottom
        const aExp = aId.includes("exp") || aId.includes("preview");
        const bExp = bId.includes("exp") || bId.includes("preview");
        if (aExp && !bExp) return 1;
        if (!aExp && bExp) return -1;

        // Compare numbers if present
        const aNum = parseFloat(aId.match(/\d+(\.\d+)?/)?.[0] || "0");
        const bNum = parseFloat(bId.match(/\d+(\.\d+)?/)?.[0] || "0");
        if (bNum !== aNum) {
          return bNum - aNum; // Higher version number first
        }
        return aId.localeCompare(bId);
      });

      // Strip models/ prefix from names
      return filtered.map(m => ({
        ...m,
        name: m.name.replace(/^models\//, "")
      })).slice(0, 4);
    }
    return [];
  }, [provider?.provider_name, remoteModels]);

  const getModelPricing = useCallback((modelId: string): string | null => {
    const idLower = modelId.toLowerCase().replace(/^models\//, "");
    for (const key of Object.keys(PRICING_MAP)) {
      if (idLower.includes(key)) {
        return PRICING_MAP[key];
      }
    }
    return null;
  }, []);

  useEffect(() => {
    if (provider?.model) {
      const filtered = getFilteredModels();
      const isFiltered = filtered.some(m => m.id === provider.model);
      if (!isFiltered) {
        setCustomModelId(provider.model);
      } else {
        setCustomModelId("");
      }
    } else {
      setCustomModelId("");
    }
    setCustomModelStatus('idle');
  }, [provider?.base_url, provider?.provider_name, provider?.model, remoteModels, getFilteredModels]);

  const handleValidateCustomModel = useCallback(() => {
    if (!customModelId.trim() || !provider) return;
    setCustomModelStatus('checking');
    const modelToUse = customModelId.trim();
    
    const exists = remoteModels.some(m => m.id.toLowerCase() === modelToUse.toLowerCase());
    if (exists) {
      setCustomModelStatus('valid');
      updateDraft("llm", "provider", {
        ...provider,
        model: modelToUse
      });
    } else {
      if (remoteModels.length > 0) {
        setCustomModelStatus('invalid');
        updateDraft("llm", "provider", {
          ...provider,
          model: modelToUse
        });
      } else {
        setCustomModelStatus('valid');
        updateDraft("llm", "provider", {
          ...provider,
          model: modelToUse
        });
      }
    }
  }, [customModelId, provider, remoteModels, updateDraft]);

  useEffect(() => {
    if (activePipelineTab === "llm" && activeCategoryTab === "model" && isRemoteLlm && provider) {
      const fetchRemoteModels = async () => {
        setLoadingRemoteModels(true);
        setRemoteModelsError(null);
        try {
          const list = await invoke<RemoteModelInfo[]>("list_remote_llm_models", {
            provider
          });
          setRemoteModels(list);
        } catch (err) {
          console.error(err);
          setRemoteModelsError("Failed to fetch remote models list");
        } finally {
          setLoadingRemoteModels(false);
        }
      };
      fetchRemoteModels();
    }
  }, [activePipelineTab, activeCategoryTab, isRemoteLlm, provider?.base_url, provider?.api_key, provider?.provider_name]);

  const getGroupIdForFile = useCallback((fileId: string): string => {
    if (!manifest) {
      if (fileId.startsWith("vad")) return "ten_vad";
      if (fileId.startsWith("translit")) return "vox_translit_rnn";
      if (fileId.startsWith("stt_nemotron")) return "nvidia_nemotron";
      if (fileId.startsWith("stt_")) return "qwen3_asr";
      if (fileId.startsWith("tts_supertonic")) return "supertonic_tts";
      return fileId;
    }
    for (const group of manifest.model_groups) {
      if (group.id === fileId || group.files.some(f => f.id === fileId)) {
        return group.id;
      }
    }
    return fileId;
  }, [manifest]);

  const isGroupRequired = useCallback((groupId: string): boolean => {
    if (!manifest) return groupId === "ten_vad" || groupId === "vox_translit_rnn" || groupId === "qwen3_asr" || groupId === "nvidia_nemotron";
    const group = manifest.model_groups.find(g => g.id === groupId);
    return group ? group.files.some(f => f.required) : false;
  }, [manifest]);



  const checkOutdated = useCallback(async () => {
    try {
      const res = await invoke<any>("check_for_model_updates");
      if (res && res.update_available) {
        setOutdatedModels(res.outdated_models);
      } else {
        setOutdatedModels([]);
      }
    } catch (e) {
      console.warn("Failed to check outdated models:", e);
    }
  }, []);

  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const presence: Record<string, boolean> = {};

    checkOutdated();

    const groups = manifest?.model_groups || [];
    const checkIds = groups.length > 0 
      ? groups.map(g => g.id)
      : [
          "ten_vad",
          "vox_translit_rnn",
          "qwen3_asr",
          "nvidia_nemotron",
          "gemma_4_reasoning",
          "llama_3_2_reasoning",
          "gemma_4_uncensored",
          "supertonic_tts"
        ];

    for (const id of checkIds) {
      try {
        const exists = await invoke<boolean>("check_model_exists", { modelId: id });
        presence[id] = exists;
      } catch (err) {
        presence[id] = false;
      }
    }

    presence["earshot"] = true;
    setModelPresence(presence);
  }, [modelCatalog, draftSettings, checkOutdated, manifest]);

  useEffect(() => {
    const loadManifest = async () => {
      try {
        const data = await invoke<VoxManifest>("fetch_manifest");
        setManifest(data);
      } catch (err) {
        console.error("Failed to fetch manifest:", err);
      }
    };
    loadManifest();
  }, []);

  useEffect(() => {
    setActiveCategoryTab("model");
  }, [activePipelineTab]);

  useEffect(() => {
    checkPresence();

    const unlistenStatus = listen<{
      model_id: string;
      step: string;
      progress: number;
      bytes_downloaded: number;
      total_bytes: number;
      error?: string;
    }>("model_setup_status", (event) => {
      const fileId = event.payload.model_id;
      const groupId = getGroupIdForFile(fileId);
      setDownloadStatuses(prev => ({
        ...prev,
        [groupId]: {
          step: event.payload.step as any,
          progress: event.payload.progress,
          bytesDownloaded: event.payload.bytes_downloaded,
          totalBytes: event.payload.total_bytes,
          error: event.payload.error
        }
      }));
    });

    const unlistenComplete = listen<string>("optional_model_complete", (event) => {
      checkPresence();
      setDownloadStatuses(prev => {
        const next = { ...prev };
        delete next[event.payload];
        return next;
      });
    });

    return () => {
      unlistenStatus.then(u => u());
      unlistenComplete.then(u => u());
    };
  }, [checkPresence, getGroupIdForFile]);

  if (!draftSettings || !modelCatalog) return null;

  const startDownload = (modelId: string) => {
    setDownloadStatuses(prev => ({
      ...prev,
      [modelId]: { step: 'idle', progress: 0, bytesDownloaded: 0, totalBytes: 0 }
    }));
    invoke("download_optional_model", { modelId });
  };

  const deleteModel = async (modelId: string) => {
    try {
      await invoke("delete_model", { modelId });
      checkPresence();
    } catch (err) {
      console.error("Failed to delete model:", err);
    }
  };

  const activeVadBackend = draftSettings.vad.vad_backend;
  const isVadVerified = activeVadBackend === "earshot" || modelPresence["ten_vad"];

  const selectedAsrId = draftSettings.asr.model;
  const isAsrVerified = modelPresence[selectedAsrId];

  const isTranslitVerified = modelPresence["vox_translit_rnn"];

  const selectedLlmId = draftSettings.llm.model;
  const isLlmDownloaded = modelPresence[selectedLlmId];

  const isTtsVerified = modelPresence["supertonic_tts"];

  const isVadCategoryMissing = activeVadBackend === "ten_vad" && !modelPresence["ten_vad"];
  const isAsrCategoryMissing = !modelPresence[selectedAsrId];
  const isTranslitCategoryMissing = !modelPresence["vox_translit_rnn"];
  const isLlmCategoryMissing = !modelPresence[selectedLlmId];
  const isTtsCategoryMissing = !modelPresence["supertonic_tts"];

  const hasVadUpdate = outdatedModels.includes("ten_vad");
  const hasAsrUpdate = outdatedModels.includes(selectedAsrId);
  const hasTranslitUpdate = outdatedModels.includes("vox_translit_rnn");
  const hasLlmUpdate = outdatedModels.includes(selectedLlmId);
  const hasTtsUpdate = outdatedModels.includes("supertonic_tts");

  const getPulseClass = (isMissing: boolean, hasUpdate: boolean) => {
    if (isMissing) return "pulse-missing border-red-500/35";
    if (hasUpdate) return "pulse-update border-purple-500/35";
    return "";
  };

  const renderOverlayIcon = (isMissing: boolean, hasUpdate: boolean) => {
    if (!isMissing && !hasUpdate) return null;
    const Icon = isMissing ? Download : RefreshCw;
    const colorClass = isMissing ? "text-[rgb(var(--accent))]/75 animate-bounce" : "text-[rgb(var(--accent))] animate-spin";
    return (
      <div className="absolute top-0.5 right-0.5 p-0.5 rounded-full bg-[rgba(var(--foreground),0.08)] dark:bg-[rgba(var(--foreground),0.2)] backdrop-blur-sm z-10">
        <Icon size={12} className={colorClass} style={{ animationDuration: isMissing ? "2s" : "4s" }} />
      </div>
    );
  };


  return (
    <div className={cn(
      "w-full h-auto flex flex-col text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
      layoutMode === "small"
        ? "bg-transparent p-0"
        : cn(
            "glass-card p-5",
            layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
          )
    )}>
      <style>{pulseStyles}</style>
      <div className="flex flex-col gap-4">
        
        {/* Header */}
        <div className="flex items-center justify-between mb-1 shrink-0">
          <div className="flex items-center gap-2">
            <Database className="text-[rgb(var(--accent))]" size={20} />
            <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
              Model Hub
            </span>
          </div>
          {/* Small Category Tabs */}
          {(activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (
            <div className="flex glass p-0.5 rounded-lg border border-[rgba(var(--accent),0.08)]">
              <button
                onClick={() => setActiveCategoryTab("model")}
                className={cn(
                  "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                  activeCategoryTab === "model"
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                    : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                )}
              >
                Model
              </button>
              <button
                onClick={() => setActiveCategoryTab("settings")}
                className={cn(
                  "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                  activeCategoryTab === "settings"
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                    : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                )}
              >
                Settings
              </button>
            </div>
          )}
        </div>

        {/* Topology Pipeline Map */}
        <div className="grid grid-cols-5 gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-1 bg-[rgba(var(--foreground),0.02)]">
          
          {/* NODE 1: VAD */}
          <button
            onClick={() => setActivePipelineTab("vad")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "vad"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isVadCategoryMissing, hasVadUpdate)
            )}
          >
            {renderOverlayIcon(isVadCategoryMissing, hasVadUpdate)}
            <Activity size={18} className={cn("transition-colors shrink-0", activePipelineTab === "vad" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">Silence</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isVadVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 2: ASR */}
          <button
            onClick={() => setActivePipelineTab("asr")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "asr"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isAsrCategoryMissing, hasAsrUpdate)
            )}
          >
            {renderOverlayIcon(isAsrCategoryMissing, hasAsrUpdate)}
            <Sparkles size={18} className={cn("transition-colors shrink-0", activePipelineTab === "asr" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">ASR</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isAsrVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 3: TRANSLIT */}
          <button
            onClick={() => setActivePipelineTab("translit")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "translit"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isTranslitCategoryMissing, hasTranslitUpdate)
            )}
          >
            {renderOverlayIcon(isTranslitCategoryMissing, hasTranslitUpdate)}
            <Languages size={18} className={cn("transition-colors shrink-0", activePipelineTab === "translit" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">Hinglish</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isTranslitVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 4: LLM */}
          <button
            onClick={() => setActivePipelineTab("llm")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "llm"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isLlmCategoryMissing, hasLlmUpdate)
            )}
          >
            {renderOverlayIcon(isLlmCategoryMissing, hasLlmUpdate)}
            <Brain size={18} className={cn("transition-colors shrink-0", activePipelineTab === "llm" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">LLM</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isLlmDownloaded ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 5: TTS */}
          <button
            onClick={() => setActivePipelineTab("tts")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "tts"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isTtsCategoryMissing, hasTtsUpdate)
            )}
          >
            {renderOverlayIcon(isTtsCategoryMissing, hasTtsUpdate)}
            <Volume2 size={18} className={cn("transition-colors shrink-0", activePipelineTab === "tts" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">Voice</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isTtsVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

        </div>

        {/* Workspace Detail Panel */}
        <div className="max-h-[190px] h-auto w-full flex flex-col glass rounded-xl p-3 relative overflow-y-auto custom-scrollbar bg-[rgba(var(--foreground),0.02)]">
                   {/* TAB 1: SILENCE DETECTION (VAD) */}
          {activePipelineTab === "vad" && (
            <div className="space-y-3">
              {activeCategoryTab === "model" ? (
                <div className="grid grid-cols-2 gap-3">
                  <SubModelCard
                    id="earshot"
                    name="Earshot (Built-in)"
                    description="Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load."
                    parameters="Built-in"
                    ramUsage="0 MB"
                    isDownloaded={true}
                    isActive={activeVadBackend === "earshot"}
                    isRequired={true}
                    layoutMode={layoutMode}
                    onSelect={() => updateDraft("vad", "vad_backend", "earshot")}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    startDownload={() => {}}
                    deleteModel={() => {}}
                  />
                  <SubModelCard
                    id="ten_vad"
                    name="TenVAD Engine"
                    description="ONNX-based voice detector. Requires downloading auxiliary neural files."
                    parameters="ONNX"
                    ramUsage="~2 MB"
                    isDownloaded={modelPresence["ten_vad"]}
                    isActive={activeVadBackend === "ten_vad"}
                    isRequired={false}
                    layoutMode={layoutMode}
                    onSelect={() => updateDraft("vad", "vad_backend", "ten_vad")}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    downloadStatus={downloadStatuses["ten_vad"]}
                    startDownload={() => startDownload("ten_vad")}
                    deleteModel={() => deleteModel("ten_vad")}
                  />
                </div>
              ) : (
                /* VAD Settings */
                <div className="space-y-4 p-1">
                  <div className="space-y-2">
                    <span className="text-[12px] text-[rgb(var(--foreground))] font-bold block">Silence Threshold</span>
                    <div className="flex gap-1">
                      {[
                        { label: "Sensitive", value: 0.3 },
                        { label: "Balanced", value: 0.5 },
                        { label: "Conservative", value: 0.7 },
                        { label: "Aggressive", value: 0.9 },
                      ].map(({ label, value }) => (
                        <button key={value} onClick={() => updateDraft("vad", "threshold", value)}
                          className={cn(
                            "flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                            Math.abs(draftSettings.vad.threshold - value) < 0.01
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                              : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                          )}
                        >{label}</button>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* TAB 2: VOICE RECOGNITION (ASR) */}
          {activePipelineTab === "asr" && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-2.5">
                {modelCatalog.asr.map((model) => {
                  const isSelected = draftSettings.asr.model === model.id;
                  const modelGroupId = model.id;
                  const isDownloaded = modelPresence[modelGroupId];
                  const status = downloadStatuses[modelGroupId];

                  return (
                    <SubModelCard
                      key={model.id}
                      id={modelGroupId}
                      name={model.name}
                      description={model.description}
                      parameters={model.parameters}
                      ramUsage={model.ram_usage}
                      tradeoffs={model.tradeoffs}
                      isDownloaded={isDownloaded}
                      isActive={isSelected}
                      isRequired={isGroupRequired(model.id)}
                      layoutMode={layoutMode}
                      onSelect={() => updateDraft("asr", "model", model.id)}
                      confirmDeleteId={confirmDeleteId}
                      setConfirmDeleteId={setConfirmDeleteId}
                      downloadStatus={status}
                      startDownload={() => startDownload(modelGroupId)}
                      deleteModel={() => deleteModel(modelGroupId)}
                    />
                  );
                })}
              </div>
            </div>
          )}

          {/* TAB 3: ROMAN TRANSLITERATION */}
          {activePipelineTab === "translit" && (
            <div className="space-y-4">
              <div className="grid grid-cols-1 gap-3">
                <SubModelCard
                  id="vox_translit_rnn"
                  name="Vox Hinglish RNN"
                  description="Converts Devanagari (Hindi) scripts dynamically to natural Hinglish phonetic spelling (~18MB)."
                  parameters="18 MB"
                  ramUsage="~18 MB"
                  isDownloaded={isTranslitVerified}
                  isActive={true}
                  isRequired={false}
                  layoutMode={layoutMode}
                  onSelect={() => {}}
                  confirmDeleteId={confirmDeleteId}
                  setConfirmDeleteId={setConfirmDeleteId}
                  downloadStatus={downloadStatuses["vox_translit_rnn"]}
                  startDownload={() => startDownload("vox_translit_rnn")}
                  deleteModel={() => deleteModel("vox_translit_rnn")}
                />
              </div>
            </div>
          )}

          {/* TAB 4: AI REASONING (LLM) */}
          {activePipelineTab === "llm" && (
            <div className="space-y-4">
              {activeCategoryTab === "model" ? (
                isRemoteLlm ? (
                  /* Remote Models Picker Panel */
                  <div className="space-y-3 p-3 rounded-2xl bg-[rgba(var(--foreground),0.015)] border border-[rgba(var(--foreground),0.02)] hover:border-[rgba(var(--accent),0.1)] transition-all duration-300 w-full animate-fade-in">
                    <div className="flex items-center justify-between">
                      <div className="flex flex-col">
                        <span className="font-bold text-[rgb(var(--foreground))]/90 text-[12px] flex items-center gap-1.5">
                          <Network size={16} className="text-[rgb(var(--accent))]" /> Connected Server
                        </span>
                        <span className="text-[10px] text-[rgb(var(--foreground-muted))]/70 font-mono mt-0.5">
                          {provider?.base_url || "No server configured"}
                        </span>
                      </div>
                      {loadingRemoteModels ? (
                        <span className="text-[10px] font-bold text-[rgb(var(--accent))] flex items-center gap-1">
                          <RefreshCw size={14} className="animate-spin" /> Fetching...
                        </span>
                      ) : (
                        <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))]/60">
                          {remoteModels.length} models available
                        </span>
                      )}
                    </div>

                    {remoteModelsError && (
                      <div className="text-[11px] font-bold text-red-400/80 bg-red-400/5 border border-red-400/15 rounded-xl px-3 py-2 flex items-center gap-2">
                        <AlertCircle size={16} />
                        <span>{remoteModelsError}</span>
                      </div>
                    )}

                    <div className="grid grid-cols-1 gap-2 max-h-[220px] overflow-y-auto pr-1">
                      {getFilteredModels().length === 0 ? (
                        <div className="text-center py-6 text-[11px] text-[rgb(var(--foreground-muted))]/70">
                          No remote models loaded. Ensure the server is online and configured in the Interaction Card.
                        </div>
                      ) : (
                        getFilteredModels().map((model) => {
                          const isSelected = provider?.model === model.id;
                          return (
                            <button
                              key={model.id}
                              onClick={() => {
                                updateDraft("llm", "provider", {
                                  ...provider,
                                  model: model.id,
                                });
                              }}
                              className={cn(
                                "w-full text-left p-3 rounded-xl border transition-all duration-300 flex items-center justify-between gap-3",
                                isSelected
                                  ? "bg-[rgba(var(--accent),0.05)] border-[rgb(var(--accent))]"
                                  : "bg-[rgba(var(--foreground),0.01)] border-[rgba(var(--foreground),0.04)] hover:border-[rgba(var(--accent),0.2)]"
                              )}
                            >
                              <div className="flex-1 space-y-1 min-w-0">
                                <div className="flex items-center gap-2 flex-wrap">
                                  <span className="font-bold text-[rgb(var(--foreground))]/90 text-[11px] truncate">
                                    {model.name}
                                  </span>
                                  {model.quantization && (
                                    <span className="text-[9px] font-bold font-mono px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground))]/70 border border-[rgba(var(--foreground),0.04)] leading-none">
                                      {model.quantization}
                                    </span>
                                  )}
                                  {model.family && (
                                    <span className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.08)] leading-none">
                                      {model.family}
                                    </span>
                                  )}
                                </div>
                                <div className="flex items-center gap-2 text-[10px] text-[rgb(var(--foreground-muted))]/70">
                                  <span className="font-mono truncate">{model.id}</span>
                                  {model.size_bytes !== null && model.size_bytes !== undefined && (
                                    <>
                                      <span>•</span>
                                      <span>{(model.size_bytes / (1024 * 1024 * 1024)).toFixed(2)} GB</span>
                                    </>
                                  )}
                                </div>
                              </div>

                              <div className="flex items-center gap-1.5 shrink-0 ml-auto">
                                {getModelPricing(model.id) && (
                                  <span className="text-[9px] font-mono font-bold px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--foreground),0.03)]" title="Prompt / Completion pricing per 1M tokens">
                                    {getModelPricing(model.id)}
                                  </span>
                                )}
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
                      <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))]/80 uppercase tracking-wider block">
                        Use Custom Model ID
                      </span>
                      <div className="flex gap-2">
                        <div className="flex-1 border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                          <input
                            type="text"
                            value={customModelId}
                            onChange={(e) => {
                              setCustomModelId(e.target.value);
                              setCustomModelStatus('idle');
                            }}
                            placeholder="e.g. gemini-2.5-pro"
                            className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                          />
                        </div>
                        <button
                          onClick={handleValidateCustomModel}
                          disabled={!customModelId.trim() || customModelStatus === 'checking'}
                          className={cn(
                            "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase tracking-wider transition-all border shrink-0",
                            customModelStatus === 'checking' && "bg-[rgba(var(--foreground),0.05)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]",
                            customModelStatus === 'valid' && "bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20",
                            customModelStatus === 'invalid' && "bg-amber-500/10 border-amber-500/20 text-amber-400 hover:bg-amber-500/20",
                            customModelStatus === 'idle' && "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] border-[rgba(var(--accent),0.2)] hover:scale-[1.02] active:scale-95"
                          )}
                        >
                          {customModelStatus === 'checking' && "Checking..."}
                          {customModelStatus === 'valid' && "Valid ✓"}
                          {customModelStatus === 'invalid' && "Not Listed ⚠"}
                          {customModelStatus === 'idle' && "Validate & Use"}
                        </button>
                      </div>
                      {customModelStatus === 'invalid' && (
                        <div className="text-[9px] text-amber-400/80 leading-normal flex items-start gap-1">
                          <span>⚠</span>
                          <span>Model ID not in standard server list. Selected in draft anyway, but verify spelling.</span>
                        </div>
                      )}
                      {customModelStatus === 'valid' && (
                        <div className="text-[9px] text-emerald-400/80 leading-normal flex items-start gap-1">
                          <span>✓</span>
                          <span>Model verified successfully! Selected and ready to save.</span>
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  /* Local GGUF Card Grid */
                  <div className="grid grid-cols-2 gap-2.5">
                    {[...modelCatalog.llm].sort((a, b) => {
                      if (selectedLlmId === a.id) return -1;
                      if (selectedLlmId === b.id) return 1;
                      return 0;
                    }).map((model) => {
                      const isSelected = selectedLlmId === model.id;
                      const modelGroupId = model.id;
                      const isDownloaded = modelPresence[modelGroupId];
                      const status = downloadStatuses[modelGroupId];

                      return (
                        <SubModelCard
                          key={model.id}
                          id={modelGroupId}
                          name={model.name}
                          description={model.description}
                          parameters={model.parameters}
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
                          startDownload={() => startDownload(modelGroupId)}
                          deleteModel={() => deleteModel(modelGroupId)}
                          showTooltip={true}
                        />
                      );
                    })}
                  </div>
                )
              ) : (
                /* LLM Settings */
                <div className="space-y-4 p-1">
                  {/* Context Size */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Memory Context Tokens</span>
                      <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.ctx_size}</span>
                    </div>
                    <div className="flex gap-1">
                      {[512, 1024, 2048, 4096, 8192].map(val => (
                        <button key={val} onClick={() => updateDraft("llm", "ctx_size", val)}
                          className={cn(
                            "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                            draftSettings.llm.ctx_size === val
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                              : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                          )}
                        >{val < 1024 ? val : `${val / 1024}k`}</button>
                      ))}
                    </div>
                  </div>

                  {/* Threads */}
                  {(() => {
                    const totalCores = (typeof navigator !== 'undefined' ? navigator.hardwareConcurrency : undefined) || 4;
                    const maxSafe = Math.max(2, totalCores - 2);
                    const threadPresets = (() => {
                      const base = [2, 4];
                      if (maxSafe > 4 && maxSafe !== totalCores) return [...base, maxSafe, totalCores];
                      if (maxSafe > 4) return [...base, maxSafe];
                      return base;
                    })();
                    return (
                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between">
                          <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Processor Threads</span>
                          <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.threads}</span>
                        </div>
                        <div className="flex gap-1">
                          {threadPresets.map(val => (
                            <button key={val} onClick={() => updateDraft("llm", "threads", val)}
                              className={cn(
                                "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                draftSettings.llm.threads === val
                                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                                  : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                              )}
                            >{val}{val === maxSafe && val !== totalCores ? " (max)" : ""}{val === totalCores && val !== maxSafe ? " (all)" : ""}</button>
                          ))}
                        </div>
                      </div>
                    );
                  })()}
                </div>
              )}
            </div>
          )}

          {/* TAB 5: VOICE SYNTHESIS (TTS) */}
          {activePipelineTab === "tts" && (
            <div className="space-y-4">
              {activeCategoryTab === "model" ? (
                <div className="grid grid-cols-1 gap-3">
                  <SubModelCard
                    id="supertonic_tts"
                    name="Supertonic 3 Multilingual"
                    description="31-language neural speech synthesis with 10 voices (~144MB INT8 quantized)."
                    parameters="144 MB"
                    ramUsage="~144 MB"
                    isDownloaded={isTtsVerified}
                    isActive={true}
                    isRequired={false}
                    layoutMode={layoutMode}
                    onSelect={() => {}}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    downloadStatus={downloadStatuses["supertonic_tts"]}
                    startDownload={() => startDownload("supertonic_tts")}
                    deleteModel={() => deleteModel("supertonic_tts")}
                  />
                </div>
              ) : (
                /* TTS Settings */
                <div className="flex flex-col gap-3.5 p-1">
                  {/* Quality Steps */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                      <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">
                        {draftSettings.tts.quality_steps <= 4 ? "Speed" : draftSettings.tts.quality_steps <= 8 ? "Quality" : "Best"}
                      </span>
                    </div>
                    <div className="flex gap-1">
                      {[2, 4, 6, 8, 10, 12].map(step => (
                        <button key={step} onClick={() => updateDraft("tts", "quality_steps", step)}
                          className={cn(
                            "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                            draftSettings.tts.quality_steps === step
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                              : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                          )}
                        >{step}</button>
                      ))}
                    </div>
                  </div>

                  {/* Speed */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Speed</span>
                      <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.tts.speed.toFixed(2)}x</span>
                    </div>
                    <input 
                      type="range" 
                      min="0.7" max="2.0" step="0.05"
                      value={draftSettings.tts.speed}
                      onChange={(e) => updateDraft("tts", "speed", Number(e.target.value))}
                      className="w-full h-1 bg-[rgba(var(--border),0.1)] rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]"
                    />
                  </div>

                  {/* Voice Profile */}
                  <div className="space-y-1.5">
                    <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]/75 block">Voice Profile</span>
                    {modelPresence["supertonic_tts"] ? (
                      <div className="grid grid-cols-2 gap-1 pr-0.5 max-h-[110px] overflow-y-auto custom-scrollbar">
                        {modelCatalog.voices.map((v) => {
                          const isSelected = draftSettings.tts.voice === v.id;
                          return (
                            <button
                              key={v.id}
                              onClick={() => updateDraft("tts", "voice", v.id)}
                              className={cn(
                                "py-1 px-2 rounded-lg text-left text-[11px] font-bold uppercase tracking-wider transition-all duration-300 border flex items-center justify-between",
                                isSelected
                                  ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                                  : "glass border-transparent text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.1)]"
                              )}
                            >
                              <span className="truncate mr-1">{v.name}</span>
                              {isSelected && <Check size={11} className="shrink-0" />}
                            </button>
                          );
                        })}
                      </div>
                    ) : (
                      <div className="flex items-center justify-center h-20 border border-dashed border-[rgba(var(--accent),0.15)] rounded-lg text-[rgb(var(--foreground-muted))]/60 text-[11px] font-bold uppercase tracking-wider text-center p-2 leading-tight">
                        Deploy TTS weights first
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          )}

        </div>

      </div>
    </div>
  );
});

ModelsCard.displayName = "ModelsCard";
