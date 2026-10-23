import { memo } from "react";
import { useSettingsStore, type VoxSettings, type SettingsState, type RealtimeSubkey } from "@/store/settingsStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { Search, Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";import {
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
  subkey: RealtimeSubkey;
  draftSettings: VoxSettings;
  updateDraft: SettingsState["updateDraft"];
  disabled: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}) {
  // Config shape — not the provider name — drives every rendering decision.
  // The subkey only selects the provider family's field layout. Values live
  // in the `models` table per active model; the model id itself is the
  // realtime active selection.
  const activeModel = draftSettings.realtime.active.model || null;
  const { params, setParam } = useModelParameters(activeModel);

  const contract = useSettingsStore((s) => s.contract);
  const realtimeProvider = contract?.realtime_providers?.find((p) => p.id === subkey);
  const voiceField = realtimeProvider?.voice_field ?? "voice";
  const supportsWebSearch = realtimeProvider?.supports_web_search ?? false;
  const supportsAgentMode = realtimeProvider?.supports_agent_mode ?? false;
  const strParam = (v: unknown): string | undefined =>
    typeof v === "string" && v.length > 0 ? v : undefined;
  const currentVoice = strParam(params[voiceField]) || VOICE_OPTIONS[0];
  const model = activeModel ?? "";
  const rawTemp = params.temperature;
  const temperature = typeof rawTemp === "number" && !Number.isNaN(rawTemp) ? rawTemp : 0.7;
  const webSearch = params.enable_web_search !== false;
  const agentMode = params.agent_mode === true;

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
            if (disabled) return;
            const cur = draftSettings.realtime.active;
            if (cur) updateDraft("realtime", "active", { ...cur, model: v });
          }}
          placeholder={REALTIME_CONFIG_DESK_COPY.modelPlaceholder}
          disabled={disabled}
        />

        {/* Temperature */}
        <TemperatureSlider
          label={REALTIME_CONFIG_DESK_COPY.temperature}
          value={temperature}
          onChange={(v) => {
            if (!disabled) setParam("temperature", v);
          }}
          disabled={disabled}
        />

        {/* Toggle (only rendered when the provider family supports the flag) */}
        {supportsWebSearch ? (
          <ToggleRow
            label="Google Search"
            sub="Live web grounding"
            enabled={webSearch}
            onChange={() => {
              if (disabled) return;
              setParam("enable_web_search", !webSearch);
            }}
            icon={
              <Search size={11} className="text-[rgb(var(--accent))]" />
            }
            disabled={disabled}
          />
        ) : null}
        {supportsAgentMode ? (
          <ToggleRow
            label="Agent Mode"
            sub="AI agent routing"
            enabled={agentMode}
            onChange={() => {
              if (disabled) return;
              setParam("agent_mode", !agentMode);
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
            setParam(voiceField, v);
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
    const contract = useSettingsStore((s) => s.contract);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    if (!draftSettings) return null;

    const fallbackProvider = contract?.realtime_providers?.[0]?.id ?? "";
    const providerId = draftSettings.realtime.active.provider ?? fallbackProvider;

    // Realtime subkey is the provider id itself (backend-defined); no provider
    // is disabled — the former realtimeProviders shim was a vestigial no-op.
    const subkey = providerId;
    const disabled = false;

    return (
      <div
        className={cn(
          "w-full h-auto flex flex-col text-[14px] gap-3 leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
          layoutMode === "small"
            ? "bg-transparent p-0"
            : "glass-card p-5",
        )}
      >
        {/* ── Header ──────────────────────────────────────────────────────── */}
        {layoutMode !== "small" && (
          <div className="flex items-center justify-between shrink-0">
            <div className="flex items-center gap-2">
              <Cpu className="text-[rgb(var(--accent))]" size={16} />
              <span className="font-display text-[12px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground))]/80">
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
