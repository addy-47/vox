import { memo, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { Heart, Loader2, Pause, Play } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import type { UnifiedVoiceDto } from "@/services/voicesService";
import { getVoiceWaveform } from "./VoicePreviewBanner";

/**
 * Bar count that fits the scrubber's measured width. The catalog grid packs
 * cards into 1–2 columns, so a fixed bar count overflows narrow cards; because
 * the list scroller is `overflow-y-auto` (which computes `overflow-x: auto`),
 * that overflow shows up as a horizontal scrollbar on the whole list. Measuring
 * the scrubber keeps the waveform responsive instead.
 */
function useFittingBarCount(active: boolean, max = 32): {
  ref: RefObject<HTMLDivElement | null>;
  count: number;
} {
  const ref = useRef<HTMLDivElement>(null);
  const [count, setCount] = useState(max);
  useEffect(() => {
    if (!active) return;
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const w = el.clientWidth;
      // ~5px per bar (≈3px bar + 2px gap). Clamp so very narrow cards still
      // read as a waveform, and wide cards stay dense.
      setCount(Math.max(8, Math.min(max, Math.floor(w / 5))));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [active, max]);
  return { ref, count };
}

export interface VoiceCardProps {
  voice?: UnifiedVoiceDto;
  selected?: boolean;
  favorited?: boolean;
  previewPlaying?: boolean;
  isSynthesizing?: boolean;
  previewProgress?: number;
  previewDisabled?: boolean;
  hideBadges?: boolean;
  onSelect?: (id: string) => void;
  onToggleFavorite?: (id: string) => void;
  onPreview?: (voice: UnifiedVoiceDto) => void;
  onSeekPreview?: (fraction: number) => void;
}

function tagLabel(tag: string): string {
  const t = tag.trim();
  if (!t) return t;
  return t.charAt(0).toUpperCase() + t.slice(1);
}

/**
 * Voice catalog card. Follows the LLM catalog (RemoteModelCard) grammar:
 * title row with top-right action, subtitle row, bottom meta + primary
 * action row — so both catalogs read as one system.
 */
export const VoiceCard = memo(
  ({
    voice,
    selected = false,
    favorited = false,
    previewPlaying = false,
    isSynthesizing = false,
    previewProgress = 0,
    previewDisabled = false,
    hideBadges = false,
    onSelect,
    onToggleFavorite,
    onPreview,
    onSeekPreview,
  }: VoiceCardProps) => {
    const { ref: scrubberRef, count: barCount } = useFittingBarCount(previewPlaying);
    const cardWaveformBars = useMemo(
      () => (voice && previewPlaying ? getVoiceWaveform(voice.id, barCount) : []),
      [voice, previewPlaying, barCount]
    );

    if (!voice) return null;

    const description = voice.description || VOICE_CATALOG_COPY.fallbackDescription;
    const badges = hideBadges
      ? []
      : Array.from(
          new Set(
            [
              tagLabel(voice.gender),
              voice.accent || voice.locale,
              ...voice.tags.slice(0, 1).map(tagLabel),
            ].filter(Boolean)
          )
        );

    return (
      <div
        role="button"
        tabIndex={0}
        onClick={() => onSelect?.(voice.id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect?.(voice.id);
          }
        }}
        className={cn(
          "group w-full text-left p-4 rounded-xl border transition-all duration-200 relative shrink-0 cursor-pointer flex items-start gap-3.5 hover:z-20 h-full",
          selected
            ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/40"
            : "bg-[rgba(var(--card),0.35)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.55)]"
        )}
      >
        {/* Left Play/Stop Action Sphere */}
        <button
          type="button"
          disabled={previewDisabled || isSynthesizing}
          title={previewDisabled ? VOICE_CATALOG_COPY.previewUnavailableTitle : undefined}
          onClick={(e) => {
            e.stopPropagation();
            onPreview?.(voice);
          }}
          aria-label={previewPlaying ? "Pause preview" : `Preview ${voice.name}`}
          className={cn(
            "w-10 h-10 rounded-full flex items-center justify-center shrink-0 mt-0.5 transition-all shadow-sm",
            previewDisabled
              ? "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/30 cursor-not-allowed"
              : previewPlaying
                ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_0_14px_rgba(var(--accent),0.4)] scale-105"
                : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:scale-105 active:scale-95 cursor-pointer"
          )}
        >
          {isSynthesizing && previewPlaying ? (
            <Loader2 size={16} className="animate-spin" />
          ) : previewPlaying ? (
            <Pause size={16} />
          ) : (
            <Play size={16} className="ml-0.5" />
          )}
        </button>

        {/* Center: Info Column */}
        <div className="flex-1 min-w-0 flex flex-col justify-between h-full">
          <div>
            <div className="flex items-start justify-between gap-2">
              <span
                className={cn(
                  "font-bold text-[14.5px] leading-tight tracking-tight truncate font-sans",
                  selected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                )}
              >
                {voice.name}
              </span>

              {/* Top-Right: Heart Button (accent colored) */}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  e.preventDefault();
                  onToggleFavorite?.(voice.id);
                }}
                aria-label={favorited ? `Remove ${voice.name} from favorites` : `Add ${voice.name} to favorites`}
                className={cn(
                  "p-1 -mr-1 -mt-1 rounded-lg shrink-0 transition-all cursor-pointer relative z-10",
                  favorited
                    ? "text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.04)]"
                )}
              >
                <Heart size={16} fill={favorited ? "currentColor" : "none"} strokeWidth={favorited ? 2 : 1.75} />
              </button>
            </div>

            <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1.5 line-clamp-2 leading-relaxed">
              {description}
            </p>
          </div>

          {/* If previewPlaying, show the interactive waveform scrubber! Otherwise show badges */}
          {previewPlaying ? (
            <div
              ref={scrubberRef}
              role="slider"
              aria-label={VOICE_CATALOG_COPY.seekAria}
              aria-valuenow={Math.round((previewProgress ?? 0) * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              tabIndex={0}
              className="mt-auto pt-2.5 flex items-center justify-between gap-[2px] h-6 w-full overflow-hidden cursor-pointer select-none rounded hover:bg-[rgba(var(--foreground),0.03)] transition-colors"
              onClick={(e) => {
                e.stopPropagation();
                if (!onSeekPreview) return;
                const rect = e.currentTarget.getBoundingClientRect();
                const frac = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
                onSeekPreview(frac);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowRight") {
                  e.stopPropagation();
                  onSeekPreview?.(Math.min(1, (previewProgress ?? 0) + 0.05));
                } else if (e.key === "ArrowLeft") {
                  e.stopPropagation();
                  onSeekPreview?.(Math.max(0, (previewProgress ?? 0) - 0.05));
                }
              }}
              title={VOICE_CATALOG_COPY.seekTitle}
            >
              {cardWaveformBars.map((height, i) => {
                const barProgress = i / cardWaveformBars.length;
                const isPlayed = (previewProgress ?? 0) > 0 && barProgress <= (previewProgress ?? 0);
                return (
                  <div
                    key={i}
                    style={{ height: `${Math.round(Math.max(22, height * 100))}%` }}
                    className={cn(
                      "flex-1 min-w-0 rounded-full max-w-[4px] transition-all duration-75",
                      isSynthesizing
                        ? "animate-pulse bg-[rgb(var(--accent))]/40"
                        : isPlayed
                          ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.45)]"
                          : "bg-[rgba(var(--foreground),0.18)] hover:bg-[rgba(var(--foreground),0.35)]"
                    )}
                  />
                );
              })}
            </div>
          ) : (
            badges.length > 0 && (
              <div className="flex items-center gap-1.5 mt-auto pt-2.5 flex-wrap">
                {badges.map((b) => (
                  <span
                    key={b}
                    className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.07)] leading-none"
                  >
                    {b}
                  </span>
                ))}
              </div>
            )
          )}
        </div>
      </div>
    );
  }
);

VoiceCard.displayName = "VoiceCard";
