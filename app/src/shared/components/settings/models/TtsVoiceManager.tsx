import React, { useState, useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { RotaryKnob } from "@/shared/ui";
import { VoiceCarousel } from "../voice/VoiceCarousel";

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

export const TtsVoiceManager = memo(({
  layoutMode,
  customVoices,
  loadCustomVoices,
  chatterboxIsAdding,
  setChatterboxIsAdding,
  edgeTtsVoices,
}: TtsVoiceManagerProps) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Region bucket filter for Edge TTS (ALL, US, UK, AU, GLOBAL)
  const [selectedRegion, setSelectedRegion] = useState<string>("ALL");

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

  return (
    <div
      className={cn(
        "w-full items-center",
        layoutMode === "small" ? "flex flex-col gap-3" : "flex flex-row gap-4 h-[145px]"
      )}
    >
      {/* ─── Left Column: Voice Carousel & Region Filter (55%) ─── */}
      <div
        className={cn(
          "shrink-0 flex flex-col justify-center h-full",
          layoutMode === "small" ? "w-full" : "w-[55%] min-w-[220px]"
        )}
      >
        <VoiceCarousel
          voices={activeVoices as any}
          selected={selectedVoiceId as any}
          onChange={handleVoiceChange as any}
          disabled={false}
          onVoicesChanged={loadCustomVoices}
          isAdding={chatterboxIsAdding}
          setIsAdding={setChatterboxIsAdding}
          allowClone={isChatterbox}
          showRegions={isEdgeTts}
          selectedRegion={selectedRegion}
          onSelectRegion={setSelectedRegion}
          regions={["ALL", "US", "UK", "AU", "GLOBAL"]}
        />
      </div>

      {/* ─── Right Column: Speed Rotary Knob (45%) ─── */}
      <div className="flex-1 flex flex-col justify-center gap-3 min-w-0">
        {/* Rotary Speed Knob */}
        <div className="flex-1 flex items-center justify-center py-1">
          <RotaryKnob
            label="Speech Speed"
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
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
