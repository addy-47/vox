import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";

interface VadWorkspaceProps {
  activeCategoryTab: "model" | "settings";
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

export const VadWorkspace = memo(
  ({
    activeCategoryTab,
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: VadWorkspaceProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!draftSettings) return null;
    const activeVadBackend = draftSettings.vad?.vad_backend || "earshot";
    const vadModels = modelCatalog?.vad || [];

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-2.5 h-full",
              vadModels.length <= 2
                ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr" : "grid-cols-2 grid-rows-1")
                : (layoutMode === "small" ? "grid-cols-1 auto-rows-full snap-y snap-mandatory" : "grid-cols-2 auto-rows-full snap-y snap-mandatory")
            )}
          >
            {vadModels.map((model) => {
              const isBuiltIn = !!model.is_built_in;
              const isDownloaded = isBuiltIn || !!modelPresence[model.id];
              // Single source: manifest group id IS the vad_backend key.
              const isActive = model.id === activeVadBackend;

              return (
                <SubModelCard
                  key={model.id}
                  id={model.id}
                  name={model.name}
                  description={model.description || ""}
                  parameters={model.parameters || (isBuiltIn ? "Built-in" : "ONNX")}
                  ramUsage={model.ram_usage || "0 MB"}
                  tradeoffs={model.tradeoffs || ""}
                  isDownloaded={isDownloaded}
                  isActive={isActive}
                  isRequired={isBuiltIn}
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
        ) : (
          <div className="space-y-4 p-1">
            <div className="space-y-2">
              <span className="text-[13px] text-[rgb(var(--foreground))] font-bold block">
                Silence Threshold
              </span>
              <div className="flex gap-1">
                {[
                  { label: "Sensitive", value: 0.3 },
                  { label: "Balanced", value: 0.5 },
                  { label: "Conservative", value: 0.7 },
                  { label: "Aggressive", value: 0.9 },
                ].map(({ label, value }) => (
                  <button
                    type="button"
                    key={value}
                    onClick={() => updateDraft("vad", "threshold", value)}
                    className={cn(
                      "flex-1 py-1.5 rounded-lg text-[12px] font-bold uppercase tracking-wider transition-all duration-300 cursor-pointer",
                      Math.abs(draftSettings.vad.threshold - value) < 0.01
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                        : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    );
  }
);

VadWorkspace.displayName = "VadWorkspace";
