import { useState, useEffect, useCallback, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  setupRemoteServer,
  listVoices,
  fetchEdgeTtsVoices,
} from "@/services/pipelineService";
import {
  downloadOptionalModel,
  deleteModel,
  checkModelExists,
} from "@/services/modelService";
import {
  checkTtsProviderHealth,
  probeModelCapabilities,
  listLlmModels,
} from "@/services/settingsService";
import { listen } from "@tauri-apps/api/event";
import { Database } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";

import { ModelsTopologyMap } from "./ModelsTopologyMap";
import { VadWorkspace } from "./VadWorkspace";
import { AsrWorkspace } from "./AsrWorkspace";
import { AuxiliaryWorkspace } from "./AuxiliaryWorkspace";
import { RemoteServerSetup } from "./RemoteServerSetup";
import { TtsVoiceManager, type CustomVoice } from "./TtsVoiceManager";
import { TtsModelWorkspace } from "./TtsModelWorkspace";
import { LlmCatalogView } from "./LlmCatalogView";

interface ModelStatus {
  step: 'idle' | 'downloading' | 'extracting' | 'verifying' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  bytesDownloaded: number;
  totalBytes: number;
  error?: string;
}

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);

  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "llm" | "tts" | "auxiliary">("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const [customVoices, setCustomVoices] = useState<CustomVoice[]>([]);
  const [chatterboxIsAdding, setChatterboxIsAdding] = useState(false);
  const [isRemoteTtsHealthy, setIsRemoteTtsHealthy] = useState<boolean | null>(null);
  const [checkingTtsHealth, setCheckingTtsHealth] = useState(false);
  const [sshConnectionString, setSshConnectionString] = useState(() => localStorage.getItem("vox_ssh_conn") || "root@localhost");
  const [sshPort, setSshPort] = useState(() => localStorage.getItem("vox_ssh_port") || "22");
  const [sshIdentityKey, setSshIdentityKey] = useState(() => localStorage.getItem("vox_ssh_key") || "~/.ssh/id_rsa");
  const [setupStatus, setSetupStatus] = useState<any>(null);

  const [edgeTtsVoices, setEdgeTtsVoices] = useState<Array<{ name: string; short_name: string; gender: string; locale: string; friendly_name: string }>>([]);
  const [edgeTtsError, setEdgeTtsError] = useState<string | null>(null);
  const [loadingEdgeVoices, setLoadingEdgeVoices] = useState<boolean>(false);

  // LLM Remote state
  const [remoteModels, setRemoteModels] = useState<LlmModelInfo[]>([]);
  const [loadingRemoteModels, setLoadingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [probingMap, setProbingMap] = useState<Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>>({});
  const [customModelId, setCustomModelId] = useState("");
  const [customModelStatus, setCustomModelStatus] = useState<'idle' | 'checking' | 'valid' | 'invalid'>('idle');

  // Base layout decisions on committed and draft settings
  const savedProvider = settings?.llm?.provider;
  const isRemoteLlm = draftSettings?.llm?.provider?.kind === "open_ai_compat" || savedProvider?.kind === "open_ai_compat";
  const provider = draftSettings?.llm?.provider || savedProvider;

  // 1. Bidirectional Pipeline Tab Synchronization with InteractionCard
  useEffect(() => {
    const handleSync = (e: Event) => {
      const tab = (e as CustomEvent).detail;
      if (tab === "stt") {
        setActivePipelineTab("asr");
      } else if (tab === "llm" || tab === "tts") {
        setActivePipelineTab(tab);
      }
    };
    window.addEventListener("sync_pipeline_tab", handleSync);
    return () => window.removeEventListener("sync_pipeline_tab", handleSync);
  }, []);

  useEffect(() => {
    let cat = "";
    if (activePipelineTab === "asr") cat = "STT";
    else if (activePipelineTab === "llm") cat = "LLM";
    else if (activePipelineTab === "tts") cat = "TTS";
    if (cat) {
      const event = new CustomEvent("sync_interaction_category", { detail: cat });
      window.dispatchEvent(event);
    }
  }, [activePipelineTab]);

  // 2. Check model files presence
  const refreshPresence = useCallback(async () => {
    try {
      const presence: Record<string, boolean> = {};
      const allModels = [
        ...(modelCatalog?.asr || []),
        ...(modelCatalog?.llm || []),
        ...(modelCatalog?.tts || []),
      ];

      for (const model of allModels) {
        presence[model.id] = await checkModelExists(model.id);
      }
      setModelPresence(presence);
    } catch (e) {
      console.error("Failed to fetch models presence:", e);
    }
  }, [modelCatalog]);

  useEffect(() => {
    refreshPresence();
  }, [refreshPresence]);

  // 3. Custom Voices & Edge TTS
  const loadCustomVoices = useCallback(async () => {
    try {
      const list = await listVoices();
      setCustomVoices(list);
    } catch (e) {
      console.error("Failed to list voices", e);
    }
  }, []);

  const loadEdgeVoices = useCallback(async () => {
    setLoadingEdgeVoices(true);
    setEdgeTtsError(null);
    try {
      const list = await fetchEdgeTtsVoices();
      setEdgeTtsVoices(list);
    } catch (err: any) {
      console.error("Failed to fetch Edge TTS voices:", err);
      setEdgeTtsError(String(err));
    } finally {
      setLoadingEdgeVoices(false);
    }
  }, []);

  useEffect(() => {
    if (activePipelineTab === "tts") {
      loadCustomVoices();
    }
  }, [activePipelineTab, loadCustomVoices]);

  useEffect(() => {
    if (draftSettings?.tts?.provider?.kind === "edge_tts" && edgeTtsVoices.length === 0 && !loadingEdgeVoices) {
      loadEdgeVoices();
    }
  }, [draftSettings?.tts?.provider?.kind, edgeTtsVoices.length, loadingEdgeVoices, loadEdgeVoices]);

  // 4. Chatterbox Remote Health Polling
  useEffect(() => {
    if (draftSettings?.tts?.provider?.kind !== "chatterbox_remote") {
      setIsRemoteTtsHealthy(null);
      return;
    }

    const checkHealth = async () => {
      if (!draftSettings?.tts?.provider) return;
      setCheckingTtsHealth(true);
      try {
        const healthy = await checkTtsProviderHealth(draftSettings.tts.provider);
        setIsRemoteTtsHealthy(healthy);
      } catch (_) {
        setIsRemoteTtsHealthy(false);
      } finally {
        setCheckingTtsHealth(false);
      }
    };

    checkHealth();
    const interval = setInterval(checkHealth, 5000);
    return () => clearInterval(interval);
  }, [draftSettings?.tts?.provider]);

  useEffect(() => {
    const unlistenPromise = listen<any>("remote_setup_status", (event) => {
      setSetupStatus(event.payload);
      if (event.payload?.step === "complete" && draftSettings?.tts?.provider) {
        checkTtsProviderHealth(draftSettings.tts.provider).then((healthy) => setIsRemoteTtsHealthy(healthy));
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [draftSettings?.tts?.provider]);

  useEffect(() => {
    localStorage.setItem("vox_ssh_conn", sshConnectionString);
  }, [sshConnectionString]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_port", sshPort);
  }, [sshPort]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_key", sshIdentityKey);
  }, [sshIdentityKey]);

  // 5. Remote LLM Fetching & Probing
  const fetchRemoteModels = useCallback(async () => {
    if (!provider || provider.kind !== "open_ai_compat") return;
    setLoadingRemoteModels(true);
    setRemoteModelsError(null);
    try {
      const list = await listLlmModels(provider);
      setRemoteModels(list);
    } catch (err: any) {
      console.error("Failed to list remote models:", err);
      setRemoteModelsError(String(err));
    } finally {
      setLoadingRemoteModels(false);
    }
  }, [provider]);

  useEffect(() => {
    if (activePipelineTab === "llm" && isRemoteLlm) {
      fetchRemoteModels();
    }
  }, [activePipelineTab, isRemoteLlm, fetchRemoteModels]);

  const handleProbeCapabilities = useCallback(
    async (modelId?: string) => {
      if (!provider) return;
      const targetId = modelId || (provider.kind === "open_ai_compat" ? provider.model : "embedded");
      if (!targetId) return;

      setProbingMap((prev) => ({
        ...prev,
        [targetId]: { status: "testing" },
      }));

      try {
        const caps = await probeModelCapabilities(provider, targetId);
        setProbingMap((prev) => ({
          ...prev,
          [targetId]: { status: "success", capabilities: caps },
        }));
        setRemoteModels((prev) =>
          prev.map((m) => (m.id === targetId ? { ...m, capabilities: caps } : m))
        );
      } catch (err) {
        console.error("[CapabilityProbe] Failed to probe model:", err);
        setProbingMap((prev) => ({
          ...prev,
          [targetId]: { status: "error", error: String(err) },
        }));
      }
    },
    [provider]
  );

  const handleValidateCustomModel = async () => {
    if (!customModelId.trim() || !provider) return;
    setCustomModelStatus("checking");
    try {
      const caps = await probeModelCapabilities(provider, customModelId.trim());
      updateDraft("llm", "provider", {
        ...provider,
        model: customModelId.trim(),
      });
      setProbingMap((prev) => ({
        ...prev,
        [customModelId.trim()]: { status: "success", capabilities: caps },
      }));
      setCustomModelStatus("valid");
    } catch (_) {
      updateDraft("llm", "provider", {
        ...provider,
        model: customModelId.trim(),
      });
      setCustomModelStatus("invalid");
    }
  };

  const triggerRemoteSetup = async () => {
    if (!draftSettings?.tts?.provider) return;
    setSetupStatus({ progress: 10, step: "initiating", log_line: "Starting connection..." });
    try {
      const endpoint =
        draftSettings.tts.provider.kind === "chatterbox_remote"
          ? draftSettings.tts.provider.endpoint
          : "http://127.0.0.1:7860";
      const remotePath =
        draftSettings.tts.provider.kind === "chatterbox_remote"
          ? draftSettings.tts.provider.remote_path
          : "~/.vox";

      let srvPort = 7860;
      try {
        const urlObj = new URL(endpoint);
        srvPort = urlObj.port ? parseInt(urlObj.port) : 7860;
      } catch (_) {
        const parts = (endpoint || "").replace("http://", "").replace("https://", "").split(":");
        if (parts.length > 1) {
          srvPort = parseInt(parts[parts.length - 1]) || 7860;
        }
      }

      await setupRemoteServer({
        connectionString: sshConnectionString,
        sshPort: sshPort ? parseInt(sshPort) : null,
        identityKeyPath: sshIdentityKey || null,
        remotePath: remotePath || "~/.vox",
        serverPort: srvPort,
      });
    } catch (err) {
      setSetupStatus({ progress: 0, step: "failed", log_line: `Error: ${err}`, error: String(err) });
    }
  };

  const startDownload = async (modelId: string) => {
    try {
      setDownloadStatuses((prev) => ({
        ...prev,
        [modelId]: { step: "downloading", progress: 0, bytesDownloaded: 0, totalBytes: 100 },
      }));
      await downloadOptionalModel(modelId);
      refreshPresence();
    } catch (e) {
      console.error("Failed to start download:", e);
    }
  };

  const handleDeleteModelGroup = async (modelGroupId: string) => {
    try {
      await deleteModel(modelGroupId);
      setModelPresence((prev) => ({ ...prev, [modelGroupId]: false }));
      setConfirmDeleteId(null);
    } catch (e) {
      console.error("Failed to delete model group:", e);
    }
  };

  const isGroupRequired = (id: string) => {
    return id.includes("required") || id.includes("base");
  };

  if (!draftSettings) return null;

  // Topology verification flags
  const isVadVerified = !!(modelPresence["silero_vad_v5"] && modelPresence["speech_tokenizer"]);
  const isAsrVerified = !!(modelPresence["whisper_medium"] || modelPresence["whisper_small"] || modelPresence["whisper_tiny"]);
  const isLlmDownloaded = !!(modelPresence["qwen2_5_0_5b"] || modelPresence["qwen2_5_1_5b"] || isRemoteLlm);
  const isTtsVerified =
    draftSettings?.tts?.provider?.kind === "chatterbox_remote"
      ? isRemoteTtsHealthy === true
      : !!(modelPresence["chatterbox_turbo"] || modelPresence["supertonic"]);
  const isAuxiliaryVerified = !!(
    modelPresence["distilbert_query_classifier"] &&
    modelPresence["minilm_l12_v2"] &&
    modelPresence["deberta_v3_xsmall_nli"] &&
    modelPresence["vox_translit_rnn"]
  );

  return (
    <div
      className={cn(
        "w-full h-auto flex flex-col text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
        layoutMode === "small"
          ? "bg-transparent p-0"
          : cn(
              "glass-card p-5",
              layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
            )
      )}
    >
      <div className="flex flex-col gap-4">
        {/* Header with Model vs Settings Toggle */}
        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Database className="text-[rgb(var(--accent))]" size={18} />
            <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Model Hub
            </span>
          </div>

          {/* Small Category Tabs: Model vs Settings */}
          {(activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (() => {
            const isRemoteTtsSetupNotDone =
              activePipelineTab === "tts" &&
              draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
              isRemoteTtsHealthy !== true;

            if (isRemoteTtsSetupNotDone && activeCategoryTab === "settings") {
              setTimeout(() => setActiveCategoryTab("model"), 0);
            }

            return (
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
                {!isRemoteTtsSetupNotDone && (
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
                )}
              </div>
            );
          })()}
        </div>

        {/* Models Topology Interactive Bar */}
        <ModelsTopologyMap
          activeTab={activePipelineTab}
          onChangeTab={setActivePipelineTab}
          layoutMode={layoutMode}
          isVadVerified={isVadVerified}
          isAsrVerified={isAsrVerified}
          isLlmDownloaded={isLlmDownloaded}
          isTtsVerified={isTtsVerified}
          isAuxiliaryVerified={isAuxiliaryVerified}
        />

        {/* Workspaces by Pipeline Tab */}
        <div
          className={cn(
            "h-auto w-full flex flex-col glass rounded-xl p-3 relative bg-[rgba(var(--foreground),0.02)]",
            layoutMode === "small" ? "max-h-none overflow-y-visible" : "max-h-[220px] overflow-y-auto custom-scrollbar"
          )}
        >
          {activePipelineTab === "vad" && (
            <VadWorkspace
              activeCategoryTab={activeCategoryTab}
              layoutMode={layoutMode}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              modelPresence={modelPresence}
              downloadStatuses={downloadStatuses}
              startDownload={startDownload}
              deleteModel={handleDeleteModelGroup}
            />
          )}

          {activePipelineTab === "asr" && (
            <AsrWorkspace
              layoutMode={layoutMode}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              modelPresence={modelPresence}
              downloadStatuses={downloadStatuses}
              startDownload={startDownload}
              deleteModel={handleDeleteModelGroup}
              isGroupRequired={isGroupRequired}
            />
          )}

          {activePipelineTab === "llm" && (
            <LlmCatalogView
              layoutMode={layoutMode}
              selectedLlmId={draftSettings.llm.model}
              modelPresence={modelPresence}
              downloadStatuses={downloadStatuses}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              startDownload={startDownload}
              handleDeleteModelGroup={handleDeleteModelGroup}
              isGroupRequired={isGroupRequired}
              isRemoteLlm={isRemoteLlm}
              provider={provider}
              remoteModels={remoteModels}
              loadingRemoteModels={loadingRemoteModels}
              remoteModelsError={remoteModelsError}
              probingMap={probingMap}
              handleProbeCapabilities={handleProbeCapabilities}
              customModelId={customModelId}
              setCustomModelId={setCustomModelId}
              customModelStatus={customModelStatus}
              handleValidateCustomModel={handleValidateCustomModel}
              activeCategoryTab={activeCategoryTab}
            />
          )}

          {activePipelineTab === "tts" && (
            <>
              {activeCategoryTab === "model" ? (
                draftSettings.tts.provider?.kind === "chatterbox_remote" && isRemoteTtsHealthy !== true ? (
                  <RemoteServerSetup
                    sshConnectionString={sshConnectionString}
                    setSshConnectionString={setSshConnectionString}
                    sshPort={sshPort}
                    setSshPort={setSshPort}
                    sshIdentityKey={sshIdentityKey}
                    setSshIdentityKey={setSshIdentityKey}
                    setupStatus={setupStatus}
                    triggerRemoteSetup={triggerRemoteSetup}
                    isRemoteTtsHealthy={isRemoteTtsHealthy}
                  />
                ) : (
                  <TtsModelWorkspace
                    layoutMode={layoutMode}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    modelPresence={modelPresence}
                    downloadStatuses={downloadStatuses}
                    startDownload={startDownload}
                    deleteModel={handleDeleteModelGroup}
                    isRemoteTtsHealthy={isRemoteTtsHealthy}
                    checkingTtsHealth={checkingTtsHealth}
                  />
                )
              ) : (
                <TtsVoiceManager
                  layoutMode={layoutMode}
                  customVoices={customVoices}
                  loadCustomVoices={loadCustomVoices}
                  chatterboxIsAdding={chatterboxIsAdding}
                  setChatterboxIsAdding={setChatterboxIsAdding}
                  edgeTtsVoices={edgeTtsVoices}
                  edgeTtsError={edgeTtsError}
                  loadingEdgeVoices={loadingEdgeVoices}
                  loadEdgeVoices={loadEdgeVoices}
                  activeCategoryTab={activeCategoryTab}
                />
              )}
            </>
          )}

          {activePipelineTab === "auxiliary" && (
            <AuxiliaryWorkspace
              layoutMode={layoutMode}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              modelPresence={modelPresence}
              downloadStatuses={downloadStatuses}
              startDownload={startDownload}
              deleteModel={handleDeleteModelGroup}
            />
          )}
        </div>
      </div>
    </div>
  );
});

ModelsCard.displayName = "ModelsCard";
