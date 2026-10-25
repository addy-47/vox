import { memo } from "react";
import { ArrowLeft, Check, ChevronRight, Download, Trash2 } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { activeSelection, promoteActive } from "@/shared/lib/activeSelection";
import { Tooltip } from "@/shared/ui/Tooltip";
import { MODEL_HUB_COPY, SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { CatalogReveal } from "../shared/CatalogReveal";
import { SelectableCard } from "../shared/SelectableCard";
import { ModelLogo } from "./ModelLogo";
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
  /**
   * Row layout (the inline card face) or tile layout (the expanded modal).
   * The inline face keeps its rows: it lives in a 260px card and a grid of
   * 188px tiles does not fit there. Only the modal renders tiles.
   */
  variant?: "rows" | "cards";
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
    variant = "rows",
  }: TtsEngineGridProps) => {
    const activeTtsId = useSettingsStore((s) => s.draftSettings?.tts.active.model ?? "");
    const committedTtsId = useSettingsStore((s) => s.settings?.tts.active.model);
    const hasDraft = useSettingsStore((s) => !!s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useCatalogStore((s) => s.modelCatalog);
    if (!hasDraft || !modelCatalog) return null;
    const rawEngines = (modelCatalog.tts || []).filter((model) => {
      if (model.is_remote) return false;
      if (facet === "local") return !model.is_cloud;
      if (facet === "cloud") return !!model.is_cloud;
      return true;
    });
    const engines = promoteActive(rawEngines, committedTtsId);

    const selectEngine = (model: (typeof engines)[number]) => {
      updateDraft(
        "tts",
        "active",
        model.is_cloud
          ? activeSelection("cloud", model.id, model.id)
          : activeSelection("embedded", model.id)
      );
    };

    const voicesAction = (id: string, name: string) =>
      onPickVoice ? (
        <Tooltip label={SPEECH_CARD_COPY.openVoices} side="top" asChild>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onPickVoice(id);
            }}
            aria-label={`${SPEECH_CARD_COPY.openVoices}: ${name}`}
            className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-colors cursor-pointer"
          >
            <ChevronRight size={15} strokeWidth={2} />
          </button>
        </Tooltip>
      ) : undefined;

    const grid = (
      <div className="w-full">
        <div
          className={cn(
            "grid gap-3 auto-rows-max content-start",
            variant === "cards"
              ? "grid-cols-1 sm:grid-cols-2 2xl:grid-cols-3"
              : columns === 3
                ? "grid-cols-1 sm:grid-cols-2 xl:grid-cols-3"
                : "grid-cols-1 sm:grid-cols-2"
          )}
        >
          {engines.map((model) => {
            const isCloud = !!model.is_cloud;
            const isSelected = model.id === activeTtsId;
            const isDownloaded = isCloud || !!model.is_built_in || !!modelPresence[model.id];
            const badge = isCloud ? SPEECH_CARD_COPY.tierCloud : SPEECH_CARD_COPY.tierOnDevice;
            const downloadStatus: ModelStatus | undefined = downloadStatuses[model.id];
            const downloading = !isDownloaded && !!downloadStatus && downloadStatus.step !== "completed";
            const isConfirmingDelete = confirmDeleteId === model.id;
            const canDelete = isDownloaded && !model.is_built_in;

            if (variant === "cards") {
              return (
                <SelectableCard
                  key={model.id}
                  id={model.id}
                  name={model.name}
                  tagline={model.description || ""}
                  isSelected={isSelected}
                  onSelect={() => selectEngine(model)}
                  leading={
                    <div
                      className={cn(
                        "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
                        isSelected
                          ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
                          : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
                      )}
                    >
                      <ModelLogo modelId={model.id} size={26} />
                    </div>
                  }
                  cornerSlot={voicesAction(model.id, model.name)}
                  actionSlot={
                    <div className="flex items-center justify-between gap-2 text-[11px] font-mono w-full">
                      <span className="flex items-center gap-1.5 min-w-0">
                        <span className="truncate text-[rgb(var(--foreground-muted))]/70">{badge}</span>
                        {!isDownloaded &&
                          (downloading ? (
                            <span className="px-1 font-bold text-[rgb(var(--accent))]">
                              {Math.round(downloadStatus!.progress)}%
                            </span>
                          ) : (
                            <Tooltip label={MODEL_HUB_COPY.row.downloadModel} side="top" asChild>
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  startDownload(model.id);
                                }}
                                className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
                                aria-label={MODEL_HUB_COPY.row.downloadModel}
                              >
                                <Download size={14} strokeWidth={2.2} />
                              </button>
                            </Tooltip>
                          ))}
                      </span>
                      <span className="ml-auto">
                        {isConfirmingDelete ? (
                          <span className="flex items-center gap-1">
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                deleteModel(model.id);
                                setConfirmDeleteId(null);
                              }}
                              className="rounded-md bg-[rgba(var(--danger),0.18)] p-1 text-[rgb(var(--danger))] transition-colors hover:bg-[rgba(var(--danger),0.3)] cursor-pointer"
                              aria-label={MODEL_HUB_COPY.row.confirmDelete}
                            >
                              <Check size={12} strokeWidth={3} />
                            </button>
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                setConfirmDeleteId(null);
                              }}
                              className="rounded-md p-1 text-[rgb(var(--foreground-muted))] transition-colors hover:bg-[rgba(var(--foreground),0.1)] cursor-pointer"
                              aria-label={MODEL_HUB_COPY.row.cancel}
                            >
                              <ArrowLeft size={12} strokeWidth={2.4} />
                            </button>
                          </span>
                        ) : (
                          canDelete && (
                            <Tooltip label={MODEL_HUB_COPY.row.deleteWeights} side="top" asChild>
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setConfirmDeleteId(model.id);
                                }}
                                className="rounded-md p-1 text-[rgb(var(--foreground-muted))]/70 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity hover:bg-[rgba(var(--danger),0.14)] hover:text-[rgb(var(--danger))] cursor-pointer"
                                aria-label={MODEL_HUB_COPY.row.deleteWeights}
                              >
                                <Trash2 size={13} strokeWidth={2} />
                              </button>
                            </Tooltip>
                          )
                        )}
                      </span>
                    </div>
                  }
                />
              );
            }

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
                onSelect={() => selectEngine(model)}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={downloadStatus}
                startDownload={() => startDownload(model.id)}
                deleteModel={() => deleteModel(model.id)}
                badge={badge}
                cornerAction={voicesAction(model.id, model.name)}
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
