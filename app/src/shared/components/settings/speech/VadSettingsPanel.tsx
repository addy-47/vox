import { memo } from "react";
import { Gauge, Hourglass, Zap } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { VAD_SETTINGS_COPY, CUSTOM_VALUE_COPY } from "@/data/settingsCopy";
import { PresetButton, PresetInput } from "../shared/CustomSvgs";
import { cn } from "@/shared/lib/utils";

export interface VadSettingsPanelProps {
  layoutMode?: "full-max" | "full-min" | "small";
  /**
   * Read-only mode. The realtime pipeline owns the microphone, so the panel
   * still renders the current values but never writes one.
   */
  disabled?: boolean;
}

type VadUserKey = "threshold" | "silence_duration_ms" | "speech_onset_ms";

interface VadKnobRowProps {
  icon: LucideIcon;
  title: string;
  description: string;
  /** The fixed choices, in reading order. */
  presets: Array<{ label: string; value: number }>;
  selectedValue: number;
  onSelect: (value: number) => void;
  /** The custom entry, shown only when the value is not one of the presets. */
  customValue: string;
  onCustomChange: (value: string) => void;
  customSelected: boolean;
  customLabel: string;
  disabled?: boolean;
}

/**
 * One detection knob.
 *
 * The panel lives inside the speech card workspace, next to the model rows.
 * It is a settings row, not a diagram: glyph, name, one line of meaning, and
 * the choices on the right. Three presets share one row; a value outside them
 * switches to a custom field.
 */
const VadKnobRow = memo(({
  icon: Icon,
  title,
  description,
  presets,
  selectedValue,
  onSelect,
  customValue,
  onCustomChange,
  customSelected,
  customLabel,
  disabled = false,
}: VadKnobRowProps) => (
  <div
    className={cn(
      "flex items-center gap-3 rounded-xl border border-[rgba(var(--foreground),0.07)] bg-[rgba(var(--foreground),0.035)] px-3 py-2.5",
      disabled && "opacity-60"
    )}
  >
    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-[rgba(var(--accent),0.22)] bg-[rgba(var(--accent),0.07)] text-[rgb(var(--accent))]">
      <Icon size={15} strokeWidth={1.75} />
    </span>

    <div className="min-w-0 flex-1">
      <span className="block truncate text-[12.5px] font-semibold leading-tight text-[rgb(var(--foreground))]">
        {title}
      </span>
      <span className="block truncate text-[10.5px] leading-tight text-[rgb(var(--foreground-muted))]/80">
        {description}
      </span>
    </div>

    <div className="flex shrink-0 items-center gap-1.5">
      <div className="grid flex-1 grid-cols-3 gap-1">
        {presets.map((preset) => (
          <PresetButton
            key={preset.value}
            selected={selectedValue === preset.value}
            onClick={() => onSelect(preset.value)}
          >
            {preset.label}
          </PresetButton>
        ))}
      </div>
      <div className="w-[76px] shrink-0">
        <PresetInput
          selected={customSelected}
          value={customValue}
          placeholder={CUSTOM_VALUE_COPY.placeholder}
          ariaLabel={customLabel}
          onChange={(e) => onCustomChange(e.target.value)}
        />
      </div>
    </div>
  </div>
));

VadKnobRow.displayName = "VadKnobRow";

/**
 * The three user-facing VAD knobs, stacked in one scrollable pane: no tabs, so
 * every control is one glance away behind the voice-detection gear.
 */
export const VadSettingsPanel = memo(
  ({ disabled = false }: VadSettingsPanelProps) => {
    const vad = useSettingsStore((s) => s.draftSettings?.vad);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    if (!vad) return null;

    const currentThreshold = vad.threshold ?? 0.5;
    const currentSilenceMs = vad.silence_duration_ms ?? 800;
    const currentSpeechOnsetMs = vad.speech_onset_ms ?? 32;

    const thresholdCustom = ![0.3, 0.5, 0.7].some((v) => Math.abs(currentThreshold - v) < 0.04);
    const silenceCustom = ![400, 800, 1200].includes(currentSilenceMs);
    const onsetCustom = ![32, 64, 128].includes(currentSpeechOnsetMs);

    const write = (key: VadUserKey, value: number) => {
      if (disabled) return;
      updateDraft("vad", key, value);
    };

    return (
      <div className="flex-1 min-h-0 w-full overflow-y-auto overscroll-contain custom-scrollbar pr-1">
        <div className="flex flex-col gap-2 pb-6">
          <VadKnobRow
            icon={Gauge}
            title={VAD_SETTINGS_COPY.sensitivity.title}
            description={VAD_SETTINGS_COPY.sensitivity.description}
            presets={[
              { label: "30%", value: 0.3 },
              { label: "50%", value: 0.5 },
              { label: "70%", value: 0.7 },
            ]}
            selectedValue={
              thresholdCustom ? Number.NaN : [0.3, 0.5, 0.7].find((v) => Math.abs(currentThreshold - v) < 0.04)!
            }
            onSelect={(value) => write("threshold", value)}
            customSelected={thresholdCustom}
            customValue={thresholdCustom ? `${Math.round(currentThreshold * 100)}%` : ""}
            customLabel={VAD_SETTINGS_COPY.sensitivity.title}
            onCustomChange={(raw) => {
              const clean = raw.replace(/[^0-9]/g, "");
              if (!clean) return;
              const num = parseInt(clean, 10);
              if (!Number.isNaN(num) && num >= 5 && num <= 95) {
                write("threshold", num / 100);
              }
            }}
            disabled={disabled}
          />

          <VadKnobRow
            icon={Hourglass}
            title={VAD_SETTINGS_COPY.silence.title}
            description={VAD_SETTINGS_COPY.silence.description}
            presets={[
              { label: "400ms", value: 400 },
              { label: "800ms", value: 800 },
              { label: "1200ms", value: 1200 },
            ]}
            selectedValue={silenceCustom ? Number.NaN : currentSilenceMs}
            onSelect={(value) => write("silence_duration_ms", value)}
            customSelected={silenceCustom}
            customValue={silenceCustom ? `${currentSilenceMs}ms` : ""}
            customLabel={VAD_SETTINGS_COPY.silence.title}
            onCustomChange={(raw) => {
              const clean = raw.replace(/[^0-9]/g, "");
              if (!clean) return;
              const num = parseInt(clean, 10);
              if (!Number.isNaN(num) && num >= 100 && num <= 3000) {
                write("silence_duration_ms", num);
              }
            }}
            disabled={disabled}
          />

          <VadKnobRow
            icon={Zap}
            title={VAD_SETTINGS_COPY.speechOnset.title}
            description={VAD_SETTINGS_COPY.speechOnset.description}
            presets={[
              { label: "32ms", value: 32 },
              { label: "64ms", value: 64 },
              { label: "128ms", value: 128 },
            ]}
            selectedValue={onsetCustom ? Number.NaN : currentSpeechOnsetMs}
            onSelect={(value) => write("speech_onset_ms", value)}
            customSelected={onsetCustom}
            customValue={onsetCustom ? `${currentSpeechOnsetMs}ms` : ""}
            customLabel={VAD_SETTINGS_COPY.speechOnset.title}
            onCustomChange={(raw) => {
              const clean = raw.replace(/[^0-9]/g, "");
              if (!clean) return;
              const num = parseInt(clean, 10);
              if (!Number.isNaN(num) && num >= 16 && num <= 1000) {
                write("speech_onset_ms", num);
              }
            }}
            disabled={disabled}
          />
        </div>
      </div>
    );
  }
);

VadSettingsPanel.displayName = "VadSettingsPanel";
