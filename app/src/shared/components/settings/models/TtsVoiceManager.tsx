import React, { useState, useMemo, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Sparkles, ArrowLeft, RefreshCw, Loader2 } from "lucide-react";
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

export const TtsVoiceManager: React.FC<TtsVoiceManagerProps> = memo(({
  layoutMode,
  customVoices,
  loadCustomVoices,
  chatterboxIsAdding,
  setChatterboxIsAdding,
  edgeTtsVoices,
  edgeTtsError,
  loadingEdgeVoices,
  loadEdgeVoices,
}) => {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Region bucket filter for Edge TTS (ALL, US, UK, AU, GLOBAL)
  const [selectedRegion, setSelectedRegion] = useState<string>("ALL");

  if (!draftSettings) return null;

  const isEdgeTts = draftSettings.tts.provider?.kind === "edge_tts";
  const isChatterbox =
    draftSettings.tts.provider?.kind === "chatterbox_remote" ||
    (draftSettings.tts.provider?.kind as any) === "chatterbox";

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

  // 2. Simplify Voice Name for Edge Neural Cloud Voices (e.g. "en-US-AriaNeural" -> "Aria (US)")
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

  // Region bucket filtered Edge Neural Voices List for VoiceCarousel
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

  // Local / Chatterbox voices
  const localVoices = isChatterbox
    ? [
        { id: "default", name: "Default" },
        ...customVoices.map((v) => ({ id: v.id, name: simplifyVoiceName(v.name), isCustom: true })),
      ]
    : (modelCatalog?.voices || []).map((v) => ({ id: String(v.id), name: simplifyVoiceName(v.name) }));

  const activeVoices = isEdgeTts ? edgeVoicesList : localVoices;

  const selectedVoiceId = isEdgeTts
    ? (draftSettings.tts.provider as any)?.voice || (edgeVoicesList[0]?.id || "en-US-AriaNeural")
    : isChatterbox
      ? (draftSettings.tts.provider?.kind === "chatterbox"
          ? (draftSettings.tts.provider as any).voice_id || "default"
          : (draftSettings.tts.provider as any)?.voice || "default")
      : String(draftSettings.tts.voice);

  const handleVoiceChange = (id: string) => {
    if (isEdgeTts) {
      updateDraft("tts", "provider", {
        kind: "edge_tts",
        voice: id,
      });
    } else if (isChatterbox) {
      const voiceIdVal = id === "default" ? null : id;
      updateDraft("tts", "provider", {
        ...draftSettings.tts.provider,
        voice_id: voiceIdVal,
        voice: id,
      } as any);
    } else {
      updateDraft("tts", "voice", Number(id));
    }
  };

  return (
    <div
      className={cn(
        "w-full items-stretch",
        layoutMode === "small" ? "flex flex-col gap-3" : "flex flex-row gap-4"
      )}
    >
      {/* Left column: Universal Voice Carousel (60% width) */}
      <div
        className={cn(
          "shrink-0 flex flex-col justify-center",
          layoutMode === "small" ? "w-full" : "w-[60%] min-w-[200px]"
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
        />
      </div>

      {/* Right column: Controls, Region Buckets & Speed Sliders (40% width) */}
      <div className="flex-1 flex flex-col justify-between gap-3 min-w-0">
        <div className="flex flex-col gap-3">
          {isEdgeTts ? (
            <>
              {/* Region Bucket Selector */}
              <div className="space-y-1">
                <div className="flex items-center justify-between">
                  <span className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
                    Region
                  </span>
                  {/* Minimal Live Status Sphere */}
                  {loadingEdgeVoices ? (
                    <Loader2 size={10} className="animate-spin text-yellow-400" />
                  ) : edgeTtsError ? (
                    <button
                      type="button"
                      onClick={loadEdgeVoices}
                      className="text-[10px] font-bold text-rose-400 hover:underline flex items-center gap-1 cursor-pointer"
                    >
                      <RefreshCw size={10} />
                    </button>
                  ) : (
                    <div className="flex items-center gap-1">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.8)] animate-pulse" />
                      <span className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))]/70">
                        {edgeTtsVoices.length > 0 ? `${edgeTtsVoices.length}` : "Live"}
                      </span>
                    </div>
                  )}
                </div>

                {/* Flat Underline / Segment Region Buttons */}
                <div className="flex items-center gap-1 border-b border-[rgba(var(--border),0.1)] pb-1">
                  {(["ALL", "US", "UK", "AU", "GLOBAL"] as const).map((region) => (
                    <button
                      key={region}
                      type="button"
                      onClick={() => setSelectedRegion(region)}
                      className={cn(
                        "flex-1 py-0.5 text-[10px] font-bold uppercase tracking-wider transition-all duration-200 cursor-pointer text-center",
                        selectedRegion === region
                          ? "text-[rgb(var(--accent))] border-b-2 border-[rgb(var(--accent))]"
                          : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      {region}
                    </button>
                  ))}
                </div>
              </div>

              {/* Edge TTS Rotary Speed Knob */}
              <div className="flex-1 flex items-center justify-center pt-1">
                <RotaryKnob
                  label="Speech Speed"
                  value={draftSettings.tts.speed || 1.0}
                  min={0.7}
                  max={2.0}
                  step={0.05}
                  formatValue={(v) => `${v.toFixed(2)}x`}
                  formatPreset={(v) => `${v.toFixed(2)}x`}
                  onChange={(v) => updateDraft("tts", "speed", v)}
                  presetSteps={[0.8, 1.0, 1.25, 1.5, 2.0]}
                />
              </div>
            </>
          ) : isChatterbox ? (
            <>
              {/* Chatterbox Quality Slider */}
              <SliderField
                label="Quality (Steps)"
                value={((draftSettings.tts.provider as any).quality_steps || 8)}
                min={2}
                max={12}
                step={1}
                formatValue={(v) => `${v} steps`}
                onChange={(v) =>
                  updateDraft("tts", "provider", {
                    ...draftSettings.tts.provider,
                    quality_steps: v,
                  } as any)
                }
              />

              {/* Chatterbox Speed Slider */}
              <SliderField
                label="Speed"
                value={((draftSettings.tts.provider as any).speed || 1.0)}
                min={0.7}
                max={2.0}
                step={0.05}
                formatValue={(v) => `${v.toFixed(2)}x`}
                onChange={(v) =>
                  updateDraft("tts", "provider", {
                    ...draftSettings.tts.provider,
                    speed: v,
                  } as any)
                }
              />
            </>
          ) : (
            <>
              {/* Supertonic Quality Steps */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                  <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">
                    {draftSettings.tts.quality_steps <= 4
                      ? "Speed"
                      : draftSettings.tts.quality_steps <= 8
                        ? "Quality"
                        : "Best"}
                  </span>
                </div>
                <div className="flex gap-1">
                  {[2, 4, 6, 8, 10, 12].map((step) => (
                    <button
                      key={step}
                      type="button"
                      onClick={() => updateDraft("tts", "quality_steps", step)}
                      className={cn(
                        "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                        draftSettings.tts.quality_steps === step
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                          : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                      )}
                    >
                      {step}
                    </button>
                  ))}
                </div>
              </div>

              {/* Supertonic Speed */}
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
          )}
        </div>

        {/* Clone Voice button for Chatterbox */}
        {isChatterbox && (
          <button
            type="button"
            onClick={() => setChatterboxIsAdding((prev) => !prev)}
            className={cn(
              "w-full py-2 rounded-xl text-[10px] font-black uppercase tracking-wider transition-all duration-300 flex items-center justify-center gap-1.5 shadow-[0_0_12px_rgba(var(--accent),0.1)]",
              chatterboxIsAdding
                ? "bg-rose-500/10 border border-rose-500/30 text-rose-400 hover:bg-rose-500/20"
                : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-[1.01] active:scale-95 hover:shadow-[0_0_16px_rgba(var(--accent),0.25)]"
            )}
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
        )}
      </div>
    </div>
  );
});

TtsVoiceManager.displayName = "TtsVoiceManager";
