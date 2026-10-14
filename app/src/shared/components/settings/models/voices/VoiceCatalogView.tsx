import { memo, useCallback, useMemo } from "react";
import { AudioLines } from "lucide-react";
import { ErrorBoundary } from "@/shared/components/common/ErrorBoundary";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import { useVoiceCatalog } from "@/shared/hooks/useVoiceCatalog";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useVoiceScope } from "@/shared/hooks/useVoiceScope";
import { VoiceCard } from "./VoiceCard";
import { VoiceCatalogModalView } from "./VoiceCatalogModalView";

export interface VoiceCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  providerId: string;
}

export const VoiceCatalogView = memo(({ layoutMode, providerId }: VoiceCatalogViewProps) => {
  // Engine -> voice family scope, resolved by the backend (no id literals
  // in the frontend). The catalog stays suspended until scope resolves so
  // another engine's (or all providers') voices are never rendered.
  const { scope, ready } = useVoiceScope(providerId);
  const catalog = useVoiceCatalog({
    provider: ready ? scope.voice_provider || undefined : undefined,
    suspended: !ready,
    engine: providerId,
  });
  const {
    voices,
    stagedId,
    stageVoice,
    toggleFavorite,
    playingId,
    togglePreview,
    stopPreview,
  } = catalog;

  const interactionState = useSessionStore((s) => s.interactionState);
  const sessionBusy = interactionState !== "Idle";

  const { containerRef: inlineGridRef } = useLenisScrollContainer<HTMLDivElement>();

  // Every voice of the active engine. No inline cap: the inline list scrolls
  // (Lenis), so truncating to a window hides voices the user can reach.
  const inlineVoices = useMemo(() => {
    return [...voices].sort((a, b) => {
      if (a.is_favorite !== b.is_favorite) return a.is_favorite ? -1 : 1;
      return (b.last_used_at ?? 0) - (a.last_used_at ?? 0);
    });
  }, [voices]);

  const handleDiscard = useCallback(() => {
    stopPreview();
  }, [stopPreview]);

  return (
    <div className="w-full h-full flex flex-col min-h-0 animate-fade-in">
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
        expandLabel={VOICE_CATALOG_COPY.expandLabel}
        ariaLabel={VOICE_CATALOG_COPY.catalogAriaLabel}
        modalSubtitle={VOICE_CATALOG_COPY.subtitle}
        onDiscard={handleDiscard}
        modalContent={({ close }) => (
          <ErrorBoundary name="VoiceCatalogModalContent">
            <VoiceCatalogModalView
            catalog={catalog}
            voiceScope={scope.voice_provider}
            canClone={scope.can_clone}
            cloudSync={scope.cloud_sync}
            onCommit={() => close()}
            onCancel={() => close()}
            />
          </ErrorBoundary>
        )}
      >
        <div
          ref={inlineGridRef}
          className={
            layoutMode === "small"
              ? "h-full overflow-y-auto overscroll-contain pr-1 custom-scrollbar"
              : "h-full overflow-y-auto overscroll-contain pr-1 custom-scrollbar"
          }
        >
          <div
            className={
              layoutMode === "small"
                ? "grid auto-rows-max content-start gap-2 min-h-full w-full grid-cols-1"
                : "grid auto-rows-max content-start gap-2 min-h-full w-full grid-cols-1 sm:grid-cols-2"
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
              previewProgress={playingId === v.id ? catalog.previewProgress : 0}
              previewDisabled={!v.has_preview && sessionBusy}
              hideBadges={true}
              onSelect={stageVoice}
              onToggleFavorite={toggleFavorite}
              onPreview={togglePreview}
              onSeekPreview={catalog.seekPreview}
            />
          ))}
          </div>
        </div>
      </ExpandableList>
    </div>
  );
});

VoiceCatalogView.displayName = "VoiceCatalogView";
