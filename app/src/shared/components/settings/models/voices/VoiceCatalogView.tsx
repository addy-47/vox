import { memo, useCallback, useMemo } from "react";
import { AudioLines } from "lucide-react";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import { useVoiceCatalog } from "@/shared/hooks/useVoiceCatalog";
import { VoiceCard } from "./VoiceCard";
import { VoiceCatalogModalView } from "./VoiceCatalogModalView";

export interface VoiceCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  providerId: string;
}

const INLINE_WINDOW = 4;

export const VoiceCatalogView = memo(({ layoutMode, providerId }: VoiceCatalogViewProps) => {
  const catalog = useVoiceCatalog({ provider: providerId || undefined });
  const {
    voices,
    loading,
    stagedId,
    stageVoice,
    toggleFavorite,
    playingId,
    togglePreview,
    stopPreview,
  } = catalog;

  const interactionState = useSessionStore((s) => s.interactionState);
  const sessionBusy = interactionState !== "Idle";

  const inlineVoices = useMemo(() => {
    const sorted = [...voices].sort((a, b) => {
      if (a.is_favorite !== b.is_favorite) return a.is_favorite ? -1 : 1;
      return (b.last_used_at ?? 0) - (a.last_used_at ?? 0);
    });
    return sorted.slice(0, INLINE_WINDOW);
  }, [voices]);

  const handleDiscard = useCallback(() => {
    stopPreview();
  }, [stopPreview]);

  return (
    <div className="w-full h-full flex flex-col min-h-0 space-y-2 animate-fade-in">
      <div className="flex items-center justify-between gap-2 min-h-[34px] pb-1 border-b border-[rgba(var(--foreground),0.04)] shrink-0">
        <div className="flex flex-col min-w-0">
          <span className="font-bold text-[rgb(var(--foreground))] text-[13px] flex items-center gap-1.5 truncate">
            <AudioLines size={15} className="text-[rgb(var(--accent))] shrink-0" />
            <span>{VOICE_CATALOG_COPY.title}</span>
          </span>
        </div>
        <span className="px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] text-[11px] font-mono font-medium shrink-0">
          {loading ? "…" : `${voices.length} voices`}
        </span>
      </div>

      <ExpandableList
        className="flex-1 min-h-0"
        domainId="tts"
        triggerPlacement="floating"
        inlineMaxHeightClass={layoutMode === "small" ? "max-h-[235px]" : undefined}
        icon={<AudioLines size={16} className="text-[rgb(var(--accent))]" />}
        title={
          <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
            {VOICE_CATALOG_COPY.title}
          </span>
        }
        modalSubtitle={
          <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
            {VOICE_CATALOG_COPY.subtitle}
          </span>
        }
        expandLabel={VOICE_CATALOG_COPY.expandLabel}
        ariaLabel={VOICE_CATALOG_COPY.catalogAriaLabel}
        onDiscard={handleDiscard}
        modalContent={({ close }) => (
          <VoiceCatalogModalView
            catalog={catalog}
            onCommit={() => close()}
            onCancel={() => close()}
          />
        )}
      >
        <div
          className={
            layoutMode === "small"
              ? "h-full overflow-y-auto pr-1 grid auto-rows-max content-start gap-2 custom-scrollbar grid-cols-1"
              : "h-full overflow-y-auto pr-1 grid auto-rows-max content-start gap-2 custom-scrollbar grid-cols-1 sm:grid-cols-2"
          }
        >
          {inlineVoices.map((v) => (
            <VoiceCard
              key={v.id}
              voice={v}
              selected={stagedId === v.id}
              favorited={v.is_favorite}
              previewPlaying={playingId === v.id}
              isSynthesizing={catalog.isSynthesizing && playingId === v.id}
              previewDisabled={!v.has_preview && sessionBusy}
              onSelect={stageVoice}
              onToggleFavorite={toggleFavorite}
              onPreview={togglePreview}
            />
          ))}
        </div>
      </ExpandableList>
    </div>
  );
});

VoiceCatalogView.displayName = "VoiceCatalogView";
