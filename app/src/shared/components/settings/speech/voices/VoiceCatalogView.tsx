import { memo, useCallback, useMemo, useState } from "react";
import { AudioLines, CircleDot, Clock, Globe, Heart, Languages, Mic, Search, Sparkles, User } from "lucide-react";
import { EmptyState } from "@/shared/components/common";
import { SPEECH_CARD_COPY, VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import { useVoiceCatalog, useVoiceScope } from "@/shared/hooks/useVoiceCatalog";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { CatalogReveal } from "@/shared/components/settings/shared/CatalogReveal";
import type { UnifiedVoiceDto } from "@/services/voicesService";
import {
  CardFilterItem,
  CardFilterMenu,
  CardHeader,
  type CardSortOrder,
} from "../../shared/CardHeader";
import { SpeechRateControl } from "../SpeechRateControl";
import { VoiceCard } from "./VoiceCard";
import { CloneVoiceCard } from "./CloneVoiceCard";
import { VoiceModal } from "./VoiceModal";
import { getSettingsInlineListMaxHeightClass } from "../../shared/settingsLayout";

type VoiceScopeFilter = "all" | "recent" | "favorites" | "custom";
type VoiceGenderFilter = "female" | "male" | "neutral";

function isEnglishVoice(v: UnifiedVoiceDto): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

export interface VoiceCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  /** Active TTS engine. Its voices are the only ones listed. */
  providerId: string;
  /** Returns to the speech-model view (optional if rendered as root card). */
  onBack?: () => void;
}

/**
 * The card-level voice catalog. It owns the whole card face: back breadcrumb,
 * title, and the `[rate] [filter] [sort] [search]` cluster in the header.
 */
export const VoiceCatalogView = memo(({ layoutMode, providerId, onBack }: VoiceCatalogViewProps) => {
  const [search, setSearch] = useState("");
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [scopeFilter, setScopeFilter] = useState<VoiceScopeFilter>("all");
  const [genderFilter, setGenderFilter] = useState<VoiceGenderFilter | null>(null);
  const [accentFilter, setAccentFilter] = useState<"english" | "multilingual" | null>(null);
  const [sort, setSort] = useState<CardSortOrder>("az");
  const [isFilterOpen, setIsFilterOpen] = useState(false);

  const normalizedProviderId = providerId ?? "";

  // Engine -> voice family scope, resolved by the backend (no id literals in
  // the frontend). The catalog stays suspended until scope resolves so another
  // engine's (or all providers') voices are never rendered.
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
    commitVoice,
    toggleFavorite,
    playingId,
    togglePreview,
    seekPreview,
    isSynthesizing,
    previewProgress,
  } = catalog;

  const interactionState = useSessionStore((s) => s.interactionState);
  const sessionBusy = interactionState !== "Idle";

  const targetProvider = (scope.voice_provider || normalizedProviderId || "").toLowerCase();

  const handleVoiceCreated = useCallback(
    (newVoice: UnifiedVoiceDto) => {
      catalog.reload();
      commitVoice(newVoice.id);
      togglePreview(newVoice);
    },
    [catalog, commitVoice, togglePreview]
  );

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

  // Every voice of the active engine, narrowed by the header facets.
  const filteredVoices = useMemo(() => {
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
      { id: "female" as const, label: VOICE_CATALOG_COPY.female, count: facetCounts.female, Icon: User },
      { id: "male" as const, label: VOICE_CATALOG_COPY.male, count: facetCounts.male, Icon: User },
      ...(facetCounts.neutral > 0
        ? [{ id: "neutral" as const, label: VOICE_CATALOG_COPY.neutral, count: facetCounts.neutral, Icon: CircleDot }]
        : []),
    ],
    [facetCounts]
  );

  return (
    <div className="w-full h-full flex flex-col min-h-0 gap-2.5 animate-fade-in">
      <CardHeader
        icon={<Mic size={14} className="text-[rgb(var(--accent))]" />}
        title={VOICE_CATALOG_COPY.title}
        onBack={onBack}
        action={<SpeechRateControl />}
        filter={{
          label: VOICE_CATALOG_COPY.filterAria,
          open: isFilterOpen,
          activeCount: activeFilterCount,
          onToggle: () => setIsFilterOpen((v) => !v),
          onClose: () => setIsFilterOpen(false),
          menu: (
            <CardFilterMenu
              title={VOICE_CATALOG_COPY.filtersTitle}
              resetLabel={activeFilterCount > 0 ? VOICE_CATALOG_COPY.resetFilters : undefined}
              onReset={resetFilters}
            >
              <div className="flex flex-col gap-1">
                {scopeFilterOptions.map((item) => (
                  <CardFilterItem
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
                  <CardFilterItem
                    key={item.id}
                    icon={item.Icon}
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
                <CardFilterItem
                  icon={Globe}
                  label={VOICE_CATALOG_COPY.english}
                  count={facetCounts.english}
                  active={accentFilter === "english"}
                  onSelect={() =>
                    setAccentFilter((prev) => (prev === "english" ? null : "english"))
                  }
                />
                <CardFilterItem
                  icon={Languages}
                  label={VOICE_CATALOG_COPY.multilingual}
                  count={facetCounts.multilingual}
                  active={accentFilter === "multilingual"}
                  onSelect={() =>
                    setAccentFilter((prev) => (prev === "multilingual" ? null : "multilingual"))
                  }
                />
              </div>
            </CardFilterMenu>
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
          onToggle: () => setIsSearchOpen((v) => !v),
          onClose: () => {
            setSearch("");
            setIsSearchOpen(false);
          },
        }}
      />

      <ExpandableList
        className="flex-1 min-h-0"
        inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
        hideInlineTrigger
        icon={<Mic size={16} className="text-[rgb(var(--accent))]" />}
        title={VOICE_CATALOG_COPY.title}
        subtitle={VOICE_CATALOG_COPY.subtitle}
        expandLabel={SPEECH_CARD_COPY.expandVoices}
        ariaLabel={VOICE_CATALOG_COPY.title}
        modalContent={({ close }) => (
          <VoiceModal
            catalog={catalog}
            voiceScope={targetProvider}
            canClone={ready && scope.can_clone}
            cloudSync={ready && scope.cloud_sync}
            onCommit={close}
            onCancel={close}
          />
        )}
      >
        {filteredVoices.length === 0 ? (
          <div className="flex-1 min-h-[140px] flex items-center justify-center p-3">
            <EmptyState
              size="sm"
              icon={voices.length === 0 ? AudioLines : Search}
              title={voices.length === 0 ? VOICE_CATALOG_COPY.emptyTitle : "No Voices Found"}
              description={
                voices.length === 0
                  ? scope.cloud_sync
                    ? VOICE_CATALOG_COPY.edgeSyncHint
                    : "No voices available for this provider. Configure an API key or check provider connection."
                  : search
                  ? `No voices match "${search}" or active filters.`
                  : "No voices match the selected filters."
              }
              action={
                voices.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => {
                      setSearch("");
                      setScopeFilter("all");
                      setGenderFilter(null);
                      setAccentFilter(null);
                    }}
                    className="px-3 py-1 rounded-lg text-[11px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
                  >
                    {VOICE_CATALOG_COPY.resetFilters}
                  </button>
                ) : undefined
              }
            />
          </div>
        ) : (
          <CatalogReveal
            className="flex-1 min-h-0"
            featherHeight={80}
            scrollerClassName="flex-1 min-h-0 overflow-y-auto overflow-x-hidden overscroll-contain pr-1 custom-scrollbar [scrollbar-gutter:stable]"
          >
            <div
              className={
                layoutMode === "small"
                  ? "grid auto-rows-max content-start gap-2 min-h-full w-full grid-cols-1"
                  : "grid auto-rows-max content-start gap-2 min-h-full w-full grid-cols-1 sm:grid-cols-2"
              }
            >
              {filteredVoices.map((v) => (
                <VoiceCard
                  key={v.id}
                  voice={v}
                  selected={stagedId === v.id}
                  favorited={v.is_favorite}
                  previewPlaying={playingId === v.id}
                  isSynthesizing={isSynthesizing && playingId === v.id}
                  previewProgress={playingId === v.id ? previewProgress : 0}
                  previewDisabled={!v.has_preview && sessionBusy}
                  hideBadges={true}
                  onSelect={commitVoice}
                  onToggleFavorite={toggleFavorite}
                  onPreview={togglePreview}
                  onSeekPreview={seekPreview}
                />
              ))}
              {ready && scope.can_clone && (
                <CloneVoiceCard onCreated={handleVoiceCreated} />
              )}
            </div>
          </CatalogReveal>
        )}
      </ExpandableList>
    </div>
  );
});

VoiceCatalogView.displayName = "VoiceCatalogView";
