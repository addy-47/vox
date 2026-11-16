import { memo, useState, useMemo } from "react";
import { ArrowLeft, Check, Cpu, Download, Layers, ShieldCheck, Trash2 } from "lucide-react";
import { MODEL_HUB_COPY, SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { promoteActive } from "@/shared/lib/activeSelection";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { Tooltip } from "@/shared/ui/Tooltip";
import { CatalogFacetRail, type CatalogFacetGroup } from "../shared/CatalogFacetRail";
import { CatalogModalShell } from "../shared/CatalogModalShell";
import { SelectableCard } from "../shared/SelectableCard";
import { ModelLogo } from "./ModelLogo";
import { SpeechFlowBanner } from "./SpeechFlowBanner";
import { VadSettingsPanel } from "./VadSettingsPanel";
import { cn } from "@/shared/lib/utils";

type Facet = "all" | "builtin" | "onnx";

export interface VadModelsModalProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

/**
 * Expanded VAD surface: voice-activity detection models rendered as full
 * SelectableCard tiles with leading marks, live status, integrated detection
 * parameters, and a domain-specific flow banner footer.
 */
export const VadModelsModal = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: VadModelsModalProps) => {
    const [facet, setFacet] = useState<Facet>("all");
    const activeVadBackend = useSettingsStore((s) => s.draftSettings?.vad?.vad_backend ?? "");
    const committedVadBackend = useSettingsStore((s) => s.settings?.vad?.vad_backend ?? "");
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useCatalogStore((s) => s.modelCatalog);

    const rawVadModels = useMemo(() => modelCatalog?.vad ?? [], [modelCatalog]);
    const vadModels = useMemo(
      () => promoteActive(rawVadModels, committedVadBackend),
      [rawVadModels, committedVadBackend]
    );

    const counts = useMemo(() => {
      let builtin = 0;
      let onnx = 0;
      for (const m of rawVadModels) {
        if (m.is_built_in) builtin++;
        else onnx++;
      }
      return { all: rawVadModels.length, builtin, onnx };
    }, [rawVadModels]);

    const filteredModels = useMemo(() => {
      if (facet === "builtin") return vadModels.filter((m) => !!m.is_built_in);
      if (facet === "onnx") return vadModels.filter((m) => !m.is_built_in);
      return vadModels;
    }, [vadModels, facet]);

    const activeModelName =
      rawVadModels.find((m) => m.id === activeVadBackend)?.name ?? activeVadBackend ?? "Silero VAD";

    const groups: CatalogFacetGroup[] = [
      {
        title: SPEECH_CARD_COPY.sourceFacet,
        items: [
          { id: "all", label: SPEECH_CARD_COPY.facetAll, Icon: Layers, count: counts.all },
          { id: "builtin", label: SPEECH_CARD_COPY.tierBuiltIn, Icon: ShieldCheck, count: counts.builtin },
          { id: "onnx", label: "ONNX", Icon: Cpu, count: counts.onnx },
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
        footer={<SpeechFlowBanner kind="vad" engineName={activeModelName} />}
      >
        <div className="flex flex-col gap-6 p-1">
          {/* VAD Models Grid as spacious SelectableCard tiles */}
          <div className="grid gap-3.5 grid-cols-1 md:grid-cols-2 lg:grid-cols-2 xl:grid-cols-3 auto-rows-max content-start">
            {filteredModels.map((model) => {
              const isBuiltIn = !!model.is_built_in;
              const isDownloaded = isBuiltIn || !!modelPresence[model.id];
              const isActive = model.id === activeVadBackend;
              const status = downloadStatuses[model.id];
              const downloading = !isDownloaded && !!status && status.step !== "completed";
              const isConfirmingDelete = confirmDeleteId === model.id;
              const canDelete = isDownloaded && !isBuiltIn;

              return (
                <SelectableCard
                  key={model.id}
                  id={model.id}
                  name={model.name}
                  tagline={model.description || ""}
                  isSelected={isActive}
                  onSelect={() => updateDraft("vad", "vad_backend", model.id)}
                  leading={
                    <div
                      className={cn(
                        "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
                        isActive
                          ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
                          : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
                      )}
                    >
                      <ModelLogo modelId={model.id} size={26} />
                    </div>
                  }
                  cornerSlot={
                    isActive ? (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] text-[10.5px] font-mono font-bold tracking-wider">
                        <Check size={11} strokeWidth={3} /> ACTIVE
                      </span>
                    ) : (
                      <span className="text-[10px] font-mono px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]">
                        {isBuiltIn ? "Built-in" : "ONNX"}
                      </span>
                    )
                  }
                  actionSlot={
                    <div className="flex items-center justify-between gap-2 text-[11px] font-mono w-full">
                      <span className="flex items-center gap-1.5 min-w-0">
                        <span className="truncate text-[rgb(var(--foreground-muted))]/70">
                          {isBuiltIn ? SPEECH_CARD_COPY.tierBuiltIn : SPEECH_CARD_COPY.tierOnDevice}
                        </span>
                        {model.ram_usage && (
                          <span className="text-[10px] text-[rgb(var(--foreground-muted))]/50">
                            · {model.ram_usage}
                          </span>
                        )}
                        {!isBuiltIn && !isDownloaded && (
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
                                  startDownload(model.id);
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
            })}
          </div>

          {/* Integrated Detection Knobs below clean divider */}
          <div className="flex flex-col gap-3 pt-4 border-t border-[rgba(var(--foreground),0.08)]">
            <div className="px-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80 font-bold select-none">
              {SPEECH_CARD_COPY.vadDividerLabel}
            </div>
            <VadSettingsPanel layoutMode={layoutMode} />
          </div>
        </div>
      </CatalogModalShell>
    );
  }
);

VadModelsModal.displayName = "VadModelsModal";
