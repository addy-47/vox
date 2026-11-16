import { memo } from "react";
import { Ear } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { SpeechModelCard } from "./SpeechModelCard";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { activeSelection, promoteActive } from "@/shared/lib/activeSelection";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { SttModelsModal } from "./SttModelsModal";
import { getSettingsInlineListMaxHeightClass } from "../shared/settingsLayout";

export interface SttModelListProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
}

/**
 * Speech-transcription models with inline grid and ExpandableList
 * modal trigger opening SttModelsModal.
 */
export const SttModelList = memo((props: SttModelListProps) => {
  const {
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    isGroupRequired,
  } = props;

  const activeSttId = useSettingsStore((s) => s.draftSettings?.stt.active.model);
  const committedSttId = useSettingsStore((s) => s.settings?.stt.active.model);
  const hasDraft = useSettingsStore((s) => !!s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const modelCatalog = useCatalogStore((s) => s.modelCatalog);

  if (!hasDraft || !modelCatalog) return null;

  const rawSttModels = modelCatalog?.stt ?? [];
  const sttModels = promoteActive(rawSttModels, committedSttId);

  return (
    <ExpandableList
      className="flex-1 min-h-0 flex flex-col"
      inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
      icon={<Ear size={16} className="text-[rgb(var(--accent))]" />}
      title={SPEECH_CARD_COPY.sttModelsTitle}
      modalSubtitle={SPEECH_CARD_COPY.sttModelsSubtitle}
      expandLabel={SPEECH_CARD_COPY.expandStt}
      ariaLabel={SPEECH_CARD_COPY.sttModelsAria}
      headerActions={<SettingsCommitControls domainId="speech" layoutMode={layoutMode} />}
      modalContent={() => <SttModelsModal {...props} />}
    >
      <div
        className={cn(
          "flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1",
          getSettingsInlineListMaxHeightClass(layoutMode)
        )}
      >
        <div
          className={cn(
            "grid gap-3 auto-rows-max content-start",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          {sttModels.map((model) => {
            const activeModel = activeSttId ?? modelCatalog.stt[0]?.id;
            const isSelected = activeModel === model.id;
            const modelGroupId = model.id;
            const isDownloaded = !!modelPresence[modelGroupId];
            const status = downloadStatuses[modelGroupId];

            return (
              <SpeechModelCard
                key={model.id}
                id={modelGroupId}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || ""}
                ramUsage={model.ram_usage}
                tradeoffs={model.tradeoffs}
                isDownloaded={isDownloaded}
                isActive={isSelected}
                isRequired={isGroupRequired(model.id)}
                badge={SPEECH_CARD_COPY.tierOnDevice}
                layoutMode={layoutMode}
                onSelect={() => {
                  updateDraft("stt", "active", activeSelection("embedded", model.id));
                }}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={status}
                startDownload={() => startDownload(modelGroupId)}
                deleteModel={() => deleteModel(modelGroupId)}
              />
            );
          })}
        </div>
      </div>
    </ExpandableList>
  );
});

SttModelList.displayName = "SttModelList";
