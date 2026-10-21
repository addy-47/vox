import { memo, useCallback, useEffect, useRef, useState } from "react";
import { Gauge } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { RotaryKnob } from "@/shared/ui";
import { useSettingsStore, type ProviderCaps } from "@/store/settingsStore";
import { getProviderCaps } from "@/services/catalogService";
import { useModelParameters } from "@/shared/hooks/useModelParameters";
import { TTS_VOICE_MANAGER_COPY } from "@/data/settingsCopy";
import { CardPopover } from "../shared/CardHeader";

/**
 * Speech rate for the active TTS engine. A single header glyph opens a
 * popover with the rate knob, so the voice catalog header keeps its
 * `[action] [filter] [sort] [search]` shape.
 *
 * The rate is a per-model parameter (Turso `models.parameters.speed`), not a
 * settings key, and its bounds come from the provider.
 */
export const SpeechRateControl = memo(() => {
  const ttsModelId = useSettingsStore((s) => s.draftSettings?.tts.active.model ?? null);
  const { params, setParam } = useModelParameters(ttsModelId);

  const [open, setOpen] = useState(false);
  const [caps, setCaps] = useState<ProviderCaps | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);

  // Bounds come from the provider, not the UI.
  useEffect(() => {
    if (!ttsModelId) {
      setCaps(null);
      return;
    }
    let isMounted = true;
    getProviderCaps(ttsModelId)
      .then((next) => {
        if (isMounted) setCaps(next);
      })
      .catch(() => {
        if (isMounted) setCaps(null);
      });
    return () => {
      isMounted = false;
    };
  }, [ttsModelId]);

  const handleChange = useCallback(
    (value: number) => {
      if (!ttsModelId) return;
      setParam("speed", value);
    },
    [setParam, ttsModelId]
  );

  const speedValue =
    typeof params.speed === "number" && !Number.isNaN(params.speed) ? params.speed : 1.0;
  const range = caps?.speed_range ?? { min: 0.7, max: 2.0, step: 0.05 };
  const copy = TTS_VOICE_MANAGER_COPY;
  const executionLabel = caps?.execution ? copy.execution[caps.execution] : null;

  return (
    <div className="relative shrink-0">
      <Tooltip label={copy.speed.title} side="bottom" asChild>
        <button
          ref={triggerRef}
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-label={copy.speed.title}
          aria-expanded={open}
          disabled={!ttsModelId}
          className={cn(
            "w-7 h-7 flex items-center justify-center rounded-md transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] disabled:opacity-40 disabled:cursor-not-allowed",
            open
              ? "text-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))]"
          )}
        >
          <Gauge size={13} strokeWidth={1.75} />
        </button>
      </Tooltip>

      {open && (
        <CardPopover
          anchorRef={triggerRef}
          panelRef={panelRef}
          onClose={() => setOpen(false)}
          menu={
            <div className="flex flex-col items-center gap-1 px-1 pb-1">
              <span className="w-full text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/80">
                {copy.speed.title}
              </span>
              <RotaryKnob
                value={speedValue}
                min={range.min}
                max={range.max}
                step={range.step}
                formatValue={(v) => `${v.toFixed(2)}x`}
                formatPreset={(v) => `${v}x`}
                onChange={handleChange}
                presetSteps={[0.8, 1.0, 1.25]}
              />
              {/* Where the audio is produced. The backend reports it per
                  provider, and it is privacy-relevant: cloud audio leaves the
                  machine. */}
              {executionLabel && (
                <span className="text-[10.5px] text-center leading-snug text-[rgb(var(--foreground-muted))]/80">
                  {executionLabel}
                </span>
              )}
            </div>
          }
        />
      )}
    </div>
  );
});

SpeechRateControl.displayName = "SpeechRateControl";
