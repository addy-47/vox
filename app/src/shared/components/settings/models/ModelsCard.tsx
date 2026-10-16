import { useState, useEffect, useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  checkTtsProviderHealth,
  getProviderCaps,
  setupRemoteServer,
} from "@/services/settingsService";
import * as eventsService from "@/services/eventsService";
import {
  MODEL_HUB_COPY,
  VAD_SETTINGS_COPY,
  STT_SETTINGS_COPY,
  LLM_SETTINGS_COPY,
} from "@/data/settingsCopy";
import { CLOUD_PROVIDERS } from "@/data/providersCopy";
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
import { isDesktop } from "@/lib/capabilities";
import { TtsVoiceManager, type TtsSubTab } from "./TtsVoiceManager";
import { VoiceCatalogView } from "./voices/VoiceCatalogView";
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

  const [isRemoteTtsHealthy, setIsRemoteTtsHealthy] = useState<boolean | null>(null);
  const [checkingTtsHealth, setCheckingTtsHealth] = useState(false);
  const [sshConnectionString, setSshConnectionString] = useState(() => localStorage.getItem("vox_ssh_conn") || "root@localhost");
  const [sshPort, setSshPort] = useState(() => localStorage.getItem("vox_ssh_port") || "22");
  const [sshIdentityKey, setSshIdentityKey] = useState(() => localStorage.getItem("vox_ssh_key") || "~/.ssh/id_rsa");
  const [setupStatus, setSetupStatus] = useState<RemoteSetupStatus | null>(null);

  // Base layout decisions on committed and draft settings
  const activeProviderKind = draftSettings?.llm?.active?.category ?? "embedded";
  const isRemoteLlm = activeProviderKind === "server" || activeProviderKind === "cloud";
  const isCloudLlm = activeProviderKind === "cloud";

  // Preview selection: the clicked card's group id IS the settings active key.
  // Tier and health gating derive from manifest flags and backend-derived
  // configs — the backend owns the provider union, the frontend never echoes it.
  const activeTtsId = draftSettings?.tts?.active?.model ?? "";
  const previewTtsGroup = modelCatalog?.tts?.find((g) => g.id === activeTtsId);
  const isCloudTts = !!previewTtsGroup?.is_cloud;
  const isRemoteTts = !!previewTtsGroup?.is_remote;

  // Settings capabilities for the preview TTS provider (caps-driven panes).
  const [ttsCaps, setTtsCaps] = useState<ProviderCaps | null>(null);

  useEffect(() => {
    let isMounted = true;
    if (!activeTtsId) {
      setTtsCaps(null);
      return;
    }
    getProviderCaps(activeTtsId).then((caps) => {
      if (isMounted) setTtsCaps(caps);
    }).catch(() => {
      if (isMounted) setTtsCaps(null);
    });
    return () => {
      isMounted = false;
    };
  }, [activeTtsId]);

  const provider: LlmProviderConfig = useMemo(() => {
    if (activeProviderKind === "embedded") {
      return { kind: "embedded" };
    }
    if (activeProviderKind === "server") {
      const serverId = draftSettings?.llm?.active?.provider;
      const entry = draftSettings?.providers?.llm?.servers?.find((s) => s.id === serverId);
      return {
        kind: "server",
        provider_id: serverId ?? undefined,
        base_url: entry?.url || "",
        model: draftSettings?.llm?.active?.model || "",
        api_key: entry?.api_key || undefined,
        provider_name: entry?.name || undefined,
      };
    }
    const cloudId = draftSettings?.llm?.active?.provider;
    const cloudEntry = CLOUD_PROVIDERS.find((p) => p.id === cloudId);
    return {
      kind: "cloud",
      provider_id: cloudId ?? undefined,
      base_url: cloudEntry?.url || "",
      model: draftSettings?.llm?.active?.model || "",
      api_key: (cloudId && draftSettings?.llm?.cloud_keys?.[cloudId]) || undefined,
      provider_name: cloudEntry?.name || undefined,
    };
  }, [
    activeProviderKind,
    draftSettings?.llm?.active?.provider,
    draftSettings?.llm?.active?.model,
    draftSettings?.llm?.cloud_keys,
    draftSettings?.providers?.llm?.servers,
  ]);


  const {
    remoteModels,
    loadingRemoteModels,
    remoteModelsError,
    probingMap,
    handleProbeCapabilities,
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

  // 2. Remote TTS Health Polling (gated by preview tier, visibility & active tab)
  useEffect(() => {
    if (!isRemoteTts) {
      setIsRemoteTtsHealthy(null);
      return;
    }

    const checkHealth = async () => {
      if (document.hidden || activePipelineTab !== "tts") return;
      setCheckingTtsHealth(true);
      try {
        const res = await checkTtsProviderHealth();
        setIsRemoteTtsHealthy(res.healthy);
      } catch (_) {
        setIsRemoteTtsHealthy(false);
      } finally {
        setCheckingTtsHealth(false);
      }
    };

    checkHealth();
    const interval = setInterval(checkHealth, 5000);
    return () => clearInterval(interval);
  }, [isRemoteTts, activePipelineTab, draftSettings?.tts?.active?.model]);

  useEffect(() => {
    return eventsService.onModelProgress((payload) => {
      if (payload?.model_id === "chatterbox_remote_server") {
        setSetupStatus({
          step: payload.step,
          progress: payload.progress,
          log_line: payload.error ? `Error: ${payload.error}` : `Step: ${payload.step}`,
          error: payload.error || undefined,
        });
        if ((payload.step === "completed" || payload.step === "Completed") && isRemoteTts) {
          checkTtsProviderHealth().then((res) => setIsRemoteTtsHealthy(res.healthy));
        }
      }
    });
  }, [isRemoteTts]);

  useEffect(() => {
    localStorage.setItem("vox_ssh_conn", sshConnectionString);
  }, [sshConnectionString]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_port", sshPort);
  }, [sshPort]);
  useEffect(() => {
    localStorage.setItem("vox_ssh_key", sshIdentityKey);
  }, [sshIdentityKey]);

  // Provisioning spawns `ssh`, which stock Android does not ship, so the setup
  // panel is desktop-only. A mobile user can still point TTS at an
  // already-provisioned remote server — that path is plain HTTP and works on
  // every platform (see .agents/rules/android-pitfalls.md trap 7).
  const showRemoteServerSetup = isRemoteTts && isRemoteTtsHealthy !== true && isDesktop();

  const triggerRemoteSetup = async () => {
    if (!isRemoteTts) return;
    setSetupStatus({ progress: 10, step: "initiating", log_line: "Starting connection..." });
    try {
      const ttsServers = draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
      const remoteCfg = ttsServers.find((s) => s.id === draftSettings?.tts?.active?.provider);
      const endpoint = remoteCfg?.url || "http://127.0.0.1:7860";
      const remotePath = remoteCfg?.remote_path || "~/.vox";

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

  const isSttCloud = draftSettings?.stt?.active?.category === "cloud";
  const activeSttModel = draftSettings?.stt?.active?.model;
  const isAsrVerified =
    isSttCloud
      ? true
      : !!(activeSttModel && modelPresence[activeSttModel]);

  const isLlmDownloaded =
    isRemoteLlm
      ? true
      : !!(draftSettings?.llm?.active?.model && modelPresence[draftSettings.llm.active.model]);

  // The preview active id IS the manifest group id — direct presence lookup.
  // Cloud needs nothing on disk; remote needs a healthy server.
  const isTtsVerified = isCloudTts
    ? true
    : isRemoteTts
    ? isRemoteTtsHealthy === true
    : !!modelPresence[activeTtsId];

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
      <div className="flex flex-col  flex-1 min-h-0">
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
                      { id: "speechOnset", label: VAD_SETTINGS_COPY.tabs.speechOnset, Icon: Zap },
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
            layoutMode === "small" ? "h-auto" : "h-full"
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
                      draftSettings?.llm?.active?.model || ""
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
                  />
                )
              )}

              {activePipelineTab === "tts" && (
                <>
                  {activeCategoryTab === "model" ? (
                    showRemoteServerSetup ? (
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
                  ) : activeTtsSubTab === "voice" ? (
                    <VoiceCatalogView
                      layoutMode={layoutMode}
                      providerId={activeTtsId}
                    />
                  ) : (
                    <TtsVoiceManager
                      layoutMode={layoutMode}
                      caps={ttsCaps}
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
