import React, { useState, useMemo, memo, useCallback } from "react";
import { useSettingsStore, type ProviderCaps } from "@/store/settingsStore";
import { Metronome, Microchip, Zap, Battery, Gauge } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { RotaryKnob } from "@/shared/ui";
import { VoiceCarousel } from "../voice/VoiceCarousel";
import { TTS_VOICE_MANAGER_COPY, COMPUTE_PROFILE_COPY } from "@/data/settingsCopy";

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
      ? customConfig?.language || "default"
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
        language: id,
      });
    } else {
      updateDraft("tts", "voice_index", Number(id));
    }
  };

  const copy = TTS_VOICE_MANAGER_COPY;
  const isSmall = layoutMode === "small";

  const effectiveSubTab = activeSubTab;

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* ─── Layer: Subtab Workspace (Full Height Ergonomics) ─── */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          isSmall ? "h-auto py-1" : "h-full"
        )}
      >
        {/* TAB 1: SELECT VOICE */}
        {effectiveSubTab === "voice" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {copy.voice.title}
              </span>
              {isEdgeTts ? (
                <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
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
                </p>
              ) : (
                <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                  {copy.voice.localDescription}
                </p>
              )}
            </div>

            {/* Right Side: Voice Selector Carousel */}
            <div className="shrink-0 w-[150px] sm:w-[175px] h-full flex flex-col justify-center">
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
          </div>
        )}

        {/* TAB 2: SPEECH SPEED / RATE */}
        {effectiveSubTab === "speed" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                  <Metronome size={14} className="text-[rgb(var(--accent))]" />
                  {copy.speed.title}
                </span>
                <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                  {(draftSettings.tts.speed || 1.0).toFixed(2)}x
                </span>
              </div>
              <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                {copy.speed.description}
              </p>
            </div>

            <div className="shrink-0 flex items-center justify-center pl-1">
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
          </div>
        )}
        {/* TAB 3: COMPUTE ALLOCATION */}
        {effectiveSubTab === "compute" && (() => {
          const totalCores = (typeof navigator !== "undefined" ? navigator.hardwareConcurrency : undefined) || 4;
          const balancedThreads = Math.max(1, Math.floor(totalCores / 2));
          const ecoThreads = Math.max(1, Math.floor(totalCores / 4));
          const currentThreads = draftSettings.tts.threads ?? 2;
          const currentProfile =
            currentThreads === totalCores ? "max"
            : currentThreads === balancedThreads ? "balanced"
            : currentThreads === ecoThreads ? "eco"
            : "custom";
          return (
            <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
              <div className="flex flex-col gap-1 min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] flex items-center gap-1.5">
                    <Microchip size={14} className="text-[rgb(var(--accent))]" />
                    {COMPUTE_PROFILE_COPY.title}
                  </span>
                  <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
                    {currentThreads} / {totalCores} Cores
                  </span>
                </div>
                <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
                  {TTS_VOICE_MANAGER_COPY.compute.description}
                </p>
              </div>

              <div className="shrink-0 grid grid-cols-2 gap-1.5 w-[116px] sm:w-[136px]">
                <button
                  type="button"
                  onClick={() => updateDraft("tts", "threads", balancedThreads)}
                  className={cn(
                    "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                    currentProfile === "balanced"
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <Zap size={11} className="text-[rgb(var(--accent))]" />
                  <span>{COMPUTE_PROFILE_COPY.auto}</span>
                </button>

                <button
                  type="button"
                  onClick={() => updateDraft("tts", "threads", ecoThreads)}
                  className={cn(
                    "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                    currentProfile === "eco"
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <Battery size={11} className="text-emerald-400" />
                  <span>{COMPUTE_PROFILE_COPY.eco}</span>
                </button>

                <button
                  type="button"
                  onClick={() => updateDraft("tts", "threads", totalCores)}
                  className={cn(
                    "py-1 rounded-lg border text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1",
                    currentProfile === "max"
                      ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                      : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <Gauge size={11} className="text-amber-400" />
                  <span>{COMPUTE_PROFILE_COPY.max}</span>
                </button>

                <div className={cn(
                  "rounded-lg border flex items-center justify-center transition-all overflow-hidden",
                  currentProfile === "custom"
                    ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
                    : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
                )}>
                  <input
                    type="text"
                    inputMode="numeric"
                    value={currentProfile === "custom" ? `${currentThreads}T` : ""}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const num = parseInt(clean, 10);
                      if (!isNaN(num) && num >= 1 && num <= 64) {
                        updateDraft("tts", "threads", num);
                      }
                    }}
                    placeholder={COMPUTE_PROFILE_COPY.custom}
                    className="w-full text-center text-[10.5px] font-mono font-bold bg-transparent outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                  />
                </div>
              </div>
            </div>
          );
        })()}
      </div>
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
