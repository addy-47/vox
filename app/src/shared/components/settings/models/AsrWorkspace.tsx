import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";

interface AsrWorkspaceProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
}

export const AsrWorkspace = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    isGroupRequired,
  }: AsrWorkspaceProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!draftSettings || !modelCatalog) return null;

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        <div
          className={cn(
            "grid gap-2.5 h-full",
            modelCatalog.stt.length <= 2
              ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
              : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
          )}
        >
          {modelCatalog.stt.map((model) => {
            const isSelected = draftSettings.stt.embedded.model === model.id;
            const modelGroupId = model.id;
            const isDownloaded = !!modelPresence[modelGroupId];
            const status = downloadStatuses[modelGroupId];

            return (
              <SubModelCard
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
                layoutMode={layoutMode}
                onSelect={() => {
                  updateDraft("stt", "model", model.id);
                  updateDraft("stt", "embedded", {
                    model: model.id,
                  });
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
    );
  }
);

AsrWorkspace.displayName = "AsrWorkspace";
