import { memo, useCallback, useMemo, useState } from "react";
import { AudioLines, Clock, Heart, Mic, Sparkles } from "lucide-react";
import { ErrorBoundary } from "@/shared/components/common/ErrorBoundary";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import { useVoiceCatalog } from "@/shared/hooks/useVoiceCatalog";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useVoiceScope } from "@/shared/hooks/useVoiceScope";
import type { UnifiedVoiceDto } from "@/services/voiceService";
import {
  CatalogFilterItem,
  CatalogFilterMenu,
  CatalogHeader,
  type CatalogSortOrder,
} from "../catalog/CatalogHeader";
import { VoiceCard } from "./VoiceCard";
import { VoiceCatalogModalView } from "./VoiceCatalogModalView";

type VoiceScopeFilter = "all" | "recent" | "favorites" | "custom";
type VoiceGenderFilter = "female" | "male" | "neutral";

function isEnglishVoice(v: UnifiedVoiceDto): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

export interface VoiceCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  providerId: string;
}

export const VoiceCatalogView = memo(({ layoutMode, providerId }: VoiceCatalogViewProps) => {
  // Inline header state (design-spec §10.1). Mirrors the facets offered by the
  // expand modal so both viewports operate on the same vocabulary.
  const [search, setSearch] = useState("");
  const [scopeFilter, setScopeFilter] = useState<VoiceScopeFilter>("all");
  const [genderFilter, setGenderFilter] = useState<VoiceGenderFilter | null>(null);
  const [accentFilter, setAccentFilter] = useState<"english" | "multilingual" | null>(null);
  const [sort, setSort] = useState<CatalogSortOrder>("az");
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isFilterOpen, setIsFilterOpen] = useState(false);

  const normalizedProviderId = providerId ?? "";

  // Engine -> voice family scope, resolved by the backend (no id literals
  // in the frontend). The catalog stays suspended until scope resolves so
  // another engine's (or all providers') voices are never rendered.
  const { scope, ready } = useVoiceScope(normalizedProviderId);
  const catalog = useVoiceCatalog({
    provider: ready ? scope.voice_provider || undefined : undefined,
    suspended: !ready,
    engine: normalizedProviderId,
    cloudSync: ready ? scope.cloud_sync : false,
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

  const targetProvider = (scope?.voice_provider || normalizedProviderId || "").toLowerCase();

  const scopedVoices = useMemo(() => {
    if (!targetProvider) return voices;
    return voices.filter((v) => (v.provider_id || "").toLowerCase() === targetProvider);
  }, [voices, targetProvider]);

  // Facet counts for the header filter popover.
  const facetCounts = useMemo(() => {
    const c = {
      all: scopedVoices.length,
      recent: 0,
      favorites: 0,
      custom: 0,
      female: 0,
      male: 0,
      neutral: 0,
      english: 0,
      multilingual: 0,
    };
    for (const v of scopedVoices) {
      if (v.last_used_at != null) c.recent += 1;
      if (v.is_favorite) c.favorites += 1;
      if (v.source_kind === "custom") c.custom += 1;
      if (v.gender === "female") c.female += 1;
      else if (v.gender === "male") c.male += 1;
      else if (v.gender === "neutral") c.neutral += 1;
      if (isEnglishVoice(v)) c.english += 1;
      else c.multilingual += 1;
    }
    return c;
  }, [scopedVoices]);

  const activeFilterCount =
    (scopeFilter === "all" ? 0 : 1) +
    (genderFilter === null ? 0 : 1) +
    (accentFilter === null ? 0 : 1);

  const resetFilters = useCallback(() => {
    setScopeFilter("all");
    setGenderFilter(null);
    setAccentFilter(null);
  }, []);

  // Every voice of the active engine, narrowed by the header facets. No inline
  // cap: the inline list scrolls (Lenis), so truncating to a window hides
  // voices the user can reach.
  const inlineVoices = useMemo(() => {
    let list = scopedVoices;

    if (scopeFilter === "recent") list = list.filter((v) => v.last_used_at != null);
    else if (scopeFilter === "favorites") list = list.filter((v) => v.is_favorite);
    else if (scopeFilter === "custom") list = list.filter((v) => v.source_kind === "custom");

    if (genderFilter) list = list.filter((v) => v.gender === genderFilter);
    if (accentFilter === "english") list = list.filter(isEnglishVoice);
    else if (accentFilter === "multilingual") list = list.filter((v) => !isEnglishVoice(v));

    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (v) =>
          v.name.toLowerCase().includes(q) ||
          (v.description || "").toLowerCase().includes(q) ||
          v.locale.toLowerCase().includes(q) ||
          v.accent?.toLowerCase().includes(q) ||
          v.tags.some((t) => t.toLowerCase().includes(q))
      );
    }

    const sorted = [...list];
    sorted.sort((a, b) => {
      const cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      return sort === "az" ? cmp : -cmp;
    });
    return sorted;
  }, [scopedVoices, scopeFilter, genderFilter, accentFilter, search, sort]);

  const closeSearch = useCallback(() => {
    setIsSearchOpen(false);
    setSearch("");
  }, []);

  const handleDiscard = useCallback(() => {
    stopPreview();
  }, [stopPreview]);

  const scopeFilterOptions = useMemo(
    () => [
      { id: "all" as const, label: VOICE_CATALOG_COPY.allVoices, count: facetCounts.all, Icon: Mic },
      { id: "favorites" as const, label: VOICE_CATALOG_COPY.favorites, count: facetCounts.favorites, Icon: Heart },
      { id: "recent" as const, label: VOICE_CATALOG_COPY.recent, count: facetCounts.recent, Icon: Clock },
      ...(scope.can_clone
        ? [{ id: "custom" as const, label: VOICE_CATALOG_COPY.cloned, count: facetCounts.custom, Icon: Sparkles }]
        : []),
    ],
    [facetCounts, scope.can_clone]
  );

  const genderFilterOptions = useMemo(
    () => [
      { id: "female" as const, label: VOICE_CATALOG_COPY.female, count: facetCounts.female },
      { id: "male" as const, label: VOICE_CATALOG_COPY.male, count: facetCounts.male },
      ...(facetCounts.neutral > 0
        ? [{ id: "neutral" as const, label: VOICE_CATALOG_COPY.neutral, count: facetCounts.neutral }]
        : []),
    ],
    [facetCounts]
  );

  return (
    <div className="w-full h-full flex flex-col min-h-0 gap-2.5 animate-fade-in">
      <CatalogHeader
        icon={<AudioLines size={14} className="text-[rgb(var(--accent))]" />}
        title={VOICE_CATALOG_COPY.title}
        filter={{
          label: VOICE_CATALOG_COPY.filterAria,
          open: isFilterOpen,
          activeCount: activeFilterCount,
          onToggle: () => setIsFilterOpen((v) => !v),
          onClose: () => setIsFilterOpen(false),
          menu: (
            <CatalogFilterMenu
              title={VOICE_CATALOG_COPY.filtersTitle}
              resetLabel={activeFilterCount > 0 ? VOICE_CATALOG_COPY.resetFilters : undefined}
              onReset={resetFilters}
            >
              <div className="flex flex-col gap-1">
                {scopeFilterOptions.map((item) => (
                  <CatalogFilterItem
                    key={item.id}
                    icon={item.Icon}
                    label={item.label}
                    count={item.count}
                    active={scopeFilter === item.id}
                    onSelect={() => {
                      setScopeFilter(item.id);
                      setIsFilterOpen(false);
                    }}
                  />
                ))}
              </div>
              <div className="mt-1 pt-1.5 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-0.5">
                <span className="px-1 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                  {VOICE_CATALOG_COPY.genderFacet}
                </span>
                {genderFilterOptions.map((item) => (
                  <CatalogFilterItem
                    key={item.id}
                    label={item.label}
                    count={item.count}
                    active={genderFilter === item.id}
                    onSelect={() =>
                      setGenderFilter((prev) => (prev === item.id ? null : item.id))
                    }
                  />
                ))}
              </div>
              <div className="mt-1 pt-1.5 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-0.5">
                <span className="px-1 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                  {VOICE_CATALOG_COPY.accentFacet}
                </span>
                <CatalogFilterItem
                  label={VOICE_CATALOG_COPY.english}
                  count={facetCounts.english}
                  active={accentFilter === "english"}
                  onSelect={() =>
                    setAccentFilter((prev) => (prev === "english" ? null : "english"))
                  }
                />
                <CatalogFilterItem
                  label={VOICE_CATALOG_COPY.multilingual}
                  count={facetCounts.multilingual}
                  active={accentFilter === "multilingual"}
                  onSelect={() =>
                    setAccentFilter((prev) => (prev === "multilingual" ? null : "multilingual"))
                  }
                />
              </div>
            </CatalogFilterMenu>
          ),
        }}
        sort={{
          order: sort,
          onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
        }}
        search={{
          open: isSearchOpen,
          value: search,
          placeholder: VOICE_CATALOG_COPY.searchPlaceholder,
          onChange: setSearch,
          onToggle: () => (isSearchOpen ? closeSearch() : setIsSearchOpen(true)),
          onClose: closeSearch,
        }}
      />
      <ExpandableList
        className="flex-1 min-h-0"
        domainId="tts"
        triggerPlacement="floating"
        inlineMaxHeightClass={layoutMode === "small" ? "max-h-[220px]" : undefined}
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
            voiceScope={targetProvider}
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
