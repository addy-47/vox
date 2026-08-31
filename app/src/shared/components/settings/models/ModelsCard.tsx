import { useState, useEffect, useCallback, useMemo, useRef, memo } from "react";
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
import { Database, Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl } from "@/shared/ui";
import { LlmModelInfo, ModelCapabilities, LlmProviderConfig } from "@/store/settingsStore";

import { ModelsTopologyMap } from "./ModelsTopologyMap";
import { VadWorkspace } from "./VadWorkspace";
import { AsrWorkspace } from "./AsrWorkspace";
import { AuxiliaryWorkspace } from "./AuxiliaryWorkspace";
import { RemoteServerSetup } from "./RemoteServerSetup";
import { TtsVoiceManager, type CustomVoice } from "./TtsVoiceManager";
import { TtsModelWorkspace } from "./TtsModelWorkspace";
import { LlmCatalogView } from "./LlmCatalogView";
import { LlmSettingsView } from "./LlmSettingsView";

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

  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);

  // Load disk capabilities cache and model catalog on initial mount
  useEffect(() => {
    useSettingsStore.getState().loadCapabilitiesCache();
    if (!useSettingsStore.getState().modelCatalog) {
      useSettingsStore.getState().loadModelCatalog();
    }
  }, []);

  // Sync disk capabilities cache into component state on mount / update
  useEffect(() => {
    if (capabilitiesCache && Object.keys(capabilitiesCache).length > 0) {
      setProbingMap((prev) => {
        const next = { ...prev };
        for (const [_, caps] of Object.entries(capabilitiesCache)) {
          const mId = caps.model_id;
          if (!next[mId] || next[mId].status === "idle") {
            next[mId] = { status: "success", capabilities: caps };
          }
        }
        return next;
      });
    }
  }, [capabilitiesCache]);

  // Base layout decisions on committed and draft settings
  const activeProviderKind = draftSettings?.llm?.active || "embedded";
  const isRemoteLlm = activeProviderKind === "server" || activeProviderKind === "cloud";

  const provider: LlmProviderConfig = useMemo(() => {
    if (activeProviderKind === "embedded") {
      return { kind: "embedded" };
    }
    if (activeProviderKind === "server") {
      return {
        kind: "open_ai_compat",
        base_url: draftSettings?.llm?.server?.base_url || "",
        model: draftSettings?.llm?.server?.model || "",
        api_key: draftSettings?.llm?.server?.api_key || undefined,
        provider_name: draftSettings?.llm?.server?.provider_name || undefined,
      };
    }
    return {
      kind: "open_ai_compat",
      base_url: draftSettings?.llm?.cloud?.base_url || "",
      model: draftSettings?.llm?.cloud?.model || "",
      api_key: draftSettings?.llm?.cloud?.api_key || undefined,
      provider_name: draftSettings?.llm?.cloud?.provider_name || undefined,
    };
  }, [
    activeProviderKind,
    draftSettings?.llm?.server?.base_url,
    draftSettings?.llm?.server?.model,
    draftSettings?.llm?.server?.api_key,
    draftSettings?.llm?.server?.provider_name,
    draftSettings?.llm?.cloud?.base_url,
    draftSettings?.llm?.cloud?.model,
    draftSettings?.llm?.cloud?.api_key,
    draftSettings?.llm?.cloud?.provider_name,
  ]);

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

  // 2. Check model files presence in parallel
  const refreshPresence = useCallback(async () => {
    try {
      const allModelIds = modelCatalog?.model_groups?.map((g) => g.id) || [
        ...(modelCatalog?.vad?.map((m) => m.id) || []),
        ...(modelCatalog?.asr?.map((m) => m.id) || []),
        ...(modelCatalog?.llm?.map((m) => m.id) || []),
        ...(modelCatalog?.tts?.map((m) => m.id) || []),
        ...(modelCatalog?.auxiliary?.map((m) => m.id) || []),
      ];

      const entries = await Promise.all(
        allModelIds.map(async (id) => [id, await checkModelExists(id)] as const)
      );
      setModelPresence(Object.fromEntries(entries));
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

  // 4. Chatterbox Remote Health Polling (gated by visibility & active tab)
  useEffect(() => {
    if (draftSettings?.tts?.provider?.kind !== "chatterbox_remote") {
      setIsRemoteTtsHealthy(null);
      return;
    }

    const checkHealth = async () => {
      if (document.hidden || !draftSettings?.tts?.provider || activePipelineTab !== "tts") return;
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
  }, [draftSettings?.tts?.provider, activePipelineTab]);

  useEffect(() => {
    return eventsService.onModelProgress((payload) => {
      if (payload?.model_id === "chatterbox_remote_server") {
        setSetupStatus({
          step: payload.step,
          progress: payload.progress,
          log_line: payload.error ? `Error: ${payload.error}` : `Step: ${payload.step}`,
          error: payload.error || undefined,
        });
        if ((payload.step === "completed" || payload.step === "Completed") && draftSettings?.tts?.provider) {
          checkTtsProviderHealth(draftSettings.tts.provider).then((healthy) => setIsRemoteTtsHealthy(healthy));
        }
      }
    });
  }, [draftSettings?.tts?.provider]);

  // Model download events listener (unified model_progress)
  useEffect(() => {
    const unlistenProgress = eventsService.onModelProgress((payload) => {
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

      if (stepLower === "completed" || (stepLower as string) === "complete") {
        refreshPresence();
      }
    });

    return () => {
      unlistenProgress();
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
  const lastFetchedKeyRef = useRef<string>("");

  const fetchRemoteModels = useCallback(async (force = false) => {
    if (!provider || provider.kind !== "open_ai_compat" || !provider.base_url) return;
    const fetchKey = `${provider.base_url}:${provider.api_key || ""}`;
    if (!force && lastFetchedKeyRef.current === fetchKey && remoteModels.length > 0) {
      return;
    }
    lastFetchedKeyRef.current = fetchKey;
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
  }, [provider, remoteModels.length]);

  useEffect(() => {
    if (activePipelineTab === "llm" && isRemoteLlm && provider?.kind === "open_ai_compat" && provider.base_url) {
      fetchRemoteModels();
    }
  }, [activePipelineTab, isRemoteLlm, provider, fetchRemoteModels]);

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
        useSettingsStore.setState((state) => ({
          capabilitiesCache: {
            ...state.capabilitiesCache,
            [`${caps.provider_kind}:${caps.model_id}`]: caps,
          },
        }));
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
    const mId = customModelId.trim();
    const draft = useSettingsStore.getState().draftSettings;
    const activeLlm = draft?.llm?.active || (provider && "base_url" in provider ? "server" : "embedded");

    try {
      const caps = await probeModelCapabilities(provider, mId);
      if (activeLlm === "server" && draft?.llm?.server) {
        updateDraft("llm", "server", { ...draft.llm.server, model: mId });
      } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
        updateDraft("llm", "cloud", { ...draft.llm.cloud, model: mId });
      }
      if (provider && "base_url" in provider) {
        updateDraft("llm", "provider", { ...provider, model: mId });
      }
      updateDraft("llm", "model", mId);
      setProbingMap((prev) => ({
        ...prev,
        [mId]: { status: "success", capabilities: caps },
      }));
      setCustomModelStatus("valid");
    } catch (_) {
      if (activeLlm === "server" && draft?.llm?.server) {
        updateDraft("llm", "server", { ...draft.llm.server, model: mId });
      } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
        updateDraft("llm", "cloud", { ...draft.llm.cloud, model: mId });
      }
      if (provider && "base_url" in provider) {
        updateDraft("llm", "provider", { ...provider, model: mId });
      }
      updateDraft("llm", "model", mId);
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
  }, [activePipelineTab, draftSettings?.tts?.active, isRemoteTtsHealthy, activeCategoryTab]);

  if (!draftSettings) return null;

  // Topology verification flags (dynamically checking configured active models and providers)
  const isVadVerified =
    draftSettings?.vad?.vad_backend === "earshot" ||
    !!modelPresence[modelCatalog?.vad?.[0]?.id || "ten_vad"];

  const isAsrVerified =
    draftSettings?.stt?.active === "cloud"
      ? true
      : !!(draftSettings?.stt?.embedded?.model && modelPresence[draftSettings.stt.embedded.model]);

  const isLlmDownloaded =
    isRemoteLlm
      ? true
      : !!(draftSettings?.llm?.embedded?.model && modelPresence[draftSettings.llm.embedded.model]);

  const isTtsVerified =
    draftSettings?.tts?.active === "edge_tts"
      ? true
      : draftSettings?.tts?.active === "chatterbox_remote"
      ? isRemoteTtsHealthy === true
      : draftSettings?.tts?.active === "supertonic"
      ? !!modelPresence[modelCatalog?.tts?.find((m) => m.id.includes("supertonic"))?.id || "supertonic_tts"]
      : draftSettings?.tts?.active === "chatterbox"
      ? !!modelPresence[modelCatalog?.tts?.find((m) => m.id.includes("chatterbox"))?.id || "chatterbox_tts"]
      : false;

  const isAuxiliaryVerified =
    (modelCatalog?.auxiliary?.length || 0) > 0
      ? (modelCatalog?.auxiliary ?? []).filter((m) => (m as { required?: boolean }).required !== false).every((m) => !!modelPresence[m.id])
      : true;

  return (
    <div
      className={cn(
        "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
        layoutMode === "small"
          ? "bg-transparent p-0 h-auto"
          : cn(
              "glass-card p-5 lg:h-[340px]",
              layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
            )
      )}
    >
      <div className="flex flex-col gap-2.5 flex-1 min-h-0">
        {/* Header with Model vs Settings Toggle */}
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Database className="text-[rgb(var(--accent))]" size={17} />
            <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              Model Hub
            </span>
          </div>

          {/* Small Category Tabs: Model vs Settings */}
          {(activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (
            <SegmentedControl
              options={[
                { id: "model", label: "Model" },
                {
                  id: "settings",
                  label: "Settings",
                  disabled:
                    activePipelineTab === "tts" &&
                    draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
                    isRemoteTtsHealthy !== true,
                  title:
                    activePipelineTab === "tts" &&
                    draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
                    isRemoteTtsHealthy !== true
                      ? "Complete server setup first to configure voices"
                      : undefined,
                },
              ]}
              value={activeCategoryTab}
              onChange={(val) => setActiveCategoryTab(val as "model" | "settings")}
              size="sm"
            />
          )}
        </div>

        {/* Models Topology Interactive Bar */}
        <div className="shrink-0">
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
        </div>

        {/* Workspaces by Pipeline Tab: Unified Glass Container */}
        <div
          className={cn(
            "flex-1 w-full flex flex-col min-h-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]",
            layoutMode === "small" ? "h-auto max-h-[235px]" : "h-full"
          )}
        >
          {!modelCatalog ? (
            <div className="flex-1 flex items-center justify-center py-12">
              <Loader2 className="animate-spin text-[rgb(var(--accent))]" size={24} />
            </div>
          ) : (
            <>
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
                activeCategoryTab === "settings" ? (
                  <LlmSettingsView
                    layoutMode={layoutMode}
                    isRemoteLlm={isRemoteLlm}
                    provider={provider}
                  />
                ) : (
                  <LlmCatalogView
                    layoutMode={layoutMode}
                    selectedLlmId={
                      (draftSettings?.llm?.active === "embedded"
                        ? draftSettings.llm.embedded?.model
                        : draftSettings?.llm?.active === "server"
                        ? draftSettings.llm.server?.model
                        : draftSettings?.llm?.cloud?.model) || ""
                    }
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
                  />
                )
              )}

              {activePipelineTab === "tts" && (
                <>
                  {activeCategoryTab === "model" ? (
                    draftSettings?.tts?.provider?.kind === "chatterbox_remote" && isRemoteTtsHealthy !== true ? (
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
            </>
          )}
        </div>
      </div>
    </div>
  );
});

ModelsCard.displayName = "ModelsCard";
