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
import * as eventsService from "@/services/eventsService";
import { Database } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl } from "@/shared/ui";
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

// Module-level persistent cache for model download statuses across modal open/close
const globalDownloadStatuses: Record<string, ModelStatus> = {};

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);

  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>(() => ({
    ...globalDownloadStatuses,
  }));
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "llm" | "tts" | "auxiliary">("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const updateDownloadStatus = useCallback((modelId: string, status: Partial<ModelStatus>) => {
    setDownloadStatuses((prev) => {
      const updated = {
        ...(prev[modelId] || { step: "idle", progress: 0, bytesDownloaded: 0, totalBytes: 100 }),
        ...status,
      };
      globalDownloadStatuses[modelId] = updated as ModelStatus;
      return { ...prev, [modelId]: updated as ModelStatus };
    });
  }, []);

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
      const allModelIds = modelCatalog?.model_groups?.map((g) => g.id) || [
        ...(modelCatalog?.vad?.map((m) => m.id) || []),
        ...(modelCatalog?.asr?.map((m) => m.id) || []),
        ...(modelCatalog?.llm?.map((m) => m.id) || []),
        ...(modelCatalog?.tts?.map((m) => m.id) || []),
        ...(modelCatalog?.auxiliary?.map((m) => m.id) || []),
      ];

      for (const id of allModelIds) {
        presence[id] = await checkModelExists(id);
      }
      setModelPresence(presence);
    } catch (e) {
      console.error("Failed to fetch models presence:", e);
    }
  }, [modelCatalog]);

  useEffect(() => {
    refreshPresence();
  }, [refreshPresence]);

  useEffect(() => {
    const unsub = eventsService.onOptionalModelComplete(() => {
      refreshPresence();
    });
    return () => {
      unsub();
    };
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
    return eventsService.onRemoteSetupStatus((payload) => {
      setSetupStatus(payload);
      if (payload?.step === "complete" && draftSettings?.tts?.provider) {
        checkTtsProviderHealth(draftSettings.tts.provider).then((healthy) => setIsRemoteTtsHealthy(healthy));
      }
    });
  }, [draftSettings?.tts?.provider]);

  // Model download events listener (model_setup_status, optional_model_complete, optional_model_failed)
  useEffect(() => {
    const unlistenStatus = eventsService.onModelSetupStatus((payload) => {
      const { model_id, step, progress, bytes_downloaded, total_bytes, error } = payload || {};
      if (!model_id) return;

      const stepLower = String(step || "downloading").toLowerCase() as ModelStatus["step"];

      updateDownloadStatus(model_id, {
        step: stepLower,
        progress: typeof progress === "number" ? progress : 0,
        bytesDownloaded: bytes_downloaded || 0,
        totalBytes: total_bytes || 100,
        error: error || undefined,
      });
    });

    const unlistenComplete = eventsService.onOptionalModelComplete((modelGroupId) => {
      if (!modelGroupId) return;

      updateDownloadStatus(modelGroupId, {
        step: "completed",
        progress: 100,
      });
      refreshPresence();
    });

    const unlistenFailed = eventsService.on<any>("optional_model_failed", (payload) => {
      const modelGroupId = Array.isArray(payload) ? payload[0] : typeof payload === "string" ? payload : payload?.model_id;
      const errStr = Array.isArray(payload) ? payload[1] : payload?.error || "Download failed";
      if (!modelGroupId) return;

      updateDownloadStatus(modelGroupId, {
        step: "failed",
        error: String(errStr),
      });
    });

    return () => {
      unlistenStatus();
      unlistenComplete();
      unlistenFailed();
    };
  }, [updateDownloadStatus, refreshPresence]);

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
      updateDownloadStatus(modelId, {
        step: "downloading",
        progress: 1,
        bytesDownloaded: 0,
        totalBytes: 100,
        error: undefined,
      });
      await downloadOptionalModel(modelId);
    } catch (e: any) {
      console.error("Failed to start download:", e);
      updateDownloadStatus(modelId, {
        step: "failed",
        error: String(e),
      });
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

  const MANDATORY_CORE_MODEL_IDS = new Set([
    "ten_vad",
    "silero_vad_v5",
    "whisper_medium",
    "qwen2_5_0_5b",
    "chatterbox_turbo",
  ]);

  const isGroupRequired = useCallback((id: string) => {
    return MANDATORY_CORE_MODEL_IDS.has(id);
  }, []);

  useEffect(() => {
    const isRemoteTtsSetupNotDone =
      activePipelineTab === "tts" &&
      draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
      isRemoteTtsHealthy !== true;

    if (isRemoteTtsSetupNotDone && activeCategoryTab === "settings") {
      setActiveCategoryTab("model");
    }
  }, [activePipelineTab, draftSettings?.tts?.provider?.kind, isRemoteTtsHealthy, activeCategoryTab]);



  if (!draftSettings) return null;

  // Topology verification flags
  const isVadVerified = draftSettings?.vad?.vad_backend === "earshot" || !!modelPresence["ten_vad"];
  const isAsrVerified = !!(modelPresence[draftSettings?.asr?.model || "nvidia_nemotron"] || modelPresence["nvidia_nemotron"] || modelPresence["qwen3_asr"]);
  const isLlmDownloaded = isRemoteLlm || !!(modelPresence[draftSettings?.llm?.model || "llama_3_2_reasoning_q4"] || modelPresence["gemma_4_reasoning"]);
  const isTtsVerified =
    draftSettings?.tts?.provider?.kind === "chatterbox_remote"
      ? isRemoteTtsHealthy === true
      : draftSettings?.tts?.provider?.kind === "edge_tts"
        ? true
        : !!(modelPresence["supertonic_tts"] || modelPresence["chatterbox_tts"]);
  const isAuxiliaryVerified =
    (modelCatalog?.auxiliary?.length || 0) > 0
      ? modelCatalog?.auxiliary?.every((m) => !!modelPresence[m.id])
      : true;

  return (
    <div
      className={cn(
        "w-full h-auto flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
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
            <span className="font-display text-[13px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
              Model Hub
            </span>
          </div>

          {/* Small Category Tabs: Model vs Settings */}
          {(activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (
            <SegmentedControl
              options={[
                { id: "model", label: "Model" },
                { id: "settings", label: "Settings" },
              ]}
              value={activeCategoryTab}
              onChange={(val) => setActiveCategoryTab(val as "model" | "settings")}
              size="sm"
            />
          )}
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
