import { memo } from "react";
import { Heart, Loader2, Pause, Play } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import type { UnifiedVoiceDto } from "@/services/voiceService";

export interface VoiceCardProps {
  voice?: UnifiedVoiceDto;
  selected?: boolean;
  favorited?: boolean;
  previewPlaying?: boolean;
  isSynthesizing?: boolean;
  previewDisabled?: boolean;
  hideBadges?: boolean;
  onSelect?: (id: string) => void;
  onToggleFavorite?: (id: string) => void;
  onPreview?: (voice: UnifiedVoiceDto) => void;
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
    previewDisabled = false,
    hideBadges = false,
    onSelect,
    onToggleFavorite,
    onPreview,
  }: VoiceCardProps) => {
    if (!voice) return null;

    const description = voice.description || VOICE_CATALOG_COPY.fallbackDescription;
    const badges = hideBadges
      ? []
      : [
          tagLabel(voice.gender),
          voice.accent || voice.locale,
          ...voice.tags.slice(0, 1).map(tagLabel),
        ].filter(Boolean);

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
          "group w-full text-left p-4 rounded-2xl border transition-all duration-200 relative shrink-0 cursor-pointer flex items-start gap-3.5 hover:z-20",
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
        <div className="flex-1 min-w-0">
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

          {/* 3 Voice Badges (modal cards show this, inline catalog cards hide this) */}
          {badges.length > 0 && (
            <div className="flex items-center gap-1.5 mt-2.5 flex-wrap">
              {badges.map((b) => (
                <span
                  key={b}
                  className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-md bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.07)] leading-none"
                >
                  {b}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }
);

VoiceCard.displayName = "VoiceCard";
