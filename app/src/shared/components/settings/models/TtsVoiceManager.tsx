import React, { useState, useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Sparkles, ArrowLeft } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SliderField, RotaryKnob } from "@/shared/ui";
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
        "w-full items-stretch",
        layoutMode === "small" ? "flex flex-col gap-3" : "flex flex-row gap-4"
      )}
    >
      {/* ─── Left Column: Voice Carousel & Region Filter ─── */}
      <div
        className={cn(
          "shrink-0 flex flex-col justify-center",
          layoutMode === "small" ? "w-full" : "w-[64%] min-w-[220px]"
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
          showRegions={isEdgeTts}
          selectedRegion={selectedRegion}
          onSelectRegion={setSelectedRegion}
          regions={["ALL", "US", "UK", "AU", "GLOBAL"]}
        />
      </div>

      {/* ─── Right Column: Speed Rotary Knob with Presets ─── */}
      <div className="flex-1 flex flex-col justify-center gap-3 min-w-0">
        <div className="flex flex-col gap-3">
          {isEdgeTts ? (
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
          ) : isChatterbox ? (
            <>
              <SliderField
                label="Quality (Steps)"
                value={draftSettings.tts.quality_steps || 8}
                min={2}
                max={12}
                step={1}
                formatValue={(v) => `${v} steps`}
                onChange={(v) => updateDraft("tts", "quality_steps", v)}
              />

              <SliderField
                label="Speed"
                value={draftSettings.tts.speed || 1.0}
                min={0.7}
                max={2.0}
                step={0.05}
                formatValue={(v) => `${v.toFixed(2)}x`}
                onChange={(v) => updateDraft("tts", "speed", v)}
              />
            </>
          ) : (
            <>
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[13px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                  <span className="text-[13px] text-[rgb(var(--accent))] font-bold">
                    {draftSettings.tts.quality_steps <= 4
                      ? "Fast (4 Steps)"
                      : draftSettings.tts.quality_steps <= 8
                      ? "Standard (8 Steps)"
                      : "High (16 Steps)"}
                  </span>
                </div>
                <div className="grid grid-cols-3 gap-2">
                  {[
                    { label: "Fast", steps: 4, desc: "Ultra-low latency" },
                    { label: "Standard", steps: 8, desc: "Balanced natural" },
                    { label: "High", steps: 16, desc: "Maximum quality" },
                  ].map((preset) => {
                    const isSelected = draftSettings.tts.quality_steps === preset.steps;
                    return (
                      <button
                        key={preset.label}
                        type="button"
                        onClick={() => updateDraft("tts", "quality_steps", preset.steps)}
                        className={cn(
                          "p-2 rounded-xl border text-center transition-all cursor-pointer",
                          isSelected
                            ? "bg-[rgba(var(--accent),0.08)] border-[rgb(var(--accent))] text-[rgb(var(--accent))] ring-1 ring-[rgb(var(--accent))]/30"
                            : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:border-[rgba(var(--accent),0.3)] hover:text-[rgb(var(--foreground))]"
                        )}
                      >
                        <div className="font-bold text-[12px]">{preset.label}</div>
                        <div className="text-[10px] opacity-70 mt-0.5">{preset.desc}</div>
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="flex-1 flex items-center justify-center pt-1">
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
            </>
          )}
        </div>

        {isChatterbox && (
          <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex items-center justify-between">
            <button
              type="button"
              onClick={() => setChatterboxIsAdding((prev) => !prev)}
              className="text-[11px] font-bold text-[rgb(var(--accent))] hover:underline transition-colors cursor-pointer flex items-center gap-1"
            >
              {chatterboxIsAdding ? (
                <>
                  <ArrowLeft size={11} />
                  Back to Presets
                </>
              ) : (
                <>
                  <Sparkles size={11} />
                  Clone Voice Profile
                </>
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
