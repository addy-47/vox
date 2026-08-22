import { memo } from "react";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";

interface AuxiliaryWorkspaceProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

export const AuxiliaryWorkspace = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: AuxiliaryWorkspaceProps) => {
    const { modelCatalog } = useSettingsStore();
    const auxiliaryModels = modelCatalog?.auxiliary || [];

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        <div
          className={cn(
            "grid gap-2.5 auto-rows-max content-start",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
          )}
        >
          {auxiliaryModels.map((model) => (
            <SubModelCard
              key={model.id}
              id={model.id}
              name={model.name}
              description={model.description || ""}
              parameters={model.parameters || "ONNX"}
              ramUsage={model.ram_usage || "N/A"}
              tradeoffs={model.tradeoffs || ""}
              isDownloaded={!!modelPresence[model.id]}
              isActive={!!modelPresence[model.id]}
              isRequired={false}
              layoutMode={layoutMode}
              onSelect={() => {}}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              downloadStatus={downloadStatuses[model.id]}
              startDownload={() => startDownload(model.id)}
              deleteModel={() => deleteModel(model.id)}
            />
          ))}
        </div>
      </div>
    );
  }
);

AuxiliaryWorkspace.displayName = "AuxiliaryWorkspace";
