import { useState, useEffect, useCallback, useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  setupRemoteServer,
  listVoices,
} from "@/services/pipelineService";
import {
  checkTtsProviderHealth,
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
import { LlmProviderConfig, ProviderCaps } from "@/store/settingsStore";
import { useModelDownloads } from "@/shared/hooks/useModelDownloads";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";

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

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);

  const {
    downloadStatuses,
    modelPresence,
    confirmDeleteId,
    setConfirmDeleteId,
    startDownload,
    handleDeleteModelGroup,
    isGroupRequired,
  } = useModelDownloads();

  const [activePipelineTab, setActivePipelineTab] = useState<PipelineTab>("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");

  // Settings Topology subtab tracking
  const [activeVadSubTab, setActiveVadSubTab] = useState<string>("sensitivity");
  const [activeSttSubTab, setActiveSttSubTab] = useState<string>("streamingRate");
  const [activeLlmSubTab, setActiveLlmSubTab] = useState<LlmSubTab>("compute");
  const [activeTtsSubTab, setActiveTtsSubTab] = useState<TtsSubTab>("voice");

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


  const {
    remoteModels,
    loadingRemoteModels,
    remoteModelsError,
    probingMap,
    customModelId,
    setCustomModelId,
    customModelStatus,
    handleProbeCapabilities,
    handleValidateCustomModel,
  } = useRemoteLlmProbing(provider, activePipelineTab, isRemoteLlm);

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

  // 2. Custom Voices & Edge TTS
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

  // 3. Remote TTS Health Polling (gated by preview tier, visibility & active tab)
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

  useEffect(() => {
    localStorage.setItem("vox_ssh_conn", sshConnectionString);
  }, [sshConnectionString]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_port", sshPort);
  }, [sshPort]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_key", sshIdentityKey);
  }, [sshIdentityKey]);

  const triggerRemoteSetup = async () => {
    if (!isRemoteTts) return;
    setSetupStatus({ progress: 10, step: "initiating", log_line: "Starting connection..." });
    try {
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
