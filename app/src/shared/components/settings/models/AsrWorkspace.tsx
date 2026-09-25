import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";
import { Microchip, Zap, Battery, Gauge } from "lucide-react";
import { STT_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import { SettingsTabPane, PresetButton, PresetInput } from "./SettingsTabPane";
import { TransliterationToggle } from "./TransliterationToggle";

interface AsrWorkspaceProps {
  activeCategoryTab?: "model" | "settings";
  activeSubTab?: string;
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
    activeCategoryTab = "model",
    activeSubTab = "streamingRate",
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

    const currentThrottle = draftSettings.stt?.embedded?.partial_throttle_ms ?? 300;
    const transliterateEnabled = draftSettings.stt?.transliterate_enabled ?? true;

    const throttleCustom = ![150, 300, 500].includes(currentThrottle);

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        {activeCategoryTab === "model" ? (
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
      ) : (
        <div
          className={cn(
            "w-full flex flex-col",
            layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
          )}
        >
          {/* SUBTAB 1: STREAMING CADENCE */}
          {activeSubTab === "streamingRate" && (
            <SettingsTabPane
              title={STT_SETTINGS_COPY.streamingRate.title}
              description={STT_SETTINGS_COPY.streamingRate.description}
              layoutMode={layoutMode}
              controls={
                <>
                  {[
                    { label: "150ms", val: 150 },
                    { label: "300ms", val: 300 },
                    { label: "500ms", val: 500 },
                  ].map(({ label, val }) => (
                    <PresetButton
                      key={val}
                      selected={currentThrottle === val}
                      onClick={() => updateDraft("stt", "partial_throttle_ms", val)}
                    >
                      {label}
                    </PresetButton>
                  ))}
                  <PresetInput
                    selected={throttleCustom}
                    value={throttleCustom ? `${currentThrottle}ms` : ""}
                    placeholder={COMPUTE_PROFILE_COPY.custom}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const num = parseInt(clean, 10);
                      if (!isNaN(num) && num >= 50 && num <= 1500) {
                        updateDraft("stt", "partial_throttle_ms", num);
                      }
                    }}
                  />
                </>
              }
            />
          )}

          {/* SUBTAB 2: TRANSLITERATION */}
          {activeSubTab === "transliteration" && (
            <SettingsTabPane
              title={STT_SETTINGS_COPY.transliteration.title}
              description={STT_SETTINGS_COPY.transliteration.description}
              layoutMode={layoutMode}
              rightSlot={
                <TransliterationToggle
                  enabled={transliterateEnabled}
                  onToggle={() => updateDraft("stt", "transliterate_enabled", !transliterateEnabled)}
                />
              }
            />
          )}

          {/* SUBTAB 3: COMPUTE ALLOCATION */}
          {activeSubTab === "compute" && (() => {
            const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
            const optimalThreads = Math.max(2, totalCores - 2);
            const ecoThreads = Math.max(1, Math.floor(totalCores / 2));
            const currentThreads = draftSettings.stt?.embedded?.threads ?? 4;
            const currentProfile =
              currentThreads === totalCores ? "max"
              : currentThreads === optimalThreads ? "auto"
              : currentThreads === ecoThreads ? "eco"
              : "custom";
            return (
              <SettingsTabPane
                icon={Microchip}
                title={COMPUTE_PROFILE_COPY.title}
                description={STT_SETTINGS_COPY.compute.description}
                layoutMode={layoutMode}
                controls={
                  <>
                    <PresetButton
                      mono={false}
                      selected={currentProfile === "auto"}
                      onClick={() => updateDraft("stt", "threads", optimalThreads)}
                    >
                      <Zap size={11} className="text-[rgb(var(--accent))]" />
                      <span>{COMPUTE_PROFILE_COPY.auto}</span>
                    </PresetButton>
                    <PresetButton
                      mono={false}
                      selected={currentProfile === "eco"}
                      onClick={() => updateDraft("stt", "threads", ecoThreads)}
                    >
                      <Battery size={11} className="text-emerald-400" />
                      <span>{COMPUTE_PROFILE_COPY.eco}</span>
                    </PresetButton>
                    <PresetButton
                      mono={false}
                      selected={currentProfile === "max"}
                      onClick={() => updateDraft("stt", "threads", totalCores)}
                    >
                      <Gauge size={11} className="text-amber-400" />
                      <span>{COMPUTE_PROFILE_COPY.max}</span>
                    </PresetButton>
                    <PresetInput
                      selected={currentProfile === "custom"}
                      value={currentProfile === "custom" ? `${currentThreads}T` : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 1 && num <= 64) {
                          updateDraft("stt", "threads", num);
                        }
                      }}
                    />
                  </>
                }
              />
            );
          })()}
        </div>
      )}
    </div>
  );
});

AsrWorkspace.displayName = "AsrWorkspace";
