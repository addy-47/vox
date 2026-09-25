import React, { useState, useMemo, memo, useCallback } from "react";
import { useSettingsStore, type ProviderCaps } from "@/store/settingsStore";
import { Metronome, Microchip, Zap, Battery, Gauge } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { RotaryKnob, VoiceCarousel } from "@/shared/ui";
import { TTS_VOICE_MANAGER_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";
import { SettingsTabPane, PresetButton, PresetInput } from "./SettingsTabPane";

export interface CustomVoice {
  id: string;
  name: string;
  source_kind: string;
  has_preview: boolean;
  created_at: number;
}

export interface EdgeTtsVoice {
  name: string;
  short_name: string;
  gender: string;
  locale: string;
  friendly_name: string;
}

export interface TtsVoiceManagerProps {
  layoutMode?: "full-max" | "full-min" | "small";
  /** Preview provider id (manifest group id) — drives which settings render. */
  providerId: string;
  /** Backend capabilities; null while loading (flag-derived fallback applies). */
  caps: ProviderCaps | null;
  customVoices: CustomVoice[];
  loadCustomVoices: () => void;
  chatterboxIsAdding: boolean;
  setChatterboxIsAdding: React.Dispatch<React.SetStateAction<boolean>>;
  edgeTtsVoices: EdgeTtsVoice[];
  edgeTtsError: string | null;
  loadingEdgeVoices: boolean;
  loadEdgeVoices: () => void;
  activeCategoryTab?: "model" | "settings";
  activeSubTab?: "voice" | "speed" | "compute";
}

export type TtsSubTab = "voice" | "speed" | "compute";

const REGIONS = ["ALL", "US", "UK", "AU", "GLOBAL"] as const;

export const TtsVoiceManager = memo(({
  layoutMode,
  providerId,
  caps,
  customVoices,
  loadCustomVoices,
  chatterboxIsAdding,
  setChatterboxIsAdding,
  edgeTtsVoices,
  activeSubTab = "voice",
}: TtsVoiceManagerProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Region bucket filter for Edge TTS
  const [selectedRegion, setSelectedRegion] = useState<string>("ALL");

  const handlePrevRegion = useCallback(() => {
    setSelectedRegion((curr) => {
      const idx = REGIONS.indexOf(curr as any);
      const prevIdx = idx <= 0 ? REGIONS.length - 1 : idx - 1;
      return REGIONS[prevIdx];
    });
  }, []);

  const handleNextRegion = useCallback(() => {
    setSelectedRegion((curr) => {
      const idx = REGIONS.indexOf(curr as any);
      const nextIdx = idx >= REGIONS.length - 1 ? 0 : idx + 1;
      return REGIONS[nextIdx];
    });
  }, []);

  // Display names render as-is (data-driven). The id/name boundary:
  // `name` is UI text only, never a key.
  const displayName = (n: string) => n.trim() || n;

  const simplifyEdgeVoiceName = (shortName: string, friendlyName: string) => {
    const parts = shortName.split("-");
    const rawClean = (friendlyName || "").replace(/Microsoft | Server Speech.*Voice | Text to Speech/gi, "").trim();
    const cleanName = rawClean.split(" ")[0] || parts[parts.length - 1]?.replace("Neural", "") || shortName;
    const country =
      shortName.startsWith("en-US") ? "US" :
      shortName.startsWith("en-GB") ? "UK" :
      shortName.startsWith("en-AU") ? "AU" :
      shortName.startsWith("en-CA") ? "CA" :
      shortName.startsWith("es-ES") ? "ES" :
      shortName.startsWith("fr-FR") ? "FR" :
      shortName.startsWith("de-DE") ? "DE" :
      shortName.startsWith("ja-JP") ? "JP" :
      shortName.startsWith("hi-IN") ? "IN" :
      parts[1] || "";
    return `${cleanName}${country ? ` (${country})` : ""}`;
  };

  const edgeVoicesList = useMemo(() => {
    const rawList = edgeTtsVoices.length > 0 ? edgeTtsVoices : [
      { short_name: "en-US-AriaNeural", friendly_name: "Aria", gender: "Female", locale: "en-US", name: "" },
      { short_name: "en-US-GuyNeural", friendly_name: "Guy", gender: "Male", locale: "en-US", name: "" },
      { short_name: "en-US-JennyNeural", friendly_name: "Jenny", gender: "Female", locale: "en-US", name: "" },
      { short_name: "en-GB-SoniaNeural", friendly_name: "Sonia", gender: "Female", locale: "en-GB", name: "" },
      { short_name: "en-AU-NatashaNeural", friendly_name: "Natasha", gender: "Female", locale: "en-AU", name: "" },
    ];

    const bucketed = rawList.filter((v) => {
      if (selectedRegion === "ALL") return true;
      if (selectedRegion === "US") return v.locale.startsWith("en-US");
      if (selectedRegion === "UK") return v.locale.startsWith("en-GB");
      if (selectedRegion === "AU") return v.locale.startsWith("en-AU");
      if (selectedRegion === "GLOBAL") return !v.locale.startsWith("en-US") && !v.locale.startsWith("en-GB") && !v.locale.startsWith("en-AU");
      return true;
    });

    const finalVoices = bucketed.length > 0 ? bucketed : rawList;

    return finalVoices.map((v) => ({
      id: v.short_name,
      name: simplifyEdgeVoiceName(v.short_name, v.friendly_name),
    }));
  }, [edgeTtsVoices, selectedRegion]);

  if (!draftSettings) return null;

  const previewGroup = modelCatalog?.tts?.find((g) => g.id === providerId);
  const voiceSource = caps?.voices ?? (previewGroup?.is_cloud ? "edge" : previewGroup?.is_remote ? "custom" : "catalog");
  const allowClone = caps?.clone ?? !!previewGroup?.is_remote;
  const isEdgeTts = voiceSource === "edge";
  const isCustomVoices = voiceSource === "custom";
  const isRemoteGroup = !!previewGroup?.is_remote;

  const localVoices = isCustomVoices
    ? [
        { id: "default", name: "Default" },
        ...customVoices.map((v) => ({ id: v.id, name: displayName(v.name), isCustom: true })),
      ]
    : (modelCatalog?.voices || []).map((v) => ({ id: String(v.id), name: displayName(v.name) }));

  const activeVoices = isEdgeTts ? edgeVoicesList : localVoices;

  const customConfigKey = isRemoteGroup ? "chatterbox_remote" : "chatterbox";
  const customConfig = draftSettings.tts[customConfigKey];

  const selectedVoiceId = isEdgeTts
    ? draftSettings.tts.edge_tts?.voice || (edgeVoicesList[0]?.id || "en-US-AriaNeural")
    : isCustomVoices
      ? customConfig?.voice_id || "default"
      : String(draftSettings.tts.voice_index ?? 0);

  const handleVoiceChange = (id: string) => {
    if (isEdgeTts) {
      updateDraft("tts", "edge_tts", {
        ...draftSettings.tts.edge_tts,
        voice: id,
      });
    } else if (isCustomVoices) {
      updateDraft("tts", customConfigKey, {
        ...customConfig,
        voice_id: id === "default" ? null : id,
        language: customConfig?.language || "en",
      });
    } else {
      updateDraft("tts", "voice_index", Number(id));
    }
  };

  const copy = TTS_VOICE_MANAGER_COPY;

  return (
    <div
      className={cn(
        "w-full flex flex-col flex-1 min-h-0 select-none",
        layoutMode === "small" ? "h-auto py-1 space-y-2.5" : "h-full"
      )}
    >
      {/* TAB 1: SELECT VOICE */}
      {activeSubTab === "voice" && (
        <SettingsTabPane
          title={copy.voice.title}
          description={
            isEdgeTts ? (
              <span>
                {copy.voice.prefix}{" "}
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] font-bold select-none align-baseline">
                  <button
                    type="button"
                    onClick={handlePrevRegion}
                    className="hover:text-[rgb(var(--foreground))] transition-colors px-0.5 cursor-pointer font-black text-[12px]"
                    aria-label={TTS_VOICE_MANAGER_COPY.region.previous}
                  >
                    ‹
                  </button>
                  <span className="font-mono text-[10.5px] uppercase tracking-wider font-black">
                    {selectedRegion}
                  </span>
                  <button
                    type="button"
                    onClick={handleNextRegion}
                    className="hover:text-[rgb(var(--foreground))] transition-colors px-0.5 cursor-pointer font-black text-[12px]"
                    aria-label={TTS_VOICE_MANAGER_COPY.region.next}
                  >
                    ›
                  </button>
                </span>{" "}
                {copy.voice.suffix}
              </span>
            ) : (
              copy.voice.localDescription
            )
          }
          layoutMode={layoutMode}
          rightSlot={
            <div className="w-full max-w-[184px] flex items-center justify-center">
              <VoiceCarousel
                voices={activeVoices}
                selected={selectedVoiceId}
                onChange={handleVoiceChange}
                disabled={false}
                onVoicesChanged={loadCustomVoices}
                isAdding={chatterboxIsAdding}
                setIsAdding={setChatterboxIsAdding}
                allowClone={allowClone}
              />
            </div>
          }
        />
      )}

      {/* TAB 2: SPEECH SPEED / RATE */}
      {activeSubTab === "speed" && (
        <SettingsTabPane
          icon={Metronome}
          title={copy.speed.title}
          description={copy.speed.description}
          layoutMode={layoutMode}
          rightSlot={
            <div className="w-full flex items-center justify-center">
              <RotaryKnob
                value={draftSettings.tts.speed || 1.0}
                min={0.7}
                max={2.0}
                step={0.05}
                formatValue={(v) => `${v.toFixed(2)}x`}
                formatPreset={(v) => `${v}x`}
                onChange={(v) => updateDraft("tts", "speed", v)}
                presetSteps={[0.8, 1.0, 1.25]}
              />
            </div>
          }
        />
      )}

      {/* TAB 3: COMPUTE ALLOCATION */}
      {activeSubTab === "compute" && (() => {
        const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
        const balancedThreads = Math.max(1, Math.floor(totalCores / 2));
        const ecoThreads = Math.max(1, Math.floor(totalCores / 4));
        const currentThreads = draftSettings.tts.threads ?? 6;
        const currentProfile =
          currentThreads === totalCores ? "max"
          : currentThreads === balancedThreads ? "balanced"
          : currentThreads === ecoThreads ? "eco"
          : "custom";
        return (
          <SettingsTabPane
            icon={Microchip}
            title={COMPUTE_PROFILE_COPY.title}
            description={copy.compute.description}
            layoutMode={layoutMode}
            controls={
              <>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "balanced"}
                  onClick={() => updateDraft("tts", "threads", balancedThreads)}
                >
                  <Zap size={11} className="text-[rgb(var(--accent))]" />
                  <span>{COMPUTE_PROFILE_COPY.auto}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "eco"}
                  onClick={() => updateDraft("tts", "threads", ecoThreads)}
                >
                  <Battery size={11} className="text-emerald-400" />
                  <span>{COMPUTE_PROFILE_COPY.eco}</span>
                </PresetButton>
                <PresetButton
                  mono={false}
                  selected={currentProfile === "max"}
                  onClick={() => updateDraft("tts", "threads", totalCores)}
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
                      updateDraft("tts", "threads", num);
                    }
                  }}
                />
              </>
            }
          />
        );
      })()}
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
