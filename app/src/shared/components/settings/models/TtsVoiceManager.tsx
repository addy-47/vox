import React, { useState, useMemo, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { RotaryKnob } from "@/shared/ui";
import { VoiceCarousel } from "../voice/VoiceCarousel";
import { TTS_VOICE_MANAGER_COPY } from "@/data/settingsCopy";

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
  customVoices: CustomVoice[];
  loadCustomVoices: () => void;
  chatterboxIsAdding: boolean;
  setChatterboxIsAdding: React.Dispatch<React.SetStateAction<boolean>>;
  edgeTtsVoices: EdgeTtsVoice[];
  edgeTtsError: string | null;
  loadingEdgeVoices: boolean;
  loadEdgeVoices: () => void;
  activeCategoryTab?: "model" | "settings";
}

type TtsSubTab = "voice" | "speed";

const REGIONS = ["ALL", "US", "UK", "AU", "GLOBAL"] as const;

export const TtsVoiceManager = memo(({
  layoutMode,
  customVoices,
  loadCustomVoices,
  chatterboxIsAdding,
  setChatterboxIsAdding,
  edgeTtsVoices,
}: TtsVoiceManagerProps) => {
  const [activeSubTab, setActiveSubTab] = useState<TtsSubTab>("voice");
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

  if (!draftSettings) return null;

  const isEdgeTts = draftSettings.tts.active === "edge_tts";
  const isChatterbox =
    draftSettings.tts.active === "chatterbox_remote" ||
    draftSettings.tts.active === "chatterbox";

  // 1. Simplify Voice Name for Local Presets
  const simplifyVoiceName = (n: string) => {
    if (n.includes("Pain")) return "Pain";
    if (n.includes("Madara")) return "Madara";
    if (n.includes("Shreya")) return "Shreya";
    if (n.includes("Hayami")) return "Hayami";
    if (n.includes("Ellen")) return "Ellen";
    if (n.includes("Juniper")) return "Juniper";
    if (n.includes("Mark")) return "Mark";
    if (n.includes("Spuds")) return "Spuds";
    return n;
  };

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

  const localVoices = isChatterbox
    ? [
        { id: "default", name: "Default" },
        ...customVoices.map((v) => ({ id: v.id, name: simplifyVoiceName(v.name), isCustom: true })),
      ]
    : (modelCatalog?.voices || []).map((v) => ({ id: String(v.id), name: simplifyVoiceName(v.name) }));

  const activeVoices = isEdgeTts ? edgeVoicesList : localVoices;

  const selectedVoiceId = isEdgeTts
    ? draftSettings.tts.edge_tts?.voice || (edgeVoicesList[0]?.id || "en-US-AriaNeural")
    : isChatterbox
      ? draftSettings.tts.chatterbox?.language || "default"
      : String(draftSettings.tts.voice_index ?? 0);

  const handleVoiceChange = (id: string) => {
    if (isEdgeTts) {
      updateDraft("tts", "edge_tts", {
        ...draftSettings.tts.edge_tts,
        voice: id,
      });
    } else if (isChatterbox) {
      updateDraft("tts", "chatterbox", {
        ...draftSettings.tts.chatterbox,
        language: id,
      });
    } else {
      updateDraft("tts", "voice_index", Number(id));
    }
  };

  const copy = TTS_VOICE_MANAGER_COPY;
  const isSmall = layoutMode === "small";

  const tabs: Array<{ id: TtsSubTab; label: string }> = [
    { id: "voice", label: copy.tabs.selectVoice },
    { id: "speed", label: copy.tabs.speechSpeed },
  ];

  return (
    <div className="w-full flex-1 flex flex-col justify-between select-none animate-fade-in">
      {/* ─── Layer 1: Subtab Navigation (Full-Width Distributed Tabs) ─── */}
      <div className="w-full flex items-center justify-between pt-0.5 pb-2 shrink-0 border-b border-[rgba(var(--accent),0.08)] mb-2 px-0.5">
        {tabs.map((tab, idx, arr) => {
          const isActive = activeSubTab === tab.id;
          return (
            <div key={tab.id} className="flex-1 flex items-center justify-center">
              <button
                type="button"
                onClick={() => setActiveSubTab(tab.id)}
                className={cn(
                  "w-full flex items-center justify-center pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer text-center",
                  isActive
                    ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/60 border-transparent hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span>{tab.label}</span>
              </button>
              {idx < arr.length - 1 && (
                <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/25 font-light select-none pb-1 shrink-0 px-1 sm:px-2">
                  |
                </span>
              )}
            </div>
          );
        })}
      </div>

      {/* ─── Layer 2: Subtab Workspace (HistoryCard Side-by-Side Ergonomics) ─── */}
      <div
        className={cn(
          "w-full flex flex-col flex-1 min-h-0 pt-0.5 pb-0.5 justify-between",
          isSmall ? "h-auto py-1" : "h-[128px] max-h-[128px]"
        )}
      >
        {/* TAB 1: SELECT VOICE */}
        {activeSubTab === "voice" && (
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
                      aria-label="Previous region"
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
                      aria-label="Next region"
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
                voices={activeVoices as any}
                selected={selectedVoiceId as any}
                onChange={handleVoiceChange as any}
                disabled={false}
                onVoicesChanged={loadCustomVoices}
                isAdding={chatterboxIsAdding}
                setIsAdding={setChatterboxIsAdding}
                allowClone={isChatterbox}
              />
            </div>
          </div>
        )}

        {/* TAB 2: SPEECH SPEED */}
        {activeSubTab === "speed" && (
          <div className="flex flex-row items-center justify-between gap-3 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
            <div className="flex flex-col gap-1 min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
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
      </div>
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
