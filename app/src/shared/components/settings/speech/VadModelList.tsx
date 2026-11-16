import { memo } from "react";
import { AudioWaveform } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { SpeechModelCard } from "./SpeechModelCard";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { promoteActive } from "@/shared/lib/activeSelection";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { VadModelsModal } from "./VadModelsModal";
import { VadSettingsPanel } from "./VadSettingsPanel";
import { getSettingsInlineListMaxHeightClass } from "../shared/settingsLayout";

export interface VadModelListProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

/**
 * Voice-detection models with integrated detection parameters below a clean divider,
 * wrapped in an ExpandableList opening VadModelsModal.
 */
export const VadModelList = memo((props: VadModelListProps) => {
  const {
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  } = props;

  const activeVadBackend = useSettingsStore((s) => s.draftSettings?.vad?.vad_backend ?? "");
  const committedVadBackend = useSettingsStore((s) => s.settings?.vad?.vad_backend ?? "");
  const hasVad = useSettingsStore((s) => !!s.draftSettings?.vad);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const modelCatalog = useCatalogStore((s) => s.modelCatalog);

  if (!hasVad || !modelCatalog) return null;

  const rawVadModels = modelCatalog?.vad ?? [];
  const vadModels = promoteActive(rawVadModels, committedVadBackend);

  return (
    <ExpandableList
      className="flex-1 min-h-0 flex flex-col"
      inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
      icon={<AudioWaveform size={16} className="text-[rgb(var(--accent))]" />}
      title={SPEECH_CARD_COPY.vadModelsTitle}
      modalSubtitle={SPEECH_CARD_COPY.vadModelsSubtitle}
      expandLabel={SPEECH_CARD_COPY.expandVad}
      ariaLabel={SPEECH_CARD_COPY.vadModelsAria}
      headerActions={<SettingsCommitControls domainId="speech" layoutMode={layoutMode} />}
      modalContent={() => <VadModelsModal {...props} />}
    >
      <div
        className={cn(
          "flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1 flex flex-col gap-4",
          getSettingsInlineListMaxHeightClass(layoutMode)
        )}
      >
        {/* VAD Models Grid */}
        <div
          className={cn(
            "grid gap-3 auto-rows-max content-start",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          {vadModels.map((model) => {
            const isBuiltIn = !!model.is_built_in;
            const isDownloaded = isBuiltIn || !!modelPresence[model.id];
            const isActive = model.id === activeVadBackend;

            return (
              <SpeechModelCard
                key={model.id}
                id={model.id}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || (isBuiltIn ? "Built-in" : "ONNX")}
                ramUsage={model.ram_usage || "0 MB"}
                tradeoffs={model.tradeoffs || ""}
                isDownloaded={isDownloaded}
                isActive={isActive}
                isBuiltIn={isBuiltIn}
                badge={
                  isBuiltIn
                    ? SPEECH_CARD_COPY.tierBuiltIn
                    : SPEECH_CARD_COPY.tierOnDevice
                }
                layoutMode={layoutMode}
                onSelect={() => updateDraft("vad", "vad_backend", model.id)}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={downloadStatuses[model.id]}
                startDownload={() => startDownload(model.id)}
                deleteModel={() => deleteModel(model.id)}
              />
            );
          })}
        </div>

        {/* Integrated Detection Knobs below divider */}
        <div className="flex flex-col gap-2 pt-3 border-t border-[rgba(var(--foreground),0.06)] mt-1">
          <div className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 font-semibold select-none">
            {SPEECH_CARD_COPY.vadDividerLabel}
          </div>
          <VadSettingsPanel layoutMode={layoutMode} />
        </div>
      </div>
    </ExpandableList>
  );
});

VadModelList.displayName = "VadModelList";
