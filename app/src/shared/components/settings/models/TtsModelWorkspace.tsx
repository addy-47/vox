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

    const isRemote = draftSettings.tts.active === "chatterbox_remote";
    const isCloud = draftSettings.tts.active === "edge_tts";

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
      <div className="space-y-3">
        <div
          className={cn(
            "grid gap-2.5",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          {filteredModels.map((model) => {
            const isSelected =
              (model.is_cloud && draftSettings.tts.active === "edge_tts") ||
              (model.id === "supertonic_tts" && draftSettings.tts.active === "supertonic") ||
              (model.id === "chatterbox_tts" && draftSettings.tts.active === "chatterbox") ||
              (model.is_remote && draftSettings.tts.active === "chatterbox_remote");

            const isDownloaded =
              !!model.is_cloud || !!model.is_remote || !!model.is_built_in || !!modelPresence[model.id];
            const status = downloadStatuses[model.id];

            return (
              <div key={model.id} className="relative">
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
                    if (model.id === "edge_tts") {
                      updateDraft("tts", "active", "edge_tts");
                    } else if (model.id === "supertonic_tts") {
                      updateDraft("tts", "active", "supertonic");
                    } else if (model.id === "chatterbox_tts") {
                      updateDraft("tts", "active", "chatterbox");
                    } else if (model.id === "chatterbox_remote") {
                      updateDraft("tts", "active", "chatterbox_remote");
                    }
                  }}
                  confirmDeleteId={confirmDeleteId}
                  setConfirmDeleteId={setConfirmDeleteId}
                  downloadStatus={status}
                  startDownload={() => startDownload(model.id)}
                  deleteModel={() => deleteModel(model.id)}
                />
                {model.id === "chatterbox_remote" && isSelected && (
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
