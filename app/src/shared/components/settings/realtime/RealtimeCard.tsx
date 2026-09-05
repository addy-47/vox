import { memo } from "react";
import { useSettingsStore, type VoxSettings } from "@/store/settingsStore";
import { Search, Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import {
  resolveRealtimeSubkey,
  isRealtimeProviderDisabled,
} from "@/shared/lib/realtimeProviders";



import {
  PipelineFlow,
  RealtimeInput as Input,
  RealtimeTemperatureSlider as TemperatureSlider,
  RealtimeToggleRow as ToggleRow,
  RealtimeVoiceSelector as VoiceCarousel,
  VOICE_OPTIONS,
} from "./RealtimeVisualElements";


function UnifiedConfig({
  subkey,
  draftSettings,
  updateDraft,
  disabled,
  layoutMode = "full-max",
}: {
  subkey: string;
  draftSettings: VoxSettings;
  updateDraft: (section: any, key: any, value: any) => void;
  disabled: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}) {
  const isDeepgram = subkey === "deepgram_voice_agent" || subkey === "deepgram";
  const isOpenAi = subkey === "openai_realtime" || subkey === "openai";
  const isElevenLabs = subkey === "elevenlabs_convai" || subkey === "elevenlabs";
  const isGemini = !isDeepgram && !isOpenAi && !isElevenLabs;

  const canonicalSubkey = isDeepgram
    ? "deepgram_voice_agent"
    : isOpenAi
    ? "openai_realtime"
    : isElevenLabs
    ? "elevenlabs_convai"
    : "gemini_live";

  const realtime = draftSettings.realtime;
  const config: Record<string, any> =
    (realtime as any)?.[canonicalSubkey] ||
    (realtime as any)?.[subkey] ||
    (isGemini ? realtime?.gemini : isDeepgram ? realtime?.deepgram : {}) ||
    {};

  const voiceField = isGemini ? "voice_name" : "voice";
  const currentVoice = (config[voiceField] as string) || VOICE_OPTIONS[0];

  return (
    <div
      className={cn(
        "w-full items-stretch",
        layoutMode === "small"
          ? "flex flex-col gap-3"
          : "flex flex-row gap-3.5",
        disabled && "opacity-60 pointer-events-none select-none",
      )}
    >
      {/* Left column: Model, Temperature, Toggle (vertical) */}
      <div className="flex-[3] flex flex-col gap-3 min-w-0">
        {/* Model ID — default shows the model name */}
        <Input
          label={REALTIME_CONFIG_DESK_COPY.modelLabel}
          value={config.model || ""}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", canonicalSubkey, { ...config, model: v });
          }}
          placeholder={REALTIME_CONFIG_DESK_COPY.modelPlaceholder}
          disabled={disabled}
        />

        {/* Temperature */}
        <TemperatureSlider
          label={REALTIME_CONFIG_DESK_COPY.temperature}
          value={config.temperature ?? 0.7}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", canonicalSubkey, { ...config, temperature: v });
          }}
          disabled={disabled}
        />

        {/* Toggle (only rendered when provider has supported boolean flags) */}
        {(isGemini || isDeepgram) && (
          <ToggleRow
            label={isGemini ? "Google Search" : "Agent Mode"}
            sub={isGemini ? "Live web grounding" : "AI agent routing"}
            enabled={isGemini ? Boolean(config.enable_web_search) : Boolean(config.agent_mode)}
            onChange={() => {
              if (disabled) return;
              if (isGemini) {
                updateDraft("realtime", "gemini_live", {
                  ...config,
                  enable_web_search: !config.enable_web_search,
                });
              } else if (isDeepgram) {
                updateDraft("realtime", "deepgram_voice_agent", {
                  ...config,
                  agent_mode: !config.agent_mode,
                });
              }
            }}
            icon={
              isGemini ? (
                <Search size={11} className="text-[rgb(var(--accent))]" />
              ) : undefined
            }
            disabled={disabled}
          />
        )}
      </div>

      {/* Right column: Voice carousel */}
      <div
        className={cn(
          "shrink-0",
          layoutMode === "small" ? "w-full" : "w-2/5 min-w-[100px]",
        )}
      >
        <VoiceCarousel
          selected={currentVoice}
          onChange={(v) => {
            if (disabled) return;
            updateDraft("realtime", canonicalSubkey, { ...config, [voiceField]: v });
          }}
          disabled={disabled}
        />
      </div>
    </div>
  );
}


interface RealtimeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const RealtimeCard = memo(
  ({ layoutMode = "full-max" }: RealtimeCardProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    if (!draftSettings) return null;

    const providerId =
      draftSettings.realtime?.active ||
      draftSettings.realtime?.provider ||
      "gemini_live";

    const subkey = resolveRealtimeSubkey(providerId);
    const disabled = isRealtimeProviderDisabled(providerId);

    return (
      <div
        className={cn(
          "w-full h-auto flex flex-col text-[14px] gap-3 leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
          layoutMode === "small"
            ? "bg-transparent p-0"
            : cn(
                "glass-card p-5",
                layoutMode === "full-min"
                  ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]"
                  : "lg:w-[520px]",
              ),
        )}
      >
        {/* ── Header ──────────────────────────────────────────────────────── */}
        {layoutMode !== "small" && (
          <div className="flex items-center justify-between shrink-0">
            <div className="flex items-center gap-2">
              <Cpu className="text-[rgb(var(--accent))]" size={16} />
              <span className="font-display text-[12px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
                {REALTIME_CONFIG_DESK_COPY.hubTitle}
              </span>
            </div>
            <span className="text-[11px] font-bold uppercase text-[rgb(var(--foreground-muted))]/60">
              {REALTIME_CONFIG_DESK_COPY.liveMode}
            </span>
          </div>
        )}

        {/* ── Pipeline Flow (transparent container) ──────────────────────── */}
        <PipelineFlow active={true} />

        {/* ── Config workspace: Unified Glass Desk Container ─────────── */}
        <div className="w-full flex flex-col shrink-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]">
          <UnifiedConfig
            subkey={subkey}
            draftSettings={draftSettings}
            updateDraft={updateDraft}
            disabled={disabled}
            layoutMode={layoutMode}
          />
        </div>
      </div>
    );
  },
);

RealtimeCard.displayName = "RealtimeCard";
