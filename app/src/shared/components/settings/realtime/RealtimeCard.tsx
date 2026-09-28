import { memo } from "react";
import { useSettingsStore, type VoxSettings, type SettingsState, type RealtimeActiveProvider } from "@/store/settingsStore";
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
  subkey: RealtimeActiveProvider;
  draftSettings: VoxSettings;
  updateDraft: SettingsState["updateDraft"];
  disabled: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}) {
  // Config shape — not the provider name — drives every rendering decision.
  // The subkey is only the settings address it was loaded from.
  const config = draftSettings.realtime[subkey];

  const voiceField = "voice_name" in config ? "voice_name" : "voice";
  const currentVoice =
    ("voice_name" in config
      ? config.voice_name
      : "voice" in config
        ? config.voice
        : undefined) || VOICE_OPTIONS[0];
  const model = "model" in config ? config.model : "";
  const temperature = "temperature" in config ? (config.temperature ?? 0.7) : 0.7;

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
          value={model}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", subkey, { ...config, model: v });
          }}
          placeholder={REALTIME_CONFIG_DESK_COPY.modelPlaceholder}
          disabled={disabled}
        />

        {/* Temperature */}
        <TemperatureSlider
          label={REALTIME_CONFIG_DESK_COPY.temperature}
          value={temperature}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", subkey, { ...config, temperature: v });
          }}
          disabled={disabled}
        />

        {/* Toggle (only rendered when the config carries a supported boolean flag) */}
        {"enable_web_search" in config ? (
          <ToggleRow
            label="Google Search"
            sub="Live web grounding"
            enabled={config.enable_web_search}
            onChange={() => {
              if (disabled) return;
              updateDraft("realtime", subkey, {
                ...config,
                enable_web_search: !config.enable_web_search,
              });
            }}
            icon={
              <Search size={11} className="text-[rgb(var(--accent))]" />
            }
            disabled={disabled}
          />
        ) : "agent_mode" in config ? (
          <ToggleRow
            label="Agent Mode"
            sub="AI agent routing"
            enabled={config.agent_mode}
            onChange={() => {
              if (disabled) return;
              updateDraft("realtime", subkey, {
                ...config,
                agent_mode: !config.agent_mode,
              });
            }}
            disabled={disabled}
          />
        ) : null}
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
            updateDraft("realtime", subkey, { ...config, [voiceField]: v });
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

    const providerId = draftSettings.realtime?.active || "gemini_live";

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
              <span className="font-display text-[12px] font-bold uppercase tracking-[0.2em] text/80">
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
