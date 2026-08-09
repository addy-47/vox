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

    if (!draftSettings) return null;
    const activeVadBackend = draftSettings.vad?.vad_backend || "earshot";

    return (
      <div className="space-y-3">
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-3",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
            )}
          >
            <SubModelCard
              id="earshot"
              name="Earshot (Built-in)"
              description="Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load."
              parameters="Built-in"
              ramUsage="0 MB"
              isDownloaded={true}
              isActive={activeVadBackend === "earshot"}
              isRequired={true}
              layoutMode={layoutMode}
              onSelect={() => updateDraft("vad", "vad_backend", "earshot")}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              startDownload={() => {}}
              deleteModel={() => {}}
            />
            <SubModelCard
              id="ten_vad"
              name="TenVAD Engine"
              description="ONNX-based voice detector. Requires downloading auxiliary neural files."
              parameters="ONNX"
              ramUsage="~2 MB"
              isDownloaded={!!modelPresence["ten_vad"]}
              isActive={activeVadBackend === "ten_vad"}
              isRequired={false}
              layoutMode={layoutMode}
              onSelect={() => updateDraft("vad", "vad_backend", "ten_vad")}
              confirmDeleteId={confirmDeleteId}
              setConfirmDeleteId={setConfirmDeleteId}
              downloadStatus={downloadStatuses["ten_vad"]}
              startDownload={() => startDownload("ten_vad")}
              deleteModel={() => deleteModel("ten_vad")}
            />
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
