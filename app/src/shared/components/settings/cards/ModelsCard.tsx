import { useState, useEffect, useCallback, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { 
  Brain, Volume2, Database, Trash2,
  Languages, Activity, Sparkles, Check, ArrowLeft,
  Download, RefreshCw, Info
} from "lucide-react";
import { cn } from "@/shared/lib/utils";

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
`;

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  void layoutMode;
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "translit" | "llm" | "tts">("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [outdatedModels, setOutdatedModels] = useState<string[]>([]);
  const [manifest, setManifest] = useState<VoxManifest | null>(null);

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

  const renderDeleteControl = (modelId: string, className?: string, isPurgeLink = false) => {
    if (confirmDeleteId === modelId) {
      return (
        <div className="flex items-center gap-1.5 transition-all duration-300">
          <span className="text-[11px] text-[rgb(var(--foreground-muted))]/80 uppercase font-bold tracking-wider mr-1">Confirm?</span>
          <button 
            onClick={(e) => {
              e.stopPropagation();
              deleteModel(modelId);
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/35 transition-colors border border-[rgb(var(--accent))]/30 flex items-center justify-center"
            aria-label="Yes, Delete"
          >
            <Check size={11} className="font-bold" />
          </button>
          <button 
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--foreground))]/[0.05] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.08] transition-colors border border-[rgba(var(--border),0.1)] flex items-center justify-center"
            aria-label="Cancel"
          >
            <ArrowLeft size={11} />
          </button>
        </div>
      );
    }

    if (isPurgeLink) {
      return (
        <button 
          onClick={(e) => {
            e.stopPropagation();
            setConfirmDeleteId(modelId);
          }}
          className="text-[rgb(var(--accent))] hover:text-[rgb(var(--accent))]/85 text-[11px] font-bold uppercase tracking-wider transition-colors"
        >
          Purge
        </button>
      );
    }

    if (className === "icon-only") {
      return (
        <button 
          onClick={(e) => {
            e.stopPropagation();
            setConfirmDeleteId(modelId);
          }}
          className="p-1.5 rounded-lg bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/20 hover:bg-[rgb(var(--accent))]/25 transition-colors"
          aria-label="Purge Weights"
        >
          <Trash2 size={12} />
        </button>
      );
    }

    return (
      <button 
        onClick={(e) => {
          e.stopPropagation();
          setConfirmDeleteId(modelId);
        }}
        className="px-3 py-1.5 rounded-xl bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/20 hover:bg-[rgb(var(--accent))]/20 transition-all font-bold uppercase tracking-wider text-[11px]"
      >
        Delete Weights
      </button>
    );
  };

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
      <div className="absolute top-0.5 right-0.5 p-0.5 rounded-full bg-black/60 backdrop-blur-sm z-10">
        <Icon size={9} className={colorClass} style={{ animationDuration: isMissing ? "2s" : "4s" }} />
      </div>
    );
  };

  const renderSubTabHeader = () => (
    <div className="flex glass-whisper glass-base p-0.5 rounded-xl mb-2.5 shrink-0">
      <button
        onClick={() => setActiveCategoryTab("model")}
        className={cn(
          "py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300 text-center w-1/2",
          activeCategoryTab === "model" 
            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
            : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
        )}
      >
        Model
      </button>
      <button
        onClick={() => setActiveCategoryTab("settings")}
        className={cn(
          "py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300 text-center w-1/2",
          activeCategoryTab === "settings" 
            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
            : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
        )}
      >
        Settings
      </button>
    </div>
  );

  return (
    <div className="w-full lg:w-[520px] lg:h-[330px] flex flex-col bg-transparent lg:bg-black/15 lg:backdrop-blur-md border-0 lg:border border-[rgba(var(--accent),0.10)] rounded-none lg:rounded-2xl p-0 lg:p-5 shadow-none lg:shadow-xl shadow-black/30 text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none">
      <style>{pulseStyles}</style>
      <div className="flex flex-col gap-4">
        
        {/* Header */}
        <div className="flex items-center gap-2 mb-1 shrink-0">
          <Database className="text-[rgb(var(--accent))]" size={16} />
          <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
            Model Hub
          </span>
        </div>

        {/* Topology Pipeline Map */}
        <div className="grid grid-cols-5 gap-1 shrink-0 p-1 rounded-xl glass-whisper glass-base overflow-visible mb-1">
          
          {/* NODE 1: VAD */}
          <button
            onClick={() => setActivePipelineTab("vad")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "vad"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.15)] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isVadCategoryMissing, hasVadUpdate)
            )}
          >
            {renderOverlayIcon(isVadCategoryMissing, hasVadUpdate)}
            <Activity size={14} className={cn("transition-colors shrink-0", activePipelineTab === "vad" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
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
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.15)] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isAsrCategoryMissing, hasAsrUpdate)
            )}
          >
            {renderOverlayIcon(isAsrCategoryMissing, hasAsrUpdate)}
            <Sparkles size={14} className={cn("transition-colors shrink-0", activePipelineTab === "asr" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
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
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.15)] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isTranslitCategoryMissing, hasTranslitUpdate)
            )}
          >
            {renderOverlayIcon(isTranslitCategoryMissing, hasTranslitUpdate)}
            <Languages size={14} className={cn("transition-colors shrink-0", activePipelineTab === "translit" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
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
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.15)] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isLlmCategoryMissing, hasLlmUpdate)
            )}
          >
            {renderOverlayIcon(isLlmCategoryMissing, hasLlmUpdate)}
            <Brain size={14} className={cn("transition-colors shrink-0", activePipelineTab === "llm" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
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
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.15)] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              getPulseClass(isTtsCategoryMissing, hasTtsUpdate)
            )}
          >
            {renderOverlayIcon(isTtsCategoryMissing, hasTtsUpdate)}
            <Volume2 size={14} className={cn("transition-colors shrink-0", activePipelineTab === "tts" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">Voice</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isTtsVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

        </div>

        {/* Workspace Detail Panel */}
        <div className="flex-1 flex flex-col glass-whisper glass-base rounded-xl p-3 relative min-h-0 overflow-y-auto custom-scrollbar">
          
          {/* TAB 1: SILENCE DETECTION (VAD) */}
          {activePipelineTab === "vad" && (
            <div className="space-y-3">
              {renderSubTabHeader()}

              {activeCategoryTab === "model" ? (
                <div className="grid grid-cols-2 gap-3">
                  {/* Option 1: Earshot */}
                  <div 
                    onClick={() => updateDraft("vad", "vad_backend", "earshot")}
                    className={cn(
                      "p-3 rounded-lg border transition-all duration-300 cursor-pointer flex flex-col justify-between h-28 glass-whisper glass-base",
                      activeVadBackend === "earshot" && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                    )}
                  >
                    <div>
                      <div className="text-[12px] font-bold text-[rgb(var(--foreground))] flex items-center justify-between">
                        <span>Earshot (Built-in)</span>
                        {activeVadBackend === "earshot" ? (
                          <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded border border-[rgb(var(--accent))]/10">Active</span>
                        ) : (
                          <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75 bg-[rgb(var(--foreground))]/5 px-1.5 py-0.5 rounded">Ready</span>
                        )}
                      </div>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 mt-1 leading-normal">
                        Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load.
                      </p>
                    </div>
                  </div>

                  {/* Option 2: TenVAD */}
                  <div 
                    onClick={() => updateDraft("vad", "vad_backend", "ten_vad")}
                    className={cn(
                      "p-3 rounded-lg border transition-all duration-300 cursor-pointer flex flex-col justify-between h-28 glass-whisper glass-base",
                      activeVadBackend === "ten_vad" && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                    )}
                  >
                    <div>
                      <div className="text-[12px] font-bold text-[rgb(var(--foreground))] flex items-center justify-between">
                        <span className="flex items-center gap-1.5">
                          <span>TenVAD Engine</span>
                          {outdatedModels.includes("ten_vad") && (
                            <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-1 py-0.5 rounded border border-[rgb(var(--accent))]/20 animate-pulse">Update Available</span>
                          )}
                        </span>
                        {activeVadBackend === "ten_vad" ? (
                          modelPresence["ten_vad"] ? (
                            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded border border-[rgb(var(--accent))]/10">Active</span>
                          ) : (
                            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]/75 bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded border border-[rgb(var(--accent))]/10">Missing</span>
                          )
                        ) : (
                          modelPresence["ten_vad"] ? (
                            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75 bg-[rgb(var(--foreground))]/5 px-1.5 py-0.5 rounded">Ready</span>
                          ) : (
                            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))]/75 bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded border border-[rgb(var(--accent))]/10">Missing</span>
                          )
                        )}
                      </div>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 mt-1 leading-normal">
                        ONNX-based voice detector. Requires downloading auxiliary neural files.
                      </p>
                    </div>
                    
                    {activeVadBackend === "ten_vad" && !modelPresence["ten_vad"] && (
                      <div className="flex items-center justify-between mt-1 pt-1.5 border-t border-[rgba(var(--border),0.05)]">
                        <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 font-bold uppercase tracking-wider">Deploy Weights</span>
                        {downloadStatuses["ten_vad"] ? (
                          <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["ten_vad"].progress)}%</span>
                        ) : (
                          <button 
                            onClick={(e) => { e.stopPropagation(); startDownload("ten_vad"); }}
                            className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shadow"
                          >
                            Get
                          </button>
                        )}
                      </div>
                    )}
                  </div>
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
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                              : "glass-whisper glass-base text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
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
                {[...modelCatalog.asr].sort((a, b) => {
                  if (draftSettings.asr.model === a.id) return -1;
                  if (draftSettings.asr.model === b.id) return 1;
                  return 0;
                }).map((model) => {
                  const isSelected = draftSettings.asr.model === model.id;
                  const modelGroupId = model.id;
                  const isDownloaded = modelPresence[modelGroupId];
                  const status = downloadStatuses[modelGroupId];

                  return (
                    <div 
                      key={model.id}
                      onClick={() => updateDraft("asr", "model", model.id)}
                      className={cn(
                        "p-3 rounded-lg border transition-all duration-300 cursor-pointer flex flex-col justify-between gap-2.5 glass-whisper glass-base",
                        isSelected && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                      )}
                    >
                      <div className="space-y-0.5">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">{model.name}</span>
                          <span className="text-[11px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded font-normal">{model.parameters}</span>
                          {outdatedModels.includes(modelGroupId) && (
                            <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-1 py-0.5 rounded border border-[rgb(var(--accent))]/20 animate-pulse">Update</span>
                          )}
                        </div>
                        <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal">
                          {model.description}
                        </p>
                      </div>

                      <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.05)] h-6">
                        <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 font-bold uppercase tracking-wider">
                          {isSelected ? "Active Pipeline" : "Ready"}
                        </span>
                        <div className="flex items-center gap-2">
                          {isDownloaded ? (
                            <>
                              <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded border border-[rgb(var(--accent))]/10">
                                Ready
                              </span>
                              {!isGroupRequired(model.id) && (
                                renderDeleteControl(modelGroupId, "icon-only")
                              )}
                            </>
                          ) : (
                            status ? (
                              <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(status.progress)}%</span>
                            ) : (
                              <button 
                                onClick={(e) => { e.stopPropagation(); startDownload(modelGroupId); }}
                                className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shadow"
                              >
                                Get
                              </button>
                            )
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* TAB 3: ROMAN TRANSLITERATION */}
          {activePipelineTab === "translit" && (
            <div className="space-y-4">
              <div className="p-3 rounded-lg border glass-whisper glass-base space-y-3">
                <div className="flex justify-between items-start">
                  <div>
                    <div className="text-[12px] font-bold text-[rgb(var(--foreground))] flex items-center gap-1.5">
                      <span>Vox Hinglish RNN</span>
                      {outdatedModels.includes("vox_translit_rnn") && (
                        <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-1 py-0.5 rounded border border-[rgb(var(--accent))]/20 animate-pulse">Update</span>
                      )}
                    </div>
                    <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 mt-1 leading-normal">
                      Converts Devanagari (Hindi) scripts dynamically to natural Hinglish phonetic spelling (~18MB).
                    </p>
                  </div>
                  <span className={cn(
                    "text-[11px] font-bold uppercase tracking-wider px-2 py-0.5 rounded border",
                    isTranslitVerified ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 border-[rgb(var(--accent))]/10" : "text-[rgb(var(--accent))]/75 bg-[rgb(var(--accent))]/5 border-[rgb(var(--accent))]/10"
                  )}>
                    {isTranslitVerified ? "Ready" : "Missing"}
                  </span>
                </div>

                <div className="flex justify-end gap-2 pt-2 border-t border-[rgba(var(--border),0.05)]">
                  {!isTranslitVerified ? (
                    downloadStatuses["vox_translit_rnn"] ? (
                      <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["vox_translit_rnn"].progress)}%</span>
                    ) : (
                      <button 
                        onClick={() => startDownload("vox_translit_rnn")}
                        className="px-3 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shadow transition-all"
                      >
                        Download
                      </button>
                    )
                  ) : (
                    renderDeleteControl("vox_translit_rnn")
                  )}
                </div>
              </div>
            </div>
          )}

          {/* TAB 4: AI REASONING (LLM) */}
          {activePipelineTab === "llm" && (
            <div className="space-y-4">
              {renderSubTabHeader()}

              {activeCategoryTab === "model" ? (
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
                      <div 
                        key={model.id}
                        onClick={() => updateDraft("llm", "model", model.id)}
                        className={cn(
                          "p-3 rounded-lg border transition-all duration-300 cursor-pointer flex flex-col justify-between gap-2.5 glass-whisper glass-base relative",
                          isSelected && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                        )}
                      >
                        <div className="space-y-0.5 relative pr-5">
                          <div className="flex items-center gap-2 flex-wrap">
                            <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">{model.name}</span>
                            <span className="text-[11px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-1.5 py-0.5 rounded font-normal">{model.parameters}</span>
                            {outdatedModels.includes(modelGroupId) && (
                              <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-1 py-0.5 rounded border border-[rgb(var(--accent))]/20 animate-pulse">Update</span>
                            )}
                          </div>
                          <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal">
                            {model.description}
                          </p>
                          
                          {/* Hover Tooltip for tradeoffs */}
                          {model.tradeoffs && (
                            <div className="absolute top-0.5 right-0 group/tooltip">
                              <Info size={12} className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-help" />
                              <div className="absolute right-full mr-2 -top-1 hidden group-hover/tooltip:block w-44 p-2 rounded-lg bg-black/95 border border-[rgba(var(--accent),0.2)] text-[11px] text-[rgb(var(--foreground))]/90 shadow-xl leading-normal z-55">
                                {model.tradeoffs}
                              </div>
                            </div>
                          )}
                        </div>

                        <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.05)] h-6">
                          <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 font-bold uppercase tracking-wider">
                            {isSelected ? "Active Pipeline" : "Ready"}
                          </span>
                          <div className="flex items-center gap-2">
                            {isDownloaded ? (
                              <>
                                <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded border border-[rgb(var(--accent))]/10">
                                  Ready
                                </span>
                                {!isGroupRequired(model.id) && (
                                  renderDeleteControl(modelGroupId, "icon-only")
                                )}
                              </>
                            ) : (
                              status ? (
                                <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(status.progress)}%</span>
                              ) : (
                                <button 
                                  onClick={(e) => { e.stopPropagation(); startDownload(modelGroupId); }}
                                  className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shadow"
                                >
                                  Get
                                </button>
                              )
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
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
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                              : "glass-whisper glass-base text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
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
                                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                                  : "glass-whisper glass-base text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
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
              {renderSubTabHeader()}

              {activeCategoryTab === "model" ? (
                <div className="grid grid-cols-1 gap-3">
                  {/* Supertonic 3 Multilingual */}
                  <div className="p-3 rounded-lg border glass-whisper glass-base flex flex-col justify-between h-28">
                    <div>
                      <div className="flex justify-between items-center">
                        <span className="text-[12px] font-bold text-[rgb(var(--foreground))] flex items-center gap-1.5">
                          <span>Supertonic 3 Multilingual</span>
                          {outdatedModels.includes("supertonic_tts") && (
                            <span className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 px-1 py-0.5 rounded border border-[rgb(var(--accent))]/20 animate-pulse">Update</span>
                          )}
                        </span>
                        <span className={cn("w-1.5 h-1.5 rounded-full", modelPresence["supertonic_tts"] ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30")} />
                      </div>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 mt-1 leading-normal">
                        31-language neural speech synthesis with 10 voices (~144MB INT8 quantized).
                      </p>
                    </div>

                    <div className="flex justify-between items-center pt-1.5 border-t border-[rgba(var(--border),0.05)]">
                      <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 font-bold uppercase tracking-wider">Deploy Weights</span>
                      {!modelPresence["supertonic_tts"] ? (
                        downloadStatuses["supertonic_tts"] ? (
                          <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["supertonic_tts"].progress)}%</span>
                        ) : (
                          <button onClick={() => startDownload("supertonic_tts")} className="px-2.5 py-0.5 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shadow">Get</button>
                        )
                      ) : (
                        renderDeleteControl("supertonic_tts", undefined, true)
                      )}
                    </div>
                  </div>
                </div>
              ) : (
                /* TTS Settings */
                <div className="grid grid-cols-2 gap-4 p-1">
                  {/* Left Column: Quality & Speed */}
                  <div className="space-y-3">
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
                                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                                : "glass-whisper glass-base text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
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
                  </div>

                  {/* Right Column: Voice Profile */}
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
                                  : "glass-whisper border-transparent text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.1)]"
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
