import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface TtsModelWorkspaceProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
  isRemoteTtsHealthy: boolean | null;
  checkingTtsHealth: boolean;
}

export const TtsModelWorkspace = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
    isRemoteTtsHealthy,
    checkingTtsHealth,
  }: TtsModelWorkspaceProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!draftSettings || !modelCatalog) return null;

    // Tier derives from the preview group's manifest flags — never id literals.
    const previewGroup = modelCatalog?.tts?.find((m) => m.id === draftSettings.tts.active);
    const isRemote = !!previewGroup?.is_remote;
    const isCloud = !!previewGroup?.is_cloud;

    // Strictly partition models according to the active provider tier (Embedded, Remote, or Cloud) using manifest flags
    const filteredModels = (modelCatalog.tts || []).filter((model) => {
      if (isCloud) {
        return !!model.is_cloud;
      }
      if (isRemote) {
        return !!model.is_remote;
      }
      // Embedded / Local Tier
      return !model.is_cloud && !model.is_remote;
    });

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        <div
          className={cn(
            "grid gap-2.5 h-full",
            filteredModels.length <= 2
              ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
              : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
          )}
        >
          {filteredModels.map((model) => {
            // Single source: manifest group id IS the settings active key.
            const isSelected = model.id === draftSettings.tts.active;

            const isDownloaded =
              !!model.is_cloud || !!model.is_remote || !!model.is_built_in || !!modelPresence[model.id];
            const status = downloadStatuses[model.id];

            return (
              <div key={model.id} className="relative h-full">
                <SubModelCard
                  id={model.id}
                  name={model.name}
                  description={model.description || ""}
                  parameters={model.parameters || ""}
                  ramUsage={model.ram_usage}
                  tradeoffs={model.tradeoffs}
                  isDownloaded={!!isDownloaded}
                  isActive={!!isSelected}
                  isRequired={false}
                  layoutMode={layoutMode}
                  onSelect={() => {
                    updateDraft("tts", "active", model.id);
                  }}
                  confirmDeleteId={confirmDeleteId}
                  setConfirmDeleteId={setConfirmDeleteId}
                  downloadStatus={status}
                  startDownload={() => startDownload(model.id)}
                  deleteModel={() => deleteModel(model.id)}
                />
                {model.is_remote && isSelected && (
                  <div className="absolute top-2.5 right-2.5 flex items-center gap-1.5 select-none pointer-events-none">
                    {checkingTtsHealth ? (
                      <Loader2 size={10} className="animate-spin text-[rgb(var(--accent))]" />
                    ) : isRemoteTtsHealthy === true ? (
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.7)]" />
                    ) : isRemoteTtsHealthy === false ? (
                      <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shadow-[0_0_6px_rgba(239,68,68,0.7)] animate-pulse" />
                    ) : null}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  }
);

TtsModelWorkspace.displayName = "TtsModelWorkspace";
