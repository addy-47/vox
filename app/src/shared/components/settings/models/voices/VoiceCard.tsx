import { memo } from "react";
import { Check, Heart, Loader2, Pause, Play, Plus } from "lucide-react";
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
  onSelect?: (id: string) => void;
  onToggleFavorite?: (id: string) => void;
  onPreview?: (voice: UnifiedVoiceDto) => void;
  isCloneSkeleton?: boolean;
  onCloneClick?: () => void;
}

function tagLabel(tag: string): string {
  const t = tag.trim();
  if (!t) return t;
  return t.charAt(0).toUpperCase() + t.slice(1);
}

export const VoiceCard = memo(
  ({
    voice,
    selected = false,
    favorited = false,
    previewPlaying = false,
    isSynthesizing = false,
    previewDisabled = false,
    onSelect,
    onToggleFavorite,
    onPreview,
    isCloneSkeleton = false,
    onCloneClick,
  }: VoiceCardProps) => {
    if (isCloneSkeleton) {
      return (
        <button
          type="button"
          onClick={onCloneClick}
          className="group w-full text-left p-3.5 rounded-xl border-2 border-dashed border-[rgba(var(--foreground),0.12)] hover:border-[rgb(var(--accent))]/60 hover:bg-[rgba(var(--accent),0.03)] transition-all duration-200 cursor-pointer min-h-[96px] flex items-center justify-center gap-3"
        >
          <div className="w-9 h-9 rounded-full bg-[rgba(var(--foreground),0.04)] group-hover:bg-[rgb(var(--accent))]/15 group-hover:text-[rgb(var(--accent))] text-[rgb(var(--foreground-muted))] flex items-center justify-center transition-all shrink-0">
            <Plus size={18} />
          </div>
          <div className="flex flex-col min-w-0">
            <span className="font-bold text-[14px] text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors">
              Clone New Voice
            </span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
              Record sample or upload WAV file
            </span>
          </div>
        </button>
      );
    }

    if (!voice) return null;

    const description = voice.description || VOICE_CATALOG_COPY.fallbackDescription;
    const tags = [
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
          "group w-full text-left p-3.5 rounded-xl border transition-all duration-200 relative cursor-pointer min-h-[96px] flex gap-3",
          selected
            ? "bg-[rgba(var(--card),0.6)] border-[rgb(var(--accent))] shadow-[0_0_16px_rgba(var(--accent),0.16)] ring-1 ring-[rgb(var(--accent))]/40"
            : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.03)]"
        )}
      >
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
            "w-9 h-9 rounded-full flex items-center justify-center shrink-0 self-start transition-all mt-0.5",
            previewDisabled
              ? "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
              : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))]/20 hover:text-[rgb(var(--accent))] cursor-pointer"
          )}
        >
          {isSynthesizing && previewPlaying ? (
            <Loader2 size={15} className="animate-spin text-[rgb(var(--accent))]" />
          ) : previewPlaying ? (
            <Pause size={15} />
          ) : (
            <Play size={15} className="ml-0.5" />
          )}
        </button>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span
              className={cn(
                "font-bold text-[14px] leading-tight truncate",
                selected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
              )}
            >
              {voice.name}
            </span>
            {selected && (
              <span className="w-4 h-4 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] flex items-center justify-center shrink-0">
                <Check size={10} strokeWidth={3} />
              </span>
            )}
          </div>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]/80 leading-snug mt-1 line-clamp-2">
            {description}
          </p>
          <div className="flex items-center gap-1.5 mt-2 flex-wrap">
            {tags.map((t) => (
              <span
                key={t}
                className="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.06)] leading-none"
              >
                {t}
              </span>
            ))}
          </div>
        </div>

        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite?.(voice.id);
          }}
          aria-label={favorited ? `Remove ${voice.name} from favorites` : `Add ${voice.name} to favorites`}
          className={cn(
            "p-1.5 rounded-md self-start shrink-0 transition-all cursor-pointer mt-0.5",
            favorited
              ? "text-[rgb(var(--accent))]"
              : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--accent))]"
          )}
        >
          <Heart size={15} fill={favorited ? "currentColor" : "none"} />
        </button>
      </div>
    );
  }
);

VoiceCard.displayName = "VoiceCard";

