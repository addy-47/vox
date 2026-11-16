import { memo, useState, useMemo } from "react";
import { ArrowLeft, Check, Cloud, Cpu, Download, Layers, Trash2 } from "lucide-react";
import { MODEL_HUB_COPY, SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { activeSelection, promoteActive } from "@/shared/lib/activeSelection";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CatalogFacetRail, type CatalogFacetGroup } from "../shared/CatalogFacetRail";
import { CatalogModalShell } from "../shared/CatalogModalShell";
import { SelectableCard } from "../shared/SelectableCard";
import { ModelLogo } from "./ModelLogo";
import { SpeechFlowBanner } from "./SpeechFlowBanner";
import { cn } from "@/shared/lib/utils";

type Facet = "all" | "local" | "cloud";

export interface SttModelsModalProps {
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
 * Expanded STT surface: speech-to-text models rendered as full SelectableCard
 * tiles with leading brand marks, live telemetry badges, and a left facet rail.
 */
export const SttModelsModal = memo(
  ({
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    isGroupRequired,
  }: SttModelsModalProps) => {
    const [facet, setFacet] = useState<Facet>("all");
    const activeSttId = useSettingsStore((s) => s.draftSettings?.stt.active.model);
    const committedSttId = useSettingsStore((s) => s.settings?.stt.active.model);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useCatalogStore((s) => s.modelCatalog);

    const rawSttModels = useMemo(() => modelCatalog?.stt ?? [], [modelCatalog]);
    const sttModels = useMemo(
      () => promoteActive(rawSttModels, committedSttId),
      [rawSttModels, committedSttId]
    );

    const counts = useMemo(() => {
      let local = 0;
      let cloud = 0;
      for (const m of rawSttModels) {
        if (m.is_cloud) cloud++;
        else local++;
      }
      return { all: rawSttModels.length, local, cloud };
    }, [rawSttModels]);

    const filteredModels = useMemo(() => {
      if (facet === "local") return sttModels.filter((m) => !m.is_cloud);
      if (facet === "cloud") return sttModels.filter((m) => !!m.is_cloud);
      return sttModels;
    }, [sttModels, facet]);

    const activeModel = activeSttId ?? rawSttModels[0]?.id;
    const activeModelName =
      rawSttModels.find((m) => m.id === activeModel)?.name ?? activeModel ?? "Nemotron";

    const groups: CatalogFacetGroup[] = [
      {
        title: SPEECH_CARD_COPY.sourceFacet,
        items: [
          { id: "all", label: SPEECH_CARD_COPY.facetAll, Icon: Layers, count: counts.all },
          { id: "local", label: SPEECH_CARD_COPY.facetLocal, Icon: Cpu, count: counts.local },
          ...(counts.cloud > 0
            ? [{ id: "cloud", label: SPEECH_CARD_COPY.facetCloud, Icon: Cloud, count: counts.cloud }]
            : []),
        ],
      },
    ];

    return (
      <CatalogModalShell
        facetRail={
          <CatalogFacetRail
            groups={groups}
            selectedId={facet}
            onSelect={(id) => setFacet(id as Facet)}
          />
        }
        footer={<SpeechFlowBanner kind="stt" engineName={activeModelName} />}
      >
        <div className="grid gap-3.5 grid-cols-1 md:grid-cols-2 lg:grid-cols-2 xl:grid-cols-3 auto-rows-max content-start p-1">
          {filteredModels.map((model) => {
            const isSelected = activeModel === model.id;
            const modelGroupId = model.id;
            const isDownloaded = model.is_cloud || !!modelPresence[modelGroupId];
            const status = downloadStatuses[modelGroupId];
            const downloading = !isDownloaded && !!status && status.step !== "completed";
            const isConfirmingDelete = confirmDeleteId === modelGroupId;
            const canDelete = isDownloaded && !model.is_cloud && !isGroupRequired(model.id);

            return (
              <SelectableCard
                key={model.id}
                id={model.id}
                name={model.name}
                tagline={model.description || ""}
                isSelected={isSelected}
                onSelect={() => {
                  updateDraft(
                    "stt",
                    "active",
                    activeSelection(model.is_cloud ? "cloud" : "embedded", model.id)
                  );
                }}
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
                cornerSlot={
                  isSelected ? (
                    <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] text-[10.5px] font-mono font-bold tracking-wider">
                      <Check size={11} strokeWidth={3} /> ACTIVE
                    </span>
                  ) : (
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]">
                      {model.parameters || (model.is_cloud ? "Cloud API" : "ASR")}
                    </span>
                  )
                }
                actionSlot={
                  <div className="flex items-center justify-between gap-2 text-[11px] font-mono w-full">
                    <span className="flex items-center gap-1.5 min-w-0">
                      <span className="truncate text-[rgb(var(--foreground-muted))]/70">
                        {model.is_cloud ? SPEECH_CARD_COPY.tierCloud : SPEECH_CARD_COPY.tierOnDevice}
                      </span>
                      {model.ram_usage && (
                        <span className="text-[10px] text-[rgb(var(--foreground-muted))]/50">
                          · {model.ram_usage}
                        </span>
                      )}
                      {!model.is_cloud && !isDownloaded && (
                        downloading ? (
                          <span className="px-1 font-bold text-[rgb(var(--accent))]">
                            {Math.round(status!.progress)}%
                          </span>
                        ) : (
                          <Tooltip label={MODEL_HUB_COPY.row.downloadModel} side="top" asChild>
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                startDownload(modelGroupId);
                              }}
                              className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
                              aria-label={MODEL_HUB_COPY.row.downloadModel}
                            >
                              <Download size={14} strokeWidth={2.2} />
                            </button>
                          </Tooltip>
                        )
                      )}
                    </span>
                    <span className="ml-auto">
                      {isConfirmingDelete ? (
                        <span className="flex items-center gap-1">
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              deleteModel(modelGroupId);
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
                                setConfirmDeleteId(modelGroupId);
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
          })}
        </div>
      </CatalogModalShell>
    );
  }
);

SttModelsModal.displayName = "SttModelsModal";
