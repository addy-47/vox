import { useState, useEffect, useCallback, useMemo, useRef, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  setupRemoteServer,
  listVoices,
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
  getProviderCaps,
} from "@/services/settingsService";
import * as eventsService from "@/services/eventsService";
import {
  MODEL_HUB_COPY,
  VAD_SETTINGS_COPY,
  STT_SETTINGS_COPY,
  LLM_SETTINGS_COPY,
} from "@/data/settingsCopy";
import { Orbit, Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl } from "@/shared/ui";
import { LlmModelInfo, ModelCapabilities, LlmProviderConfig, ProviderCaps } from "@/store/settingsStore";

import { ModelsTopologyMap, type PipelineTab } from "./ModelsTopologyMap";
import { SettingsTopologyMap } from "./SettingsTopologyMap";
import { VadWorkspace } from "./VadWorkspace";
import { AsrWorkspace } from "./AsrWorkspace";
import { AuxiliaryWorkspace } from "./AuxiliaryWorkspace";
import { RemoteServerSetup, type RemoteSetupStatus } from "./RemoteServerSetup";
import { TtsVoiceManager, type CustomVoice, type TtsSubTab } from "./TtsVoiceManager";
import { TtsModelWorkspace } from "./TtsModelWorkspace";
import { LlmCatalogView } from "./LlmCatalogView";
import { LlmSettingsView, type SettingsSubTab as LlmSubTab } from "./LlmSettingsView";
import {
  AudioWaveform,
  AudioLines,
  Microchip,
  TextCursorInput,
  Layers2,
  WandSparkles,
  Metronome,
  Hourglass,
  SlidersHorizontal,
  Zap,
  Languages,
} from "lucide-react";

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
  const [activePipelineTab, setActivePipelineTab] = useState<PipelineTab>("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  // Settings Topology subtab tracking
  const [activeVadSubTab, setActiveVadSubTab] = useState<string>("sensitivity");
  const [activeSttSubTab, setActiveSttSubTab] = useState<string>("streamingRate");
  const [activeLlmSubTab, setActiveLlmSubTab] = useState<LlmSubTab>("compute");
  const [activeTtsSubTab, setActiveTtsSubTab] = useState<TtsSubTab>("voice");

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
  const [setupStatus, setSetupStatus] = useState<RemoteSetupStatus | null>(null);

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
  const isCloudLlm = activeProviderKind === "cloud";

  // Preview selection: the clicked card's group id IS the settings active key.
  // Tier and health gating derive from manifest flags — never from id literals
  // and never from the stale derived `draftSettings.tts.provider.kind`.
  const previewTtsGroup = modelCatalog?.tts?.find((g) => g.id === draftSettings?.tts?.active);
  const isCloudTts = !!previewTtsGroup?.is_cloud;
  const isRemoteTts = !!previewTtsGroup?.is_remote;

  // Settings capabilities for the preview TTS provider (caps-driven panes).
  const [ttsCaps, setTtsCaps] = useState<ProviderCaps | null>(null);

  useEffect(() => {
    let isMounted = true;
    const previewId = draftSettings?.tts?.active;
    if (!previewId) {
      setTtsCaps(null);
      return;
    }
    getProviderCaps(previewId).then((caps) => {
      if (isMounted) setTtsCaps(caps);
    }).catch(() => {
      if (isMounted) setTtsCaps(null);
    });
    return () => {
      isMounted = false;
    };
  }, [draftSettings?.tts?.active]);

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
        setActivePipelineTab("stt");
      } else if (tab === "llm" || tab === "tts") {
        setActivePipelineTab(tab);
      }
    };
    window.addEventListener("sync_pipeline_tab", handleSync);
    return () => window.removeEventListener("sync_pipeline_tab", handleSync);
  }, []);

  useEffect(() => {
    let cat = "";
    if (activePipelineTab === "stt") cat = "STT";
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
        ...(modelCatalog?.stt?.map((m) => m.id) || []),
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
      const list = await listVoices("edge");
      const mapped = list.map((v) => ({
        name: v.id,
        short_name: v.id,
        gender: "Unknown",
        locale: "en-US",
        friendly_name: v.name,
      }));
      setEdgeTtsVoices(mapped);
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
    if (isCloudTts && edgeTtsVoices.length === 0 && !loadingEdgeVoices) {
      loadEdgeVoices();
    }
  }, [isCloudTts, edgeTtsVoices.length, loadingEdgeVoices, loadEdgeVoices]);

  // 4. Remote TTS Health Polling (gated by preview tier, visibility & active tab)
  useEffect(() => {
    if (!isRemoteTts) {
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

  // Per-file progress accumulator: backend emits model_progress with file-entry
  // ids (e.g. tts_kokoro_model) while cards are keyed by group id
  // (e.g. kokoro). Aggregate here so multi-file groups
  // show real weighted progress instead of sticking at the optimistic 1%.
  const fileProgressRef = useRef<Record<string, { progress: number; bytesDownloaded: number; totalBytes: number; done: boolean }>>({});

  // Model download events listener (unified model_progress)
  useEffect(() => {
    const unlistenProgress = eventsService.onModelProgress((payload) => {
      const { model_id, step, progress, bytes_downloaded, total_bytes, error } = payload || {};
      if (!model_id) return;

      const stepLower = String(step || "downloading").toLowerCase() as ModelStatus["step"];
      const fileProgress = typeof progress === "number" ? progress : 0;
      const fileBytes = bytes_downloaded || 0;
      const fileTotal = total_bytes || 100;

      fileProgressRef.current[model_id] = {
        progress: fileProgress,
        bytesDownloaded: fileBytes,
        totalBytes: fileTotal,
        done: stepLower === "completed" || (stepLower as string) === "complete",
      };

      const groups = modelCatalog?.model_groups || [];
      const parent = groups.find(
        (g) => g.id === model_id || (g.files || []).some((f) => f.id === model_id)
      );
      const targetId = parent ? parent.id : model_id;

      if (parent && parent.id !== model_id) {
        const files = parent.files || [];
        let totalBytes = 0;
        let doneBytes = 0;
        let allDone = files.length > 0;
        for (const f of files) {
          const fp = fileProgressRef.current[f.id];
          const tb = fp?.totalBytes || f.size || 0;
          totalBytes += tb;
          doneBytes += fp ? (fp.bytesDownloaded || (fp.progress / 100) * tb) : 0;
          if (!fp?.done) allDone = false;
        }
        if (stepLower === "failed" || stepLower === "cancelled") {
          updateDownloadStatus(targetId, {
            step: stepLower,
            progress: totalBytes > 0 ? (doneBytes / totalBytes) * 100 : 0,
            bytesDownloaded: Math.round(doneBytes),
            totalBytes: totalBytes || 100,
            error: error || undefined,
          });
        } else if (allDone) {
          updateDownloadStatus(targetId, {
            step: "completed",
            progress: 100,
            bytesDownloaded: totalBytes,
            totalBytes: totalBytes || 100,
            error: undefined,
          });
          refreshPresence();
        } else {
          updateDownloadStatus(targetId, {
            step: "downloading",
            progress: totalBytes > 0 ? (doneBytes / totalBytes) * 100 : 0,
            bytesDownloaded: Math.round(doneBytes),
            totalBytes: totalBytes || 100,
            error: undefined,
          });
        }
        return;
      }

      updateDownloadStatus(model_id, {
        step: stepLower,
        progress: fileProgress,
        bytesDownloaded: fileBytes,
        totalBytes: fileTotal,
        error: error || undefined,
      });

      if (stepLower === "completed" || (stepLower as string) === "complete") {
        refreshPresence();
      }
    });

    return () => {
      unlistenProgress();
    };
  }, [updateDownloadStatus, refreshPresence, modelCatalog]);

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
    if (!isRemoteTts) return;
    setSetupStatus({ progress: 10, step: "initiating", log_line: "Starting connection..." });
    try {
      // Endpoint values come from the derived provider config; the gate above
      // is the preview tier flag, not the provider kind.
      const ttsProvider = draftSettings?.tts?.provider;
      const endpoint =
        ttsProvider && ttsProvider.kind === "chatterbox_remote"
          ? ttsProvider.endpoint
          : "http://127.0.0.1:7860";
      const remotePath =
        ttsProvider && ttsProvider.kind === "chatterbox_remote"
          ? ttsProvider.remote_path
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

  // Required = manifest SSOT (`required` flag or built-in). No id literals.
  const isGroupRequired = useCallback((id: string) => {
    const g = modelCatalog?.model_groups?.find((x) => x.id === id);
    return !!g && (!!g.required || !!g.is_built_in);
  }, [modelCatalog]);

  useEffect(() => {
    const isRemoteTtsSetupNotDone =
      activePipelineTab === "tts" && isRemoteTts && isRemoteTtsHealthy !== true;

    if (isRemoteTtsSetupNotDone && activeCategoryTab === "settings") {
      setActiveCategoryTab("model");
    }
  }, [activePipelineTab, isRemoteTts, isRemoteTtsHealthy, activeCategoryTab]);

  if (!draftSettings) return null;


  // Topology verification flags: the preview active id IS the manifest group id.
  const isVadVerified =
    !!modelPresence[draftSettings?.vad?.vad_backend || ""] ||
    !!modelCatalog?.vad?.find((m) => m.is_built_in && m.id === draftSettings?.vad?.vad_backend);

  const isAsrVerified =
    draftSettings?.stt?.active === "cloud"
      ? true
      : !!(draftSettings?.stt?.embedded?.model && modelPresence[draftSettings.stt.embedded.model]);

  const isLlmDownloaded =
    isRemoteLlm
      ? true
      : !!(draftSettings?.llm?.embedded?.model && modelPresence[draftSettings.llm.embedded.model]);

  // The preview active id IS the manifest group id — direct presence lookup.
  // Cloud needs nothing on disk; remote needs a healthy server.
  const isTtsVerified = isCloudTts
    ? true
    : isRemoteTts
    ? isRemoteTtsHealthy === true
    : !!modelPresence[draftSettings?.tts?.active || ""];

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
        {/* Header Row: Title on Left, Model | Settings Toggle on Right */}
        <div className="flex items-center justify-between gap-2 mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2 min-w-0">
            <Orbit className="text-[rgb(var(--accent))] shrink-0" size={17} />
            <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {MODEL_HUB_COPY.title}
            </span>
          </div>

          {/* Small Category Tabs: Model vs Settings */}
          {(activePipelineTab === "vad" || activePipelineTab === "stt" || activePipelineTab === "llm" || activePipelineTab === "tts") && (
            <SegmentedControl
              options={[
                { id: "model", label: "Model" },
                {
                  id: "settings",
                  label: "Settings",
                  disabled:
                    activePipelineTab === "tts" &&
                    isRemoteTts &&
                    isRemoteTtsHealthy !== true,
                  title:
                    activePipelineTab === "tts" &&
                    isRemoteTts &&
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

        {/* Topology Bar: Swapped between Models Topology and Settings Topology */}
        <div className="shrink-0">
          {activeCategoryTab === "model" ? (
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
          ) : (
            <SettingsTopologyMap
              layoutMode={layoutMode}
              nodes={
                activePipelineTab === "vad"
                  ? [
                      { id: "sensitivity", label: VAD_SETTINGS_COPY.tabs.sensitivity, Icon: AudioWaveform },
                      { id: "silence", label: VAD_SETTINGS_COPY.tabs.silence, Icon: Hourglass },
                      { id: "noiseGate", label: VAD_SETTINGS_COPY.tabs.noiseGate, Icon: SlidersHorizontal },
                    ]
                  : activePipelineTab === "stt"
                  ? [
                      { id: "streamingRate", label: STT_SETTINGS_COPY.tabs.streamingRate, Icon: Zap },
                      { id: "transliteration", label: STT_SETTINGS_COPY.tabs.transliteration, Icon: Languages },
                      { id: "compute", label: "Compute", Icon: Microchip },
                    ]
                  : activePipelineTab === "llm"
                  ? [
                      { id: "compute", label: LLM_SETTINGS_COPY.tabs.compute, Icon: Microchip },
                      { id: "tokens", label: LLM_SETTINGS_COPY.tabs.tokens, Icon: TextCursorInput },
                      { id: "context", label: LLM_SETTINGS_COPY.tabs.context, Icon: Layers2 },
                      { id: "creativity", label: LLM_SETTINGS_COPY.tabs.creativity, Icon: WandSparkles },
                    ]
                  : [
                      { id: "voice", label: "Voice", Icon: AudioLines },
                      { id: "speed", label: "Speech Rate", Icon: Metronome },
                      { id: "compute", label: "Compute", Icon: Microchip },
                    ]
              }
              activeSubTab={
                activePipelineTab === "vad"
                  ? activeVadSubTab
                  : activePipelineTab === "stt"
                  ? activeSttSubTab
                  : activePipelineTab === "llm"
                  ? activeLlmSubTab
                  : activeTtsSubTab
              }
              onChangeSubTab={(id) => {
                if (activePipelineTab === "vad") setActiveVadSubTab(id);
                else if (activePipelineTab === "stt") setActiveSttSubTab(id);
                else if (activePipelineTab === "llm") setActiveLlmSubTab(id as LlmSubTab);
                else if (activePipelineTab === "tts") setActiveTtsSubTab(id as TtsSubTab);
              }}
            />
          )}
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
                  activeSubTab={activeVadSubTab}
                  layoutMode={layoutMode}
                  confirmDeleteId={confirmDeleteId}
                  setConfirmDeleteId={setConfirmDeleteId}
                  modelPresence={modelPresence}
                  downloadStatuses={downloadStatuses}
                  startDownload={startDownload}
                  deleteModel={handleDeleteModelGroup}
                />
              )}

              {activePipelineTab === "stt" && (
                <AsrWorkspace
                  activeCategoryTab={activeCategoryTab}
                  activeSubTab={activeSttSubTab}
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
                    activeSubTab={activeLlmSubTab}
                    layoutMode={layoutMode}
                    isRemoteLlm={isRemoteLlm}
                    isCloud={isCloudLlm}
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
                    isRemoteTts && isRemoteTtsHealthy !== true ? (
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
                      providerId={draftSettings?.tts?.active || ""}
                      caps={ttsCaps}
                      customVoices={customVoices}
                      loadCustomVoices={loadCustomVoices}
                      chatterboxIsAdding={chatterboxIsAdding}
                      setChatterboxIsAdding={setChatterboxIsAdding}
                      edgeTtsVoices={edgeTtsVoices}
                      edgeTtsError={edgeTtsError}
                      loadingEdgeVoices={loadingEdgeVoices}
                      loadEdgeVoices={loadEdgeVoices}
                      activeCategoryTab={activeCategoryTab}
                      activeSubTab={activeTtsSubTab}
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
