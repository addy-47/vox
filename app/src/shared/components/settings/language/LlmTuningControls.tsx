import { memo, useMemo, useRef, useState, type ReactNode } from "react";
import { Layers, Thermometer, TextCursorInput, type LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { RotaryKnob } from "@/shared/ui";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { resolveCapabilitiesById } from "@/shared/lib/catalogCapabilities";
import { CardPopover } from "../shared/CardHeader";
import { PresetButton, PresetInput } from "../shared/CustomSvgs";
import { LLM_TUNING_COPY } from "@/data/settingsCopy";

/** The context floor. No model accepts less. */
const MIN_CONTEXT_WINDOW = 8192;

/** Response length options, in reading order. `0` means the model decides. */
const RESPONSE_LENGTH_PRESETS = [
  { label: "300", value: 300 },
  { label: "1000", value: 1000 },
  { label: "2000", value: 2000 },
  { label: "Native", value: 0 },
] as const;

/**
 * One header glyph and the panel it opens.
 *
 * The panel is a popover, not a card face: the catalog keeps its list, and the
 * knob stays one click away. The trigger is the only hit area; the panel
 * closes on Escape or an outside press.
 */
const TuningGlyph = memo(({
  icon: Icon,
  label,
  valueLabel,
  disabled = false,
  children,
}: {
  icon: LucideIcon;
  label: string;
  /** Short value shown next to the glyph, e.g. "16K". */
  valueLabel?: string;
  disabled?: boolean;
  children: ReactNode;
}) => {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);

  return (
    <div className="relative shrink-0">
      <Tooltip label={valueLabel ? `${label} · ${valueLabel}` : label} side="bottom" asChild>
        <button
          ref={triggerRef}
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-label={label}
          aria-expanded={open}
          disabled={disabled}
          className={cn(
            "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer",
            "focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
            "disabled:opacity-40 disabled:cursor-not-allowed",
            open
              ? "text-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]"
          )}
        >
          <Icon size={13} strokeWidth={1.75} />
        </button>
      </Tooltip>

      {open && (
        <CardPopover
          anchorRef={triggerRef}
          panelRef={panelRef}
          menu={children}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
});

TuningGlyph.displayName = "TuningGlyph";

/** Panel chrome: a title, the controls, and one line of meaning. */
const TuningPanel = memo(({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children: ReactNode;
}) => (
  <div className="flex w-full flex-col gap-2 px-1 pb-1">
    <span className="w-full text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80">
      {title}
    </span>
    {children}
    <span className="text-[10.5px] leading-snug text-[rgb(var(--foreground-muted))]/70">{hint}</span>
  </div>
));

TuningPanel.displayName = "TuningPanel";

/**
 * The three LLM knobs, as three header glyphs.
 *
 * They are per-model parameters (Turso `models.parameters`), not settings
 * keys, so they follow the active model and never touch the settings draft.
 * The context window is the exception: a remote provider owns its own window,
 * so that glyph reports "Server managed" until a probed capability states the
 * ceiling.
 */
export const LlmTuningControls = memo(() => {
  const llm = useSettingsStore((s) => s.draftSettings?.llm);
  const activeModelId = llm?.active.model ?? null;
  const { params, setParam } = useModelParameters(activeModelId);
  const capabilitiesCache = useCatalogStore((s) => s.capabilitiesCache);

  const num = (value: unknown): number | undefined =>
    typeof value === "number" && !Number.isNaN(value) ? value : undefined;

  const tokens = num(params.max_output_tokens) ?? 300;
  const temperature = num(params.temperature) ?? 0.7;
  const context = Math.max(MIN_CONTEXT_WINDOW, num(params.context_window) ?? MIN_CONTEXT_WINDOW);

  /** The probed ceiling. Unknown means the provider decides. */
  const contextCeiling = useMemo(() => {
    // A model that was never probed has no cache entry and no ceiling. This
    // lookup goes through the SSOT so the crash on a cache miss cannot return.
    return resolveCapabilitiesById(activeModelId, capabilitiesCache)?.context_window;
  }, [activeModelId, capabilitiesCache]);

  const contextPresets = useMemo(
    () =>
      [8192, 16384, 32768, 65536].filter(
        (size) => size <= (contextCeiling ?? MIN_CONTEXT_WINDOW)
      ),
    [contextCeiling]
  );

  const contextManaged = contextCeiling === undefined;
  const copy = LLM_TUNING_COPY;

  const formatTokens = (value: number) => (value === 0 ? "Native" : String(value));
  const formatContext = (value: number) => `${Math.round(value / 1024)}K`;

  return (
    <div className="flex shrink-0 items-center gap-0.5">
      {/* Response length: discrete sizes plus a custom token count. */}
      <TuningGlyph
        icon={TextCursorInput}
        label={copy.responseLength.title}
        valueLabel={formatTokens(tokens)}
        disabled={!activeModelId}
      >
        <TuningPanel title={copy.responseLength.title} hint={copy.responseLength.hint}>
          <div className="grid grid-cols-2 gap-1">
            {RESPONSE_LENGTH_PRESETS.map((preset) => (
              <PresetButton
                key={preset.label}
                selected={tokens === preset.value}
                onClick={() => setParam("max_output_tokens", preset.value)}
              >
                {preset.label}
              </PresetButton>
            ))}
            <div className="col-span-2">
              <PresetInput
                selected={!RESPONSE_LENGTH_PRESETS.some((preset) => preset.value === tokens)}
                value={RESPONSE_LENGTH_PRESETS.some((preset) => preset.value === tokens) ? "" : String(tokens)}
                placeholder={copy.custom}
                ariaLabel={copy.responseLength.title}
                onChange={(e) => {
                  const clean = e.target.value.replace(/[^0-9]/g, "");
                  if (!clean) return;
                  const parsed = parseInt(clean, 10);
                  if (!Number.isNaN(parsed) && parsed >= 50 && parsed <= 128000) {
                    setParam("max_output_tokens", parsed);
                  }
                }}
              />
            </div>
          </div>
        </TuningPanel>
      </TuningGlyph>

      {/* Context window: the provider owns it unless a probe states a ceiling. */}
      <TuningGlyph
        icon={Layers}
        label={copy.context.title}
        valueLabel={contextManaged ? copy.context.serverManaged : formatContext(context)}
        disabled={!activeModelId || contextManaged}
      >
        <TuningPanel title={copy.context.title} hint={copy.context.hint}>
          {contextManaged ? (
            <span className="px-1 py-1.5 text-[11px] font-bold text-[rgb(var(--foreground-muted))]/80">
              {copy.context.serverManaged}
            </span>
          ) : (
            <>
              <div className="grid grid-cols-2 gap-1">
                {contextPresets.map((preset) => (
                  <PresetButton
                    key={preset}
                    selected={context === preset}
                    onClick={() => setParam("context_window", preset)}
                  >
                    {formatContext(preset)}
                  </PresetButton>
                ))}
                <div className="col-span-2">
                  <PresetInput
                    selected={!contextPresets.includes(context)}
                    value={contextPresets.includes(context) ? "" : String(context)}
                    placeholder={copy.custom}
                    ariaLabel={copy.context.title}
                    onChange={(e) => {
                      const clean = e.target.value.replace(/[^0-9]/g, "");
                      if (!clean) return;
                      const parsed = parseInt(clean, 10);
                      if (!Number.isNaN(parsed) && parsed >= MIN_CONTEXT_WINDOW && parsed <= 1_000_000) {
                        setParam("context_window", parsed);
                      }
                    }}
                  />
                </div>
              </div>
            </>
          )}
        </TuningPanel>
      </TuningGlyph>

      {/* Creativity: a continuous dial, so it gets a knob. */}
      <TuningGlyph
        icon={Thermometer}
        label={copy.creativity.title}
        valueLabel={temperature.toFixed(2)}
        disabled={!activeModelId}
      >
        <TuningPanel title={copy.creativity.title} hint={copy.creativity.hint}>
          <div className="flex justify-center py-1">
            <RotaryKnob
              value={temperature}
              min={0}
              max={1.5}
              step={0.05}
              defaultValue={0.7}
              formatValue={(v) => v.toFixed(2)}
              formatPreset={(v) => v.toFixed(1)}
              onChange={(value) => setParam("temperature", value)}
              presetSteps={[0.2, 0.7, 1.0]}
            />
          </div>
        </TuningPanel>
      </TuningGlyph>
    </div>
  );
});

LlmTuningControls.displayName = "LlmTuningControls";
