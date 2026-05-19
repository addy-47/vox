import React, { useState, useEffect, useCallback } from "react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { 
  Brain, Volume2, Database, Trash2,
  Sliders, Languages, 
  Activity, Sparkles, Shield, Check, ArrowLeft
} from "lucide-react";

interface ModelStatus {
  step: 'idle' | 'downloading' | 'extracting' | 'verifying' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  bytesDownloaded: number;
  totalBytes: number;
  error?: string;
}

const mapModelId = (id: string): string => {
  switch (id) {
    case "gemma4":
      return "llm_gemma_4_q4_k_m";
    case "kokoro":
      return "tts_kokoro_onnx";
    case "qwen3-asr":
      return "stt_encoder";
    case "piper_hi":
      return "tts_hi_piper_onnx";
    case "translit":
      return "translit_encoder";
    default:
      return id;
  }
};

export const ModelSettings: React.FC = () => {
  const { draftSettings, updateDraft, modelCatalog } = useSettings();
  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [promptTab, setPromptTab] = useState<"en" | "hi">("en");
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "translit" | "llm" | "tts">("llm");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

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

  // Check local model existence dynamically
  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const presence: Record<string, boolean> = {};

    const checkIds = [
      "gemma4",
      "llm_llama_3_2_1b_instruct_q6_k",
      "llm_gemma_4_e2b_uncensored_aggressive_q2_k_p",
      "qwen3-asr",
      "translit",
      "kokoro",
      "piper_hi",
      "ten_vad"
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
  }, [modelCatalog, draftSettings]);

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
      const canonicalId = event.payload.model_id;
      setDownloadStatuses(prev => ({
        ...prev,
        [canonicalId]: {
          step: event.payload.step as any,
          progress: event.payload.progress,
          bytesDownloaded: event.payload.bytes_downloaded,
          totalBytes: event.payload.total_bytes,
          error: event.payload.error
        }
      }));
    });

    const unlistenComplete = listen<string>("optional_download_complete", (event) => {
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
  }, [checkPresence]);

  if (!draftSettings || !modelCatalog) return null;

  const startDownload = (modelId: string) => {
    setDownloadStatuses(prev => ({
      ...prev,
      [mapModelId(modelId)]: { step: 'idle', progress: 0, bytesDownloaded: 0, totalBytes: 0 }
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
  const isAsrVerified = modelPresence["qwen3-asr"];

  // Translit logic
  const isTranslitVerified = modelPresence["translit"];

  // LLM logic
  const selectedLlmId = draftSettings.llm.model;
  const isLlmDownloaded = modelPresence[selectedLlmId];

  // TTS logic
  const isTtsVerified = modelPresence["kokoro"] && modelPresence["piper_hi"];

  return (
    <div className="h-full overflow-y-auto lg:overflow-hidden custom-scrollbar pr-1 -mr-1 select-none pb-10">
      <div className="lg:h-full flex flex-col lg:grid lg:grid-cols-12 gap-8 items-stretch pb-10">
        
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
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]"
                )}
              >
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
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]"
                )}
              >
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
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]"
                )}
              >
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
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]"
                )}
              >
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
                    : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]"
                )}
              >
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
                  <div className="pb-2 border-b border-[rgba(var(--border),0.05)]">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Silence Filtering</span>
                  </div>

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
                          <span>TenVAD Engine</span>
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
                          {downloadStatuses[mapModelId("ten_vad")] ? (
                            <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses[mapModelId("ten_vad")].progress)}%</span>
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
                </div>
              )}

              {/* TAB 2: VOICE RECOGNITION (ASR) */}
              {activePipelineTab === "asr" && (
                <div className="space-y-4">
                  <div className="pb-2 border-b border-[rgba(var(--border),0.05)]">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Voice Recognition</span>
                  </div>

                  <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] space-y-4">
                    <div className="flex justify-between items-start">
                      <div>
                        <div className="text-[13px] font-bold text-[rgb(var(--foreground))]">Qwen3-ASR Decoder</div>
                        <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-1.5 leading-relaxed">
                          Multilingual Speech Recognition engine. Decodes live voice streams to text completely offline (~950MB).
                        </p>
                      </div>
                      <span className={cn(
                        "text-[13px] font-bold uppercase tracking-wider px-2.5 py-0.5 rounded border",
                        isAsrVerified ? "text-emerald-400 bg-emerald-500/5 border-emerald-500/10" : "text-red-400 bg-red-500/5 border-red-500/10"
                      )}>
                        {isAsrVerified ? "Active" : "Missing"}
                      </span>
                    </div>

                    <div className="flex justify-end gap-3 pt-3 border-t border-[rgba(var(--border),0.05)]">
                      {!isAsrVerified ? (
                        downloadStatuses[mapModelId("qwen3-asr")] ? (
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses[mapModelId("qwen3-asr")].progress)}%</span>
                        ) : (
                          <button 
                            onClick={() => startDownload("qwen3-asr")}
                            className="px-4 py-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow hover:scale-[1.02] transition-all"
                          >
                            Download Model
                          </button>
                        )
                      ) : (
                        renderDeleteControl("qwen3-asr")
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* TAB 3: ROMAN TRANSLITERATION */}
              {activePipelineTab === "translit" && (
                <div className="space-y-4">
                  <div className="pb-2 border-b border-[rgba(var(--border),0.05)]">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Roman Transliteration</span>
                  </div>

                  <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] space-y-4">
                    <div className="flex justify-between items-start">
                      <div>
                        <div className="text-[13px] font-bold text-[rgb(var(--foreground))]">Vox Hinglish RNN</div>
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
                        downloadStatuses[mapModelId("translit")] ? (
                          <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses[mapModelId("translit")].progress)}%</span>
                        ) : (
                          <button 
                            onClick={() => startDownload("translit")}
                            className="px-4 py-2 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow hover:scale-[1.02] transition-all"
                          >
                            Download Model
                          </button>
                        )
                      ) : (
                        renderDeleteControl("translit")
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* TAB 4: AI REASONING (LLM) - FULL CARD INVENTORY GRID SELECTOR */}
              {activePipelineTab === "llm" && (
                <div className="space-y-4">
                  <div className="pb-2 border-b border-[rgba(var(--border),0.05)]">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Select Active AI Reasoning Model</span>
                  </div>

                  <div className="grid grid-cols-1 gap-3.5">
                    {modelCatalog.llm.map((model) => {
                      const isSelected = selectedLlmId === model.id;
                      const isDownloaded = modelPresence[model.id];
                      const status = downloadStatuses[mapModelId(model.id)];

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
                            <div className="flex items-center gap-2">
                              <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">{model.name}</span>
                              <span className="text-[13px] font-mono text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded font-normal">{model.parameters}</span>
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
                                {model.id !== "llm_llama_3_2_1b_instruct_q6_k" && model.id !== "gemma4" && (
                                  renderDeleteControl(model.id, "icon-only")
                                )}
                              </div>
                            ) : (
                              status ? (
                                <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(status.progress)}%</span>
                              ) : (
                                <button 
                                  onClick={(e) => { e.stopPropagation(); startDownload(model.id); }}
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

              {/* TAB 5: VOICE SYNTHESIS (TTS) */}
              {activePipelineTab === "tts" && (
                <div className="space-y-4">
                  <div className="pb-2 border-b border-[rgba(var(--border),0.05)]">
                    <span className="text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">Voice Output</span>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    {/* English Voice synthesis */}
                    <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] flex flex-col justify-between h-36">
                      <div>
                        <div className="flex justify-between items-center">
                          <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">English Voice</span>
                          <span className={cn("w-2 h-2 rounded-full", modelPresence["kokoro"] ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500")} />
                        </div>
                        <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-2">
                          High quality English vocal syntheses package (~345MB).
                        </p>
                      </div>

                      <div className="flex justify-between items-center pt-2 border-t border-[rgba(var(--border),0.05)]">
                        <span className="text-[13px] text-[rgb(var(--foreground-muted))]">Deploy Weights</span>
                        {!modelPresence["kokoro"] ? (
                          downloadStatuses[mapModelId("kokoro")] ? (
                            <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses[mapModelId("kokoro")].progress)}%</span>
                          ) : (
                            <button onClick={() => startDownload("kokoro")} className="px-3 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow">Get</button>
                          )
                        ) : (
                          renderDeleteControl("kokoro", undefined, true)
                        )}
                      </div>
                    </div>

                    {/* Hindi Voice synthesis */}
                    <div className="p-4 rounded-xl border bg-[rgb(var(--foreground))]/[0.02] border-[rgba(var(--border),0.08)] flex flex-col justify-between h-36">
                      <div>
                        <div className="flex justify-between items-center">
                          <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">Hindi Voice</span>
                          <span className={cn("w-2 h-2 rounded-full", modelPresence["piper_hi"] ? "bg-emerald-500 shadow-[0_0_8px_#10B981]" : "bg-red-500")} />
                        </div>
                        <p className="text-[13px] text-[rgb(var(--foreground-muted))] opacity-85 mt-2">
                          Highly optimized Hindi synthesis speech weights (~63MB).
                        </p>
                      </div>

                      <div className="flex justify-between items-center pt-2 border-t border-[rgba(var(--border),0.05)]">
                        <span className="text-[13px] text-[rgb(var(--foreground-muted))]">Deploy Weights</span>
                        {!modelPresence["piper_hi"] ? (
                          downloadStatuses[mapModelId("piper_hi")] ? (
                            <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{Math.round(downloadStatuses[mapModelId("piper_hi")].progress)}%</span>
                          ) : (
                            <button onClick={() => startDownload("piper_hi")} className="px-3 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[13px] font-bold uppercase tracking-wider shadow">Get</button>
                          )
                        ) : (
                          renderDeleteControl("piper_hi", undefined, true)
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              )}

            </div>

          </div>
        </div>

        {/* Right Column: Parameters Tuning & Compact System Prompts Switcher */}
        <div className="lg:col-span-5 flex flex-col gap-6 lg:min-h-0">
          
          {/* Numerical Parameters Grid */}
          <div className="premium-card p-4 sm:p-6 lg:p-8 flex flex-col gap-5 shrink-0">
            <div className="flex items-center gap-3 shrink-0">
              <Sliders className="text-[rgb(var(--accent))]" size={22} />
              <div className="space-y-0.5">
                <h2 className="text-lg font-bold text-[rgb(var(--foreground))]">Tuning</h2>
                <p className="text-[13px] text-[rgb(var(--foreground-muted))] uppercase tracking-wider font-bold opacity-80">Precision and Audio Tuning</p>
              </div>
            </div>

            <div className="space-y-4">
              {/* VAD Activation Threshold */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Silence Threshold</span>
                  <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.vad.threshold.toFixed(2)}</span>
                </div>
                <input 
                  type="range" 
                  min="0.1" max="0.9" step="0.05"
                  value={draftSettings.vad.threshold}
                  onChange={(e) => updateDraft("vad", "threshold", Number(e.target.value))}
                  className="w-full"
                />
              </div>

              {/* LLM Context Limit */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Memory Context Tokens</span>
                  <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.ctx_size}</span>
                </div>
                <input 
                  type="range" 
                  min="512" max="8192" step="512"
                  value={draftSettings.llm.ctx_size}
                  onChange={(e) => updateDraft("llm", "ctx_size", Number(e.target.value))}
                  className="w-full"
                />
              </div>

              {/* Execution Threads */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Processor Threads</span>
                  <span className="text-[13px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.threads}</span>
                </div>
                <input 
                  type="range" 
                  min="1" max="16" step="1"
                  value={draftSettings.llm.threads}
                  onChange={(e) => updateDraft("llm", "threads", Number(e.target.value))}
                  className="w-full"
                />
              </div>

              <div className="h-px bg-[rgba(var(--border),0.05)] my-2" />

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
          <div className="premium-card p-4 sm:p-6 lg:p-8 flex flex-col gap-4 lg:flex-1 lg:min-h-0">
            
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
                <span className="text-[13px] font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-wider opacity-90">
                  {promptTab === "en" ? "English Instructions" : "Hindi Instructions"}
                </span>
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
