import { memo } from "react";
import { AudioWaveform } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { activeSelection } from "@/shared/lib/activeSelection";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { CatalogReveal } from "../shared/CatalogReveal";
import { SpeechModelCard } from "./SpeechModelCard";

export interface TtsEngineGridProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
  /** Opens the engine's voice list. Only rendered by the expanded modal. */
  onPickVoice?: (engineId: string) => void;
  /** Narrows the grid by tier. Ignored when absent. */
  facet?: "all" | "local" | "cloud";
  columns?: 2 | 3;
  /** Height classes for the scroll container. */
  className?: string;
}

/**
 * The seven TTS engines as one flat grid: four on-device, three cloud.
 * The self-hosted server engine is excluded — provisioning that server is its
 * own flow and lives in the backlog.
 */
export const TtsEngineGrid = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    onPickVoice,
    facet = "all",
    columns = 2,
    className,
  }: TtsEngineGridProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useCatalogStore((s) => s.modelCatalog);
    if (!draftSettings || !modelCatalog) return null;

    const activeTtsId = draftSettings.tts.active.model ?? "";
    const engines = (modelCatalog.tts || []).filter((model) => {
      if (model.is_remote) return false;
      if (facet === "local") return !model.is_cloud;
      if (facet === "cloud") return !!model.is_cloud;
      return true;
    });

    const grid = (
      <div className="w-full">
        <div
          className={cn(
            "grid gap-3 auto-rows-max content-start",
            columns === 3 ? "grid-cols-1 sm:grid-cols-2 xl:grid-cols-3" : "grid-cols-1 sm:grid-cols-2"
          )}
        >
          {engines.map((model) => {
            const isCloud = !!model.is_cloud;
            const isSelected = model.id === activeTtsId;
            const isDownloaded = isCloud || !!model.is_built_in || !!modelPresence[model.id];

            return (
              <SpeechModelCard
                key={model.id}
                id={model.id}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || ""}
                ramUsage={model.ram_usage}
                tradeoffs={model.tradeoffs}
                isDownloaded={isDownloaded}
                isActive={isSelected}
                isBuiltIn={!!model.is_built_in}
                layoutMode={layoutMode}
                onSelect={() => {
                  updateDraft(
                    "tts",
                    "active",
                    isCloud
                      ? activeSelection("cloud", model.id, model.id)
                      : activeSelection("embedded", model.id)
                  );
                }}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={downloadStatuses[model.id]}
                startDownload={() => startDownload(model.id)}
                deleteModel={() => deleteModel(model.id)}
                badge={
                  isCloud ? SPEECH_CARD_COPY.tierCloud : SPEECH_CARD_COPY.tierOnDevice
                }
                cornerAction={
                  onPickVoice ? (
                    <Tooltip label={SPEECH_CARD_COPY.openVoices} side="top" asChild>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onPickVoice(model.id);
                        }}
                        aria-label={`${SPEECH_CARD_COPY.openVoices}: ${model.name}`}
                        className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-colors cursor-pointer"
                      >
                        <AudioWaveform size={15} strokeWidth={1.75} />
                      </button>
                    </Tooltip>
                  ) : undefined
                }
              />
            );
          })}
        </div>
      </div>
    );

    return (
      <CatalogReveal
        scrollerClassName={cn("w-full overflow-y-auto custom-scrollbar pr-1", className)}
      >
        {grid}
      </CatalogReveal>
    );
  }
);

TtsEngineGrid.displayName = "TtsEngineGrid";
