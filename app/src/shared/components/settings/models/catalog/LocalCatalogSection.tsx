import { memo, useMemo } from "react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { SubModelCard } from "../../SubModelCard";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";

export interface LocalCatalogSectionProps {
  layoutMode?: "full-max" | "full-min" | "small";
  selectedLlmId: string;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  startDownload: (id: string) => void;
  handleDeleteModelGroup: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
}

export const LocalCatalogSection = memo(({
  layoutMode,
  selectedLlmId,
  modelPresence,
  downloadStatuses,
  confirmDeleteId,
  setConfirmDeleteId,
  startDownload,
  handleDeleteModelGroup,
  isGroupRequired,
}: LocalCatalogSectionProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const committedSettings = useSettingsStore((s) => s.settings);

  const committedLocalModelId = useMemo(
    () => committedSettings?.llm?.embedded?.model,
    [committedSettings]
  );

  // Model Tab: Local GGUF Model Grid (Pinned: saved model first)
  const sortedLocalModels = useMemo(() => {
    const list = [...(modelCatalog?.llm || [])];
    if (committedLocalModelId) {
      const idx = list.findIndex((m) => m.id === committedLocalModelId);
      if (idx > 0) {
        const [selected] = list.splice(idx, 1);
        list.unshift(selected);
      }
    }
    return list;
  }, [modelCatalog?.llm, committedLocalModelId]);

  const { containerRef: localGridRef, lenisRef: localLenisRef } =
    useLenisScrollContainer<HTMLDivElement>();
  useScrollTopOnChange(localGridRef, committedLocalModelId, localLenisRef);

  return (
    <div
      ref={localGridRef}
      className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar pr-1 h-full"
    >
      <div
        className={cn(
          "grid gap-2.5 min-h-full w-full",
          sortedLocalModels.length <= 2
            ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
            : (layoutMode === "small" ? "grid-cols-1 auto-rows-max content-start" : "grid-cols-2 auto-rows-max content-start")
        )}
      >
        {sortedLocalModels.map((model) => {
          const isSelected = selectedLlmId === model.id;
          const isDownloaded = modelPresence[model.id];
          const status = downloadStatuses[model.id];

          return (
            <SubModelCard
              key={model.id}
              id={model.id}
              name={model.name}
              description={model.description || ""}
              parameters={model.parameters || ""}
              ramUsage={model.ram_usage}
              tradeoffs={model.tradeoffs}
              isDownloaded={isDownloaded}
              isActive={isSelected}
              isRequired={isGroupRequired(model.id)}
              layoutMode={layoutMode}
              onSelect={() => {
                const draft = useSettingsStore.getState().draftSettings;
                if (draft?.llm?.embedded) {
                  updateDraft("llm", "embedded", { ...draft.llm.embedded, model: model.id });
                }
              }}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              downloadStatus={status}
              startDownload={() => startDownload(model.id)}
              deleteModel={() => handleDeleteModelGroup(model.id)}
              showTooltip={false}
            />
          );
        })}
      </div>
    </div>
  );
});

LocalCatalogSection.displayName = "LocalCatalogSection";
