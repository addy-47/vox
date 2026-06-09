import React, { useState, useEffect, useCallback } from "react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { 
  Brain, Volume2, Database, Trash2,
  Wifi, Languages, 
  Activity, Sparkles, Shield, Check, ArrowLeft,
  Download, RefreshCw, Info
} from "lucide-react";

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
  0%, 100% { border-color: rgba(239, 68, 68, 0.2); box-shadow: 0 0 4px rgba(239, 68, 68, 0.1); }
  50% { border-color: rgba(239, 68, 68, 0.7); box-shadow: 0 0 12px rgba(239, 68, 68, 0.35); }
}
@keyframes premium-pulse-purple {
  0%, 100% { border-color: rgba(168, 85, 247, 0.2); box-shadow: 0 0 4px rgba(168, 85, 247, 0.1); }
  50% { border-color: rgba(168, 85, 247, 0.7); box-shadow: 0 0 12px rgba(168, 85, 247, 0.35); }
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

export const ModelSettings: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [promptTab, setPromptTab] = useState<"en" | "hi">("en");
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
          <span className="text-[11px] text-[rgb(var(--foreground-muted))] uppercase font-bold tracking-wider mr-1">Confirm?</span>
          <button 
            onClick={(e) => {
              e.stopPropagation();
              deleteModel(modelId);
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/35 transition-colors border border-red-500/30 flex items-center justify-center"
            title="Yes, Delete"
          >
            <Check size={12} className="font-bold" />
          </button>
          <button 
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--foreground))]/[0.05] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.08] transition-colors border border-[rgba(var(--border),0.1)] flex items-center justify-center"
            title="Cancel"
          >
            <ArrowLeft size={12} />
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
          className="text-red-400 hover:text-red-500 text-[13px] font-bold uppercase tracking-wider transition-colors"
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
          className="p-2 rounded-lg bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/25 transition-colors"
          title="Purge Weights"
        >
          <Trash2 size={14} />
        </button>
      );
    }

    return (
      <button 
        onClick={(e) => {
          e.stopPropagation();
          setConfirmDeleteId(modelId);
        }}
        className="px-4 py-2 rounded-xl bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 transition-all font-bold uppercase tracking-wider text-[13px]"
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

  // Check local model existence dynamically
  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const presence: Record<string, boolean> = {};

    // Trigger outdated model checking
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

    presence["earshot"] = true; // Always verified
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

  // Reset to Model tab when switching categories
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

  // VAD logic
  const activeVadBackend = draftSettings.vad.vad_backend;
  const isVadVerified = activeVadBackend === "earshot" || modelPresence["ten_vad"];

  // ASR logic
  const selectedAsrId = draftSettings.asr.model;
  const isAsrVerified = modelPresence[selectedAsrId];

  // Translit logic
  const isTranslitVerified = modelPresence["vox_translit_rnn"];

  // LLM logic
  const selectedLlmId = draftSettings.llm.model;
  const isLlmDownloaded = modelPresence[selectedLlmId];

  // TTS logic
  const isTtsVerified = modelPresence["supertonic_tts"];

  // Highlights state for Topology Pipeline Map
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
    if (isMissing) return "pulse-missing border-red-500/30";
    if (hasUpdate) return "pulse-update border-purple-500/30";
    return "";
  };

  const renderOverlayIcon = (isMissing: boolean, hasUpdate: boolean) => {
    if (!isMissing && !hasUpdate) return null;
    const Icon = isMissing ? Download : RefreshCw;
    const colorClass = isMissing ? "text-red-400 animate-bounce" : "text-purple-400 animate-spin";
    return (
      <div className="absolute top-1 right-1 p-0.5 rounded-full bg-black/40 backdrop-blur-sm z-10">
        <Icon size={10} className={colorClass} style={{ animationDuration: isMissing ? "2s" : "4s" }} />
      </div>
    );
  };

  const renderSubTabHeader = () => (
    <div className="flex bg-[rgb(var(--foreground))]/[0.03] p-0.5 rounded-xl border border-[rgba(var(--border),0.06)] mb-4">
      <button
        onClick={() => setActiveCategoryTab("model")}
        className={cn(
          "py-2 rounded-lg text-[13px] font-bold uppercase tracking-wider transition-all duration-300 text-center w-1/2",
          activeCategoryTab === "model" 
            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
        )}
      >
        Model
      </button>
      <button
        onClick={() => setActiveCategoryTab("settings")}
        className={cn(
          "py-2 rounded-lg text-[13px] font-bold uppercase tracking-wider transition-all duration-300 text-center w-1/2",
          activeCategoryTab === "settings" 
            ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
        )}
      >
        Settings
      </button>
    </div>
  );

  return (
    <div className="h-full overflow-y-auto lg:overflow-hidden custom-scrollbar pr-1 -mr-1 select-none pb-10">
      <style>{pulseStyles}</style>
      <div className="lg:h-full flex flex-col lg:grid lg:grid-cols-12 gap-6 items-stretch">
        
        {/* Left Column: Interactive Topology Pipeline Selector */}
        <div className="lg:col-span-7 flex flex-col lg:min-h-0">
          <div className="premium-card p-4 sm:p-6 lg:p-8 flex flex-col lg:h-full lg:min-h-0 relative overflow-hidden">
            
            {/* Header */}
            <div className="flex items-center justify-between mb-6 shrink-0">
              <div className="flex items-center gap-3">
                <Database className="text-[rgb(var(--accent))]" size={22} />
                <div className="space-y-0.5">
                  <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Model Hub</h2>
                  <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-wider font-bold opacity-80">Manage Speech Pipeline Models</p>
                </div>
              </div>
            </div>

            {/* Topology Pipeline Map */}
            <div className="grid grid-cols-6 lg:grid-cols-5 gap-2 lg:gap-3 mb-6 shrink-0 relative p-1.5 rounded-2xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.06)] overflow-visible">
              
              {/* NODE 1: VAD */}
              <button
                onClick={() => setActivePipelineTab("vad")}
                className={cn(
                  "col-span-2 lg:col-span-1 p-2.5 lg:p-4 rounded-xl flex flex-row lg:flex-col items-center justify-center gap-2 border text-center transition-all duration-300 relative group overflow-hidden",
                  activePipelineTab === "vad"
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)] scale-[1.02]"
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                  getPulseClass(isVadCategoryMissing, hasVadUpdate)
                )}
              >
                {renderOverlayIcon(isVadCategoryMissing, hasVadUpdate)}
                <Activity size={16} className={cn("transition-colors shrink-0", activePipelineTab === "vad" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]")} />
                <span className="text-[11px] sm:text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Silence</span>
                <span className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0 lg:mt-1",
                  isVadVerified ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500 shadow-[0_0_8px_#EF4444]"
                )} />
              </button>

              {/* NODE 2: ASR */}
              <button
                onClick={() => setActivePipelineTab("asr")}
                className={cn(
                  "col-span-2 lg:col-span-1 p-2.5 lg:p-4 rounded-xl flex flex-row lg:flex-col items-center justify-center gap-2 border text-center transition-all duration-300 relative group overflow-hidden",
                  activePipelineTab === "asr"
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)] scale-[1.02]"
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                  getPulseClass(isAsrCategoryMissing, hasAsrUpdate)
                )}
              >
                {renderOverlayIcon(isAsrCategoryMissing, hasAsrUpdate)}
                <Sparkles size={16} className={cn("transition-colors shrink-0", activePipelineTab === "asr" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]")} />
                <span className="text-[11px] sm:text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">ASR</span>
                <span className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0 lg:mt-1",
                  isAsrVerified ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500 shadow-[0_0_8px_#EF4444]"
                )} />
              </button>

              {/* NODE 3: TRANSLIT */}
              <button
                onClick={() => setActivePipelineTab("translit")}
                className={cn(
                  "col-span-2 lg:col-span-1 p-2.5 lg:p-4 rounded-xl flex flex-row lg:flex-col items-center justify-center gap-2 border text-center transition-all duration-300 relative group overflow-hidden",
                  activePipelineTab === "translit"
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)] scale-[1.02]"
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                  getPulseClass(isTranslitCategoryMissing, hasTranslitUpdate)
                )}
              >
                {renderOverlayIcon(isTranslitCategoryMissing, hasTranslitUpdate)}
                <Languages size={16} className={cn("transition-colors shrink-0", activePipelineTab === "translit" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]")} />
                <span className="text-[11px] sm:text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Hinglish</span>
                <span className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0 lg:mt-1",
                  isTranslitVerified ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500 shadow-[0_0_8px_#EF4444]"
                )} />
              </button>

              {/* NODE 4: LLM */}
              <button
                onClick={() => setActivePipelineTab("llm")}
                className={cn(
                  "col-span-3 lg:col-span-1 p-2.5 lg:p-4 rounded-xl flex flex-row lg:flex-col items-center justify-center gap-2 border text-center transition-all duration-300 relative group overflow-hidden",
                  activePipelineTab === "llm"
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)] scale-[1.02]"
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                  getPulseClass(isLlmCategoryMissing, hasLlmUpdate)
                )}
              >
                {renderOverlayIcon(isLlmCategoryMissing, hasLlmUpdate)}
                <Brain size={16} className={cn("transition-colors shrink-0", activePipelineTab === "llm" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]")} />
                <span className="text-[11px] sm:text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">LLM</span>
                <span className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0 lg:mt-1",
                  isLlmDownloaded ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500 shadow-[0_0_8px_#EF4444]"
                )} />
              </button>

              {/* NODE 5: TTS */}
              <button
                onClick={() => setActivePipelineTab("tts")}
                className={cn(
                  "col-span-3 lg:col-span-1 p-2.5 lg:p-4 rounded-xl flex flex-row lg:flex-col items-center justify-center gap-2 border text-center transition-all duration-300 relative group overflow-hidden",
                  activePipelineTab === "tts"
                    ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)] scale-[1.02]"
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
                  getPulseClass(isTtsCategoryMissing, hasTtsUpdate)
                )}
              >
                {renderOverlayIcon(isTtsCategoryMissing, hasTtsUpdate)}
                <Volume2 size={16} className={cn("transition-colors shrink-0", activePipelineTab === "tts" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--foreground))]")} />
                <span className="text-[11px] sm:text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Voice</span>
                <span className={cn(
                  "w-1.5 h-1.5 rounded-full shrink-0 lg:mt-1",
                  isTtsVerified ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500 shadow-[0_0_8px_#EF4444]"
                )} />
              </button>

            </div>

            {/* Direct Multi-Card/Grid Selector Workspace (Zero Jargon, Zero Ugly Select Boxes) */}
            <div className="lg:flex-1 lg:min-h-0 flex flex-col bg-[rgb(var(--foreground))]/[0.01] border border-[rgba(var(--border),0.06)] rounded-2xl p-4 sm:p-5 relative lg:overflow-y-auto lg:custom-scrollbar">
              
              {/* TAB 1: SILENCE DETECTION (VAD) */}
              {activePipelineTab === "vad" && (
                <div className="space-y-4">
                  {renderSubTabHeader()}

                  {activeCategoryTab === "model" ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      {/* Option 1: Earshot */}
                      <div 
                        onClick={() => updateDraft("vad", "vad_backend", "earshot")}
                        className={cn(
                          "p-4 rounded-xl border transition-all duration-300 cursor-pointer flex flex-col justify-between h-36 bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)]",
                          activeVadBackend === "earshot" && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                        )}
                      >
                        <div>
                          <div className="text-[13px] font-bold text-[rgb(var(--foreground))] flex items-center justify-between">
                            <span>Earshot (Built-in)</span>
                            {activeVadBackend === "earshot" ? (
                              <span className="text-[13px] font-bold uppercase tracking-wider text-emerald-400 bg-emerald-500/5 px-2 py-0.5 rounded border border-emerald-500/10">Active</span>
                            ) : (
                              <span className="text-[13px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] opacity-80 bg-[rgb(var(--foreground))]/5 px-2 py-0.5 rounded">Ready</span>
                            )}
                          </div>
                          <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-2 leading-relaxed">
                            Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load.
                          </p>
                        </div>
                      </div>

                      {/* Option 2: TenVAD */}
                      <div 
                        onClick={() => updateDraft("vad", "vad_backend", "ten_vad")}
                        className={cn(
                          "p-4 rounded-xl border transition-all duration-300 cursor-pointer flex flex-col justify-between h-36 bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)]",
                          activeVadBackend === "ten_vad" && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                        )}
                      >
                        <div>
                          <div className="text-[13px] font-bold text-[rgb(var(--foreground))] flex items-center justify-between">
                            <span className="flex items-center gap-1.5">
                              <span>TenVAD Engine</span>
                              {outdatedModels.includes("ten_vad") && (
                                <span className="text-[9px] font-black uppercase tracking-wider text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20 animate-pulse">Update Available</span>
                              )}
                            </span>
                            {activeVadBackend === "ten_vad" ? (
                              modelPresence["ten_vad"] ? (
                                <span className="text-[13px] font-bold uppercase tracking-wider text-emerald-400 bg-emerald-500/5 px-2 py-0.5 rounded border border-emerald-500/10">Active</span>
                              ) : (
                                <span className="text-[13px] font-bold uppercase tracking-wider text-red-400 bg-red-500/5 px-2 py-0.5 rounded border border-red-500/10">Missing</span>
                              )
                            ) : (
                              modelPresence["ten_vad"] ? (
                                <span className="text-[13px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] opacity-80 bg-[rgb(var(--foreground))]/5 px-2 py-0.5 rounded">Ready</span>
                              ) : (
                                <span className="text-[13px] font-bold uppercase tracking-wider text-red-400 bg-red-500/5 px-2 py-0.5 rounded border border-red-500/10">Missing</span>
                              )
                            )}
                          </div>
                          <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-2 leading-relaxed">
                            ONNX-based voice detector. Requires downloading auxiliary neural files.
                          </p>
                        </div>
                        
                        {activeVadBackend === "ten_vad" && !modelPresence["ten_vad"] && (
                          <div className="flex items-center justify-between mt-2 pt-2 border-t border-[rgba(var(--border),0.05)]">
                            <span className="text-[13px] text-[rgb(var(--foreground-muted))]">Deploy Weights</span>
                            {downloadStatuses["ten_vad"] ? (
                              <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["ten_vad"].progress)}%</span>
                            ) : (
                              <button 
                                onClick={(e) => { e.stopPropagation(); startDownload("ten_vad"); }}
                                className="px-3 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow"
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
                    <div className="space-y-4">
                      <div className="space-y-2">
                        <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Silence Threshold</span>
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
                                  : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/10"
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
                <div className="space-y-4">
                  <div className="grid grid-cols-1 gap-3.5">
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
                            "p-4 rounded-xl border transition-all duration-300 cursor-pointer flex flex-col sm:flex-row sm:items-center justify-between gap-4 bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)]",
                            isSelected && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                          )}
                        >
                          <div className="space-y-1 flex-1">
                            <div className="flex items-center gap-2 flex-wrap">
                              <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">{model.name}</span>
                              <span className="text-[13px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded font-normal">{model.parameters}</span>
                              {outdatedModels.includes(modelGroupId) && (
                                <span className="text-[9px] font-black uppercase tracking-wider text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20 animate-pulse">Update Available</span>
                              )}
                            </div>
                            <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-normal max-w-[420px]">
                              {model.description}
                            </p>
                          </div>

                          <div className="flex items-center gap-3 shrink-0 self-end sm:self-auto">
                            {isDownloaded ? (
                              <div className="flex items-center gap-3">
                                {isSelected ? (
                                  <span className="text-[13px] font-bold uppercase tracking-wider text-emerald-400 bg-emerald-500/5 px-2.5 py-1 rounded border border-emerald-500/10">Active</span>
                                ) : (
                                  <span className="text-[13px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] opacity-80 bg-[rgb(var(--foreground))]/5 px-2.5 py-1 rounded">Select</span>
                                )}
                                {!isGroupRequired(model.id) && (
                                  renderDeleteControl(modelGroupId, "icon-only")
                                )}
                              </div>
                            ) : (
                              status ? (
                                <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(status.progress)}%</span>
                              ) : (
                                <button 
                                  onClick={(e) => { e.stopPropagation(); startDownload(modelGroupId); }}
                                  className="px-3.5 py-1.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow"
                                >
                                  Get
                                </button>
                              )
                            )}
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
                  <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] space-y-4">
                    <div className="flex justify-between items-start">
                      <div>
                        <div className="text-[13px] font-bold text-[rgb(var(--foreground))] flex items-center gap-1.5">
                          <span>Vox Hinglish RNN</span>
                          {outdatedModels.includes("vox_translit_rnn") && (
                            <span className="text-[9px] font-black uppercase tracking-wider text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20 animate-pulse">Update Available</span>
                          )}
                        </div>
                        <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-1.5 leading-relaxed">
                          Converts Devanagari (Hindi) scripts dynamically to natural Hinglish phonetic spelling (~18MB).
                        </p>
                      </div>
                      <span className={cn(
                        "text-[13px] font-bold uppercase tracking-wider px-2.5 py-0.5 rounded border",
                        isTranslitVerified ? "text-emerald-400 bg-emerald-500/5 border-emerald-500/10" : "text-red-400 bg-red-500/5 border-red-500/10"
                      )}>
                        {isTranslitVerified ? "Ready" : "Missing"}
                      </span>
                    </div>

                    <div className="flex justify-end gap-3 pt-3 border-t border-[rgba(var(--border),0.05)]">
                      {!isTranslitVerified ? (
                        downloadStatuses["vox_translit_rnn"] ? (
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["vox_translit_rnn"].progress)}%</span>
                        ) : (
                          <button 
                            onClick={() => startDownload("vox_translit_rnn")}
                            className="px-4 py-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow hover:scale-[1.02] transition-all"
                          >
                            Download Model
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
                    <div className="grid grid-cols-1 gap-3.5">
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
                              "p-4 rounded-xl border transition-all duration-300 cursor-pointer flex flex-col sm:flex-row sm:items-center justify-between gap-4 bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)]",
                              isSelected && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
                            )}
                          >
                            <div className="space-y-1 flex-1">
                              <div className="flex items-center gap-2 flex-wrap">
                                <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">{model.name}</span>
                                <span className="text-[13px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded font-normal">{model.parameters}</span>
                                {outdatedModels.includes(modelGroupId) && (
                                  <span className="text-[9px] font-black uppercase tracking-wider text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20 animate-pulse">Update Available</span>
                                )}
                              </div>
                              <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 leading-normal max-w-[420px]">
                                {model.description}
                              </p>
                              {model.tradeoffs && (
                                <div className="mt-1.5 flex items-start gap-1.5">
                                  <Info size={12} className="text-[rgb(var(--foreground-muted))] mt-0.5 shrink-0" />
                                  <p className="text-[12px] text-[rgb(var(--foreground-muted))] opacity-70 leading-relaxed">
                                    {model.tradeoffs}
                                  </p>
                                </div>
                              )}
                            </div>

                            <div className="flex items-center gap-3 shrink-0 self-end sm:self-auto">
                              {isDownloaded ? (
                                <div className="flex items-center gap-3">
                                  {isSelected ? (
                                    <span className="text-[13px] font-bold uppercase tracking-wider text-emerald-400 bg-emerald-500/5 px-2.5 py-1 rounded border border-emerald-500/10">Active</span>
                                  ) : (
                                    <span className="text-[13px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] opacity-80 bg-[rgb(var(--foreground))]/5 px-2.5 py-1 rounded">Select</span>
                                  )}
                                  {!isGroupRequired(model.id) && (
                                    renderDeleteControl(modelGroupId, "icon-only")
                                  )}
                                </div>
                              ) : (
                                status ? (
                                  <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(status.progress)}%</span>
                                ) : (
                                  <button 
                                    onClick={(e) => { e.stopPropagation(); startDownload(modelGroupId); }}
                                    className="px-3.5 py-1.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow"
                                  >
                                    Get
                                  </button>
                                )
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ) : (
                    /* LLM Settings */
                    <div className="space-y-5">
                      {/* Context tokens — safe presets */}
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Memory Context Tokens</span>
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.ctx_size}</span>
                        </div>
                        <div className="flex gap-1">
                          {[512, 1024, 2048, 4096, 8192].map(val => (
                            <button key={val} onClick={() => updateDraft("llm", "ctx_size", val)}
                              className={cn(
                                "flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                draftSettings.llm.ctx_size === val
                                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                                  : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/10"
                              )}
                            >{val < 1024 ? val : val >= 1024 && val < 1024 * 1024 ? `${val / 1024}k` : `${val / 1024 / 1024}M`}</button>
                          ))}
                        </div>
                      </div>
                      {/* Processor Threads — CPU-aware safe presets */}
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
                          <div className="space-y-2">
                            <div className="flex items-center justify-between">
                              <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Processor Threads</span>
                              <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.threads}</span>
                            </div>
                            <div className="flex gap-1">
                              {threadPresets.map(val => (
                                <button key={val} onClick={() => updateDraft("llm", "threads", val)}
                                  className={cn(
                                    "flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                    draftSettings.llm.threads === val
                                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                                      : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/10"
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
                    <div className="grid grid-cols-1 gap-4">
                      {/* Supertonic 3 Multilingual */}
                      <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] flex flex-col justify-between h-36">
                        <div>
                          <div className="flex justify-between items-center">
                            <span className="text-[13px] font-bold text-[rgb(var(--foreground))] flex items-center gap-1.5">
                              <span>Supertonic 3 Multilingual</span>
                              {outdatedModels.includes("supertonic_tts") && (
                                <span className="text-[9px] font-black uppercase tracking-wider text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20 animate-pulse">Update Available</span>
                              )}
                            </span>
                            <span className={cn("w-2 h-2 rounded-full", modelPresence["supertonic_tts"] ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500")} />
                          </div>
                          <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-2">
                            31-language neural speech synthesis with 10 voices (~144MB INT8 quantized).
                          </p>
                        </div>

                        <div className="flex justify-between items-center pt-2 border-t border-[rgba(var(--border),0.05)]">
                          <span className="text-[13px] text-[rgb(var(--foreground-muted))]">Deploy Weights</span>
                          {!modelPresence["supertonic_tts"] ? (
                            downloadStatuses["supertonic_tts"] ? (
                              <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses["supertonic_tts"].progress)}%</span>
                            ) : (
                              <button onClick={() => startDownload("supertonic_tts")} className="px-3 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow">Get</button>
                            )
                          ) : (
                            renderDeleteControl("supertonic_tts", undefined, true)
                          )}
                        </div>
                      </div>
                    </div>
                  ) : (
                    /* TTS Settings */
                    <div className="space-y-4">
                      {/* Quality Steps */}
                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between">
                          <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">
                            {draftSettings.tts.quality_steps <= 4 ? "Speed" : draftSettings.tts.quality_steps <= 8 ? "Quality" : "Best"}
                          </span>
                        </div>
                        <div className="flex gap-1">
                          {[2, 4, 6, 8, 10, 12].map(step => (
                            <button key={step} onClick={() => updateDraft("tts", "quality_steps", step)}
                              className={cn(
                                "flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                draftSettings.tts.quality_steps === step
                                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
                                  : "bg-[rgb(var(--foreground))]/[0.03] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--border),0.05)] hover:bg-[rgb(var(--foreground))]/10"
                              )}
                            >{step}</button>
                          ))}
                        </div>
                        <div className="flex justify-between text-[11px] text-[rgb(var(--foreground-muted))] opacity-70">
                          <span>Speed</span>
                          <span>Quality</span>
                          <span>Best</span>
                        </div>
                      </div>
                      {/* Speed */}
                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between">
                          <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Speed</span>
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.tts.speed.toFixed(2)}x</span>
                        </div>
                        <input 
                          type="range" 
                          min="0.7" max="2.0" step="0.05"
                          value={draftSettings.tts.speed}
                          onChange={(e) => updateDraft("tts", "speed", Number(e.target.value))}
                          className="w-full"
                        />
                      </div>
                    </div>
                  )}
                </div>
              )}

            </div>

          </div>
        </div>

        {/* Right Column: Parameters Tuning & Compact System Prompts Switcher */}
        <div className="lg:col-span-5 flex flex-col gap-6 lg:min-h-0">
          
          {/* Connectivity Card (mock — cloud inference planned for v0.8.3+) */}
          <div className="premium-card p-4 sm:p-6 lg:p-8 flex flex-col gap-5 shrink-0">
            <div className="flex items-center gap-3 shrink-0">
              <Wifi className="text-[rgb(var(--accent))]" size={22} />
              <div className="space-y-0.5">
                <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Connectivity</h2>
                <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-wider font-bold opacity-80">Local vs Cloud Inference</p>
              </div>
            </div>

            <div className="space-y-4">
              {/* Local / Cloud Toggle (mock) */}
              <div className="flex items-center justify-between p-3.5 rounded-xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)]">
                <div className="space-y-0.5">
                  <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">Inference Mode</span>
                  <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-80 leading-normal">
                    Run models locally or route to cloud
                  </p>
                </div>
                <div className="flex bg-[rgb(var(--foreground))]/[0.05] p-0.5 rounded-lg border border-[rgba(var(--border),0.05)]">
                  <span className="px-2.5 py-1 rounded-md text-[11px] font-bold uppercase tracking-wider bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]">Local</span>
                  <span className="px-2.5 py-1 rounded-md text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] opacity-40">Cloud</span>
                </div>
              </div>

              {/* API Key Input (mock) */}
              <div className="p-3.5 rounded-xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)] space-y-2">
                <div className="space-y-0.5">
                  <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">API Key</span>
                  <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-80 leading-normal">
                    Cloud provider authentication
                  </p>
                </div>
                <input
                  type="password"
                  placeholder="sk-... (coming in v0.8.3+)"
                  disabled
                  className="w-full px-3 py-2 rounded-lg bg-[rgb(var(--foreground))]/[0.05] border border-[rgba(var(--border),0.08)] text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 disabled:opacity-50 focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all"
                />
              </div>

              <div className="h-px bg-[rgba(var(--border),0.05)]" />

              {/* Transliteration Enabled/Disabled Toggle */}
              <div className="flex items-center justify-between p-3.5 rounded-xl bg-[rgb(var(--foreground))]/[0.02] border border-[rgba(var(--border),0.05)]">
                <div className="space-y-0.5">
                  <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">Hinglish Transliteration</span>
                  <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-80 leading-normal">
                    Converts Hindi characters into English spelling syntax
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => updateDraft("asr", "transliterate_enabled", !draftSettings.asr.transliterate_enabled)}
                  className={cn(
                    "relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border border-[rgba(var(--border),0.15)] transition-colors duration-200 ease-in-out focus:outline-none shadow-sm",
                    draftSettings.asr.transliterate_enabled ? "bg-[rgb(var(--accent))]" : "bg-zinc-300 dark:bg-zinc-600"
                  )}
                  style={{
                    backgroundColor: draftSettings.asr.transliterate_enabled ? 'rgb(var(--accent))' : undefined
                  }}
                >
                  <span
                    className={cn(
                      "pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out",
                      draftSettings.asr.transliterate_enabled ? "translate-x-5" : "translate-x-0"
                    )}
                  />
                </button>
              </div>

            </div>
          </div>

          {/* Prompts Section with Switcher and Balanced Height Textarea */}
          <div className="premium-card p-4 sm:p-6 lg:p-4 flex flex-col gap-4 lg:flex-1 lg:min-h-0">
            
            <div className="flex items-center justify-between shrink-0">
              <div className="flex items-center gap-3">
                <Shield className="text-[rgb(var(--accent))]" size={22} />
                <div className="space-y-0.5">
                  <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">System Prompts</h2>
                  <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-wider font-bold opacity-80">Core AI Instructions</p>
                </div>
              </div>

              {/* Tab Switcher */}
              <div className="flex bg-[rgb(var(--foreground))]/[0.05] p-1 rounded-xl border border-[rgba(var(--border),0.05)]">
                <button
                  onClick={() => setPromptTab("en")}
                  className={cn(
                    "px-3.5 py-1.5 rounded-lg text-[13px] font-bold uppercase tracking-wider transition-all duration-300",
                    promptTab === "en" 
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  English
                </button>
                <button
                  onClick={() => setPromptTab("hi")}
                  className={cn(
                    "px-3.5 py-1.5 rounded-lg text-[13px] font-bold uppercase tracking-wider transition-all duration-300",
                    promptTab === "hi" 
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md" 
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  Hindi
                </button>
              </div>
            </div>

            <div className="flex-1 min-h-0 flex flex-col space-y-3">
              <div className="flex justify-between items-center shrink-0">
              </div>
              
              <div className="flex-1 min-h-[200px] lg:min-h-[160px] relative h-[250px] lg:h-auto">
                {promptTab === "en" ? (
                  <textarea 
                    value={draftSettings.assistant?.english_prompt || ""}
                    onChange={(e) => updateDraft("assistant", "english_prompt", e.target.value)}
                    className="absolute inset-0 w-full h-full p-4 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] text-[13px] text-[rgb(var(--foreground))] opacity-90 focus:opacity-100 focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all resize-none custom-scrollbar leading-relaxed"
                    placeholder="Provide guidelines for the AI assistant when speaking in English..."
                  />
                ) : (
                  <textarea 
                    value={draftSettings.assistant?.hindi_prompt || ""}
                    onChange={(e) => updateDraft("assistant", "hindi_prompt", e.target.value)}
                    className="absolute inset-0 w-full h-full p-4 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--border),0.05)] text-[13px] text-[rgb(var(--foreground))] opacity-90 focus:opacity-100 focus:outline-none focus:border-[rgb(var(--accent))]/50 transition-all resize-none custom-scrollbar leading-relaxed"
                    placeholder="Provide guidelines for the AI assistant when replying in Hindi..."
                  />
                )}
              </div>
            </div>

          </div>

        </div>

      </div>
    </div>
  );
};
