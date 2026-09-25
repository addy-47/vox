import { memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";
import { VAD_SETTINGS_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import { SettingsTabPane, PresetButton, PresetInput } from "./SettingsTabPane";

interface VadWorkspaceProps {
  activeCategoryTab: "model" | "settings";
  activeSubTab?: string;
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
    activeSubTab = "sensitivity",
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: VadWorkspaceProps) => {
    const vad = useSettingsStore((s) => s.draftSettings?.vad);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);

    if (!vad) return null;
    const activeVadBackend = vad.vad_backend || "earshot";
    const vadModels = modelCatalog?.vad || [];
    if (process.env.NODE_ENV === "development") {
      console.log("[VadWorkspace] Loaded VAD models:", vadModels);
    }

    const currentThreshold = vad.threshold ?? 0.5;
    const currentSilenceMs = vad.silence_duration_ms ?? 800;
    const currentSpeechOnsetMs = vad.speech_onset_ms ?? 32;
    const currentNoiseGate = vad.ptt_noise_gate ?? 0.005;

    const thresholdCustom = ![0.3, 0.5, 0.7].some((v) => Math.abs(currentThreshold - v) < 0.04);
    const silenceCustom = ![400, 800, 1200].includes(currentSilenceMs);
    const onsetCustom = ![32, 64, 128].includes(currentSpeechOnsetMs);
    const noiseGateCustom = ![0.001, 0.005, 0.020].some((v) => Math.abs(currentNoiseGate - v) < 0.0015);

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1">
        {activeCategoryTab === "model" ? (
          <div
            className={cn(
              "grid gap-2.5",
              vadModels.length <= 2
                ? (layoutMode === "small" ? "grid-cols-1 auto-rows-fr h-full" : "grid-cols-2 grid-rows-1 h-full")
                : (layoutMode === "small" ? "grid-cols-1 auto-rows-auto" : "grid-cols-2 auto-rows-auto")
            )}
          >
            {vadModels.map((model) => {
              const isBuiltIn = !!model.is_built_in;
              const isDownloaded = isBuiltIn || !!modelPresence[model.id];
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
          <div
            className={cn(
              "w-full flex flex-col",
              layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
            )}
          >
            {/* SUBTAB 1: SENSITIVITY (THRESHOLD) */}
            {activeSubTab === "sensitivity" && (
              <SettingsTabPane
                title={VAD_SETTINGS_COPY.sensitivity.title}
                description={VAD_SETTINGS_COPY.sensitivity.description}
                layoutMode={layoutMode}
                controls={
                  <>
                    {[
                      { label: "30%", val: 0.3 },
                      { label: "50%", val: 0.5 },
                      { label: "70%", val: 0.7 },
                    ].map(({ label, val }) => (
                      <PresetButton
                        key={val}
                        selected={Math.abs(currentThreshold - val) < 0.04}
                        onClick={() => updateDraft("vad", "threshold", val)}
                      >
                        {label}
                      </PresetButton>
                    ))}
                    <PresetInput
                      selected={thresholdCustom}
                      value={thresholdCustom ? `${Math.round(currentThreshold * 100)}%` : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 5 && num <= 95) {
                          updateDraft("vad", "threshold", num / 100);
                        }
                      }}
                    />
                  </>
                }
              />
            )}

            {/* SUBTAB 2: SILENCE CUTOFF (DURATION) */}
            {activeSubTab === "silence" && (
              <SettingsTabPane
                title={VAD_SETTINGS_COPY.silence.title}
                description={VAD_SETTINGS_COPY.silence.description}
                layoutMode={layoutMode}
                controls={
                  <>
                    {[
                      { label: "400ms", val: 400 },
                      { label: "800ms", val: 800 },
                      { label: "1200ms", val: 1200 },
                    ].map(({ label, val }) => (
                      <PresetButton
                        key={val}
                        selected={currentSilenceMs === val}
                        onClick={() => updateDraft("vad", "silence_duration_ms", val)}
                      >
                        {label}
                      </PresetButton>
                    ))}
                    <PresetInput
                      selected={silenceCustom}
                      value={silenceCustom ? `${currentSilenceMs}ms` : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 100 && num <= 3000) {
                          updateDraft("vad", "silence_duration_ms", num);
                        }
                      }}
                    />
                  </>
                }
              />
            )}

            {/* SUBTAB 3: SPEECH ONSET DURATION */}
            {activeSubTab === "speechOnset" && (
              <SettingsTabPane
                title={VAD_SETTINGS_COPY.speechOnset.title}
                description={VAD_SETTINGS_COPY.speechOnset.description}
                layoutMode={layoutMode}
                controls={
                  <>
                    {[
                      { label: "32ms", val: 32 },
                      { label: "64ms", val: 64 },
                      { label: "128ms", val: 128 },
                    ].map(({ label, val }) => (
                      <PresetButton
                        key={val}
                        selected={currentSpeechOnsetMs === val}
                        onClick={() => updateDraft("vad", "speech_onset_ms", val)}
                      >
                        {label}
                      </PresetButton>
                    ))}
                    <PresetInput
                      selected={onsetCustom}
                      value={onsetCustom ? `${currentSpeechOnsetMs}ms` : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9]/g, "");
                        if (!clean) return;
                        const num = parseInt(clean, 10);
                        if (!isNaN(num) && num >= 16 && num <= 1000) {
                          updateDraft("vad", "speech_onset_ms", num);
                        }
                      }}
                    />
                  </>
                }
              />
            )}

            {/* SUBTAB 4: NOISE GATE */}
            {activeSubTab === "noiseGate" && (
              <SettingsTabPane
                title={VAD_SETTINGS_COPY.noiseGate.title}
                description={VAD_SETTINGS_COPY.noiseGate.description}
                layoutMode={layoutMode}
                controls={
                  <>
                    {[
                      { label: "Studio", val: 0.001 },
                      { label: "Normal", val: 0.005 },
                      { label: "Noisy", val: 0.020 },
                    ].map(({ label, val }) => (
                      <PresetButton
                        key={val}
                        selected={Math.abs(currentNoiseGate - val) < 0.0015}
                        onClick={() => updateDraft("vad", "ptt_noise_gate", val)}
                      >
                        {label}
                      </PresetButton>
                    ))}
                    <PresetInput
                      selected={noiseGateCustom}
                      value={noiseGateCustom ? currentNoiseGate.toFixed(3) : ""}
                      placeholder={COMPUTE_PROFILE_COPY.custom}
                      inputMode="decimal"
                      onChange={(e) => {
                        const clean = e.target.value.replace(/[^0-9.]/g, "");
                        if (!clean) return;
                        const num = parseFloat(clean);
                        if (!isNaN(num) && num >= 0.001 && num <= 0.09) {
                          updateDraft("vad", "ptt_noise_gate", num);
                        }
                      }}
                    />
                  </>
                }
              />
            )}
          </div>
        )}
      </div>
    );
  }
);

VadWorkspace.displayName = "VadWorkspace";
