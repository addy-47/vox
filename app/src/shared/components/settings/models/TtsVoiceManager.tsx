import { memo } from "react";
import { useSettingsStore, type ProviderCaps } from "@/store/settingsStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { Zap, Battery, Gauge } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { RotaryKnob } from "@/shared/ui";
import { TTS_VOICE_MANAGER_COPY, COMPUTE_PROFILE_COPY, LLM_SETTINGS_COPY } from "@/data/settingsCopy";
import { CustomSvgs, PresetButton, PresetInput, RemoteComputeGraphic } from "./CustomSvgs";

export interface TtsVoiceManagerProps {
  layoutMode?: "full-max" | "full-min" | "small";
  /** Backend capabilities; null while loading (flag-derived fallback applies). */
  caps: ProviderCaps | null;
  activeSubTab?: "voice" | "speed" | "compute";
}

export type TtsSubTab = "voice" | "speed" | "compute";

/**
 * TTS settings panes (speech rate + compute). Voice selection lives in
 * VoiceCatalogView; this component renders only the non-voice subtabs.
 */
export const TtsVoiceManager = memo(({ layoutMode, caps, activeSubTab = "speed" }: TtsVoiceManagerProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);

  if (!draftSettings) return null;

  const ttsModelId =
    draftSettings.tts?.active?.model || draftSettings.tts?.active_voice_id || null;
  return (
    <TtsVoiceManagerBody
      layoutMode={layoutMode}
      caps={caps}
      activeSubTab={activeSubTab}
      modelId={ttsModelId}
    />
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";

const TtsVoiceManagerBody = memo(({
  layoutMode,
  caps,
  activeSubTab,
  modelId,
}: {
  layoutMode: TtsVoiceManagerProps["layoutMode"];
  caps: ProviderCaps | null;
  activeSubTab: "voice" | "speed" | "compute";
  modelId: string | null;
}) => {
  const { params, setParam } = useModelParameters(modelId);
  const numParam = (v: unknown): number | undefined =>
    typeof v === "number" && !Number.isNaN(v) ? v : undefined;
  const copy = TTS_VOICE_MANAGER_COPY;

  const isCloud = caps?.execution === "cloud";
  const isRemote = caps?.execution === "remote";
  const isRemoteOrCloud = isCloud || isRemote;

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 select-none",
        layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
      )}
    >
      {/* TAB 2: SPEECH SPEED / RATE */}
      {activeSubTab === "speed" && (() => {
        // Bounds come from the provider, not the UI. Loading fallback matches
        // five of six providers and is replaced when caps arrive.
        const range = caps?.speed_range ?? { min: 0.7, max: 2.0, step: 0.05 };
        return (
          <CustomSvgs
            title={copy.speed.title}
            description={copy.speed.description}
            layoutMode={layoutMode}
            rightSlot={
              <div className="w-full flex items-center justify-center">
                <RotaryKnob
                  value={numParam(params.speed) ?? 1.0}
                  min={range.min}
                  max={range.max}
                  step={range.step}
                  formatValue={(v) => `${v.toFixed(2)}x`}
                  formatPreset={(v) => `${v}x`}
                  onChange={(v) => setParam("speed", v)}
                  presetSteps={[0.8, 1.0, 1.25]}
                />
              </div>
            }
          />
        );
      })()}

      {/* TAB 3: COMPUTE ALLOCATION */}
      {activeSubTab === "compute" && (() => {
        if (isRemoteOrCloud) {
          return (
            <CustomSvgs
              title={isCloud ? LLM_SETTINGS_COPY.compute.remoteTitleLocal : LLM_SETTINGS_COPY.compute.remoteTitleRemote}
              description={LLM_SETTINGS_COPY.compute.remoteDescription}
              layoutMode={layoutMode}
              emptyGraphic={<RemoteComputeGraphic />}
            />
          );
        }
        const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
        const balancedThreads = Math.max(1, Math.floor(totalCores / 2));
        const ecoThreads = Math.max(1, Math.floor(totalCores / 4));
        const currentThreads = numParam(params.threads) ?? 6;
        const currentProfile =
          currentThreads === totalCores ? "max"
          : currentThreads === balancedThreads ? "balanced"
          : currentThreads === ecoThreads ? "eco"
          : "custom";
        return (
          <CustomSvgs
            title={COMPUTE_PROFILE_COPY.title}
            description={copy.compute.description}
            layoutMode={layoutMode}
            rightSlot={
              <div className="grid grid-cols-2 gap-1.5 w-full max-w-[155px]">
                <PresetButton
                  mono={false}
                  selected={currentProfile === "balanced"}
                  onClick={() => setParam("threads", balancedThreads)}
                >
                  <Zap size={11} className="text-[rgb(var(--accent))]" />
                  <span>{COMPUTE_PROFILE_COPY.auto}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "eco"}
                  onClick={() => setParam("threads", ecoThreads)}
                >
                  <Battery size={11} className="text-[rgb(var(--success))]" />
                  <span>{COMPUTE_PROFILE_COPY.eco}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "max"}
                  onClick={() => setParam("threads", totalCores)}
                >
                  <Gauge size={11} className="text-[rgb(var(--warn-soft))]" />
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
                      setParam("threads", num);
                    }
                  }}
                />
              </div>
            }
          />
        );
      })()}
    </div>
  );
});

TtsVoiceManagerBody.displayName = "TtsVoiceManagerBody";
