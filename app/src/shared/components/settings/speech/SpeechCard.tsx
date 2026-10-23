import { useState, useEffect, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { onSpeechTabRequest } from "@/shared/lib/speechTabSync";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";

import {
  AudioLines,
  AudioWaveform,
  ArrowLeft,
  Ear,
  Loader2,
  Settings2,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

import { useModelDownloads } from "@/shared/hooks/useModelDownloads";

import { PipelineTopologyMap, type PipelineTopologyNode } from "../shared/PipelineTopologyMap";
import { CardTitleBar } from "../shared/CardTitleBar";
import { SttModelList } from "./SttModelList";
import { TtsModelList } from "./TtsModelList";
import { VadModelList } from "./VadModelList";
import { VadSettingsPanel } from "./VadSettingsPanel";
import { VoiceCatalogView } from "./voices/VoiceCatalogView";
import { SETTINGS_DOMAIN_CARD_HEIGHT_CLASS } from "../shared/settingsLayout";

type SpeechTab = "vad" | "stt" | "tts";

interface SpeechCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

/**
 * Speech card (08:00). Owns everything that turns audio into text and text into
 * audio: voice detection, speech transcription, and text to speech.
 *
 * It has no Model | Settings toggle. The header carries one control, Select
 * voice, which swaps the whole card face for the voice catalog.
 */
export const SpeechCard = memo(({ layoutMode = "full-max" }: SpeechCardProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const modelCatalog = useCatalogStore((s) => s.modelCatalog);
  const isCategoryDirty = useSettingsStore((s) => s.isCategoryDirty);

  useEffect(() => {
    if (!modelCatalog) {
      void useCatalogStore.getState().loadModelCatalog();
    }
  }, [modelCatalog]);

  const {
    downloadStatuses,
    modelPresence,
    confirmDeleteId,
    setConfirmDeleteId,
    startDownload,
    handleDeleteModelGroup,
    isGroupRequired,
  } = useModelDownloads();

  const [activeTab, setActiveTab] = useState<SpeechTab>("tts");
  const [view, setView] = useState<"models" | "voices">("models");
  const [vadSettingsOpen, setVadSettingsOpen] = useState(false);

  // Interaction card -> this card. Only the speech stages are shared now.
  useEffect(() => onSpeechTabRequest((tab) => setActiveTab(tab)), []);

  if (!draftSettings) return null;

  const activeTtsId = draftSettings.tts.active.model ?? "";

  const nodes: PipelineTopologyNode<SpeechTab>[] = [
    {
      id: "tts",
      label: SPEECH_CARD_COPY.tabs.tts,
      shortLabel: SPEECH_CARD_COPY.tabs.ttsShort,
      caption: SPEECH_CARD_COPY.tabCaptions.tts,
      Icon: AudioLines,
      isDirty: isCategoryDirty("tts"),
    },
    {
      id: "stt",
      label: SPEECH_CARD_COPY.tabs.stt,
      shortLabel: SPEECH_CARD_COPY.tabs.sttShort,
      caption: SPEECH_CARD_COPY.tabCaptions.stt,
      Icon: Ear,
      isDirty: isCategoryDirty("stt"),
    },
    {
      id: "vad",
      label: SPEECH_CARD_COPY.tabs.vad,
      shortLabel: SPEECH_CARD_COPY.tabs.vadShort,
      caption: SPEECH_CARD_COPY.tabCaptions.vad,
      Icon: AudioWaveform,
      isDirty: isCategoryDirty("vad"),
    },
  ];

  const isRealtime = draftSettings.interaction.pipeline_mode === "realtime";
  const activeVoiceProvider = isRealtime
    ? (draftSettings.realtime.active.provider ?? "gemini_live")
    : activeTtsId;

  return (
    <div
      className={cn(
        "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
        layoutMode === "small"
          ? "bg-transparent p-0 h-auto"
          : cn("glass-card p-5", SETTINGS_DOMAIN_CARD_HEIGHT_CLASS)
      )}
    >
      {isRealtime || view === "voices" ? (
        <VoiceCatalogView
          layoutMode={layoutMode}
          providerId={activeVoiceProvider}
          onBack={isRealtime ? undefined : () => setView("models")}
        />
      ) : (
        <div className="flex flex-col flex-1 min-h-0">
          {/* Header: title on the left, the single voice control on the right */}
          <CardTitleBar
            icon={<AudioLines size={17} />}
            title={SPEECH_CARD_COPY.title}
            actionLabel={SPEECH_CARD_COPY.selectVoice}
            onAction={() => setView("voices")}
          />

          <PipelineTopologyMap
            nodes={nodes}
            activeTab={activeTab}
            onChangeTab={(id) => {
              setActiveTab(id);
              setVadSettingsOpen(false);
            }}
            layoutMode={layoutMode}
            ariaLabel={SPEECH_CARD_COPY.topologyAria}
          />

          {/* Workspace: no panel chrome of its own. The tiles sit directly on the
              card, so the grid never reads as a box inside a box. */}
          <div
            className={cn(
              "flex-1 w-full flex flex-col min-h-0 relative",
              layoutMode === "small" ? "h-auto" : "h-full"
            )}
          >
            {!modelCatalog ? (
              <div className="flex-1 flex items-center justify-center py-12">
                <Loader2 className="animate-spin text-[rgb(var(--accent))]" size={24} />
              </div>
            ) : (
              <>
                {activeTab === "vad" && (
                  <>
                    {/* Sits in the same bottom-right corner as the expand
                        trigger, in the same size: no row, no reserved height. */}
                    <div className="absolute -bottom-2 right-1 z-20">
                      <Tooltip
                        label={
                          vadSettingsOpen
                            ? SPEECH_CARD_COPY.backToVadModels
                            : SPEECH_CARD_COPY.vadSettings
                        }
                        side="top"
                        asChild
                      >
                        <button
                          type="button"
                          onClick={() => setVadSettingsOpen((v) => !v)}
                          aria-label={
                            vadSettingsOpen
                              ? SPEECH_CARD_COPY.backToVadModels
                              : SPEECH_CARD_COPY.vadSettings
                          }
                          aria-expanded={vadSettingsOpen}
                          className={cn(
                            "w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] shadow-sm",
                            "border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.85)]",
                            vadSettingsOpen
                              ? "text-[rgb(var(--accent))] border-[rgba(var(--accent),0.35)]"
                              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.95)]"
                          )}
                        >
                          {vadSettingsOpen ? (
                            <ArrowLeft size={13} strokeWidth={2} />
                          ) : (
                            <Settings2 size={13} strokeWidth={1.75} />
                          )}
                        </button>
                      </Tooltip>
                    </div>

                    {vadSettingsOpen ? (
                      <VadSettingsPanel layoutMode={layoutMode} />
                    ) : (
                      <VadModelList
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

                {activeTab === "stt" && (
                  <SttModelList
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

                {activeTab === "tts" && (
                  <TtsModelList
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
      )}
    </div>
  );
});

SpeechCard.displayName = "SpeechCard";
