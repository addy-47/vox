import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  AudioLines,
  CircleDot,
  CircleUserRound,
  Clock,
  Globe,
  Heart,
  Languages,
  Mars,
  Mic,
  Sparkles,
  Type,
  User,
  Venus,
} from "lucide-react";
import { EmptyState } from "@/shared/components/common";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import type { UnifiedVoiceDto } from "@/services/voicesService";
import type { UseVoiceCatalogResult } from "@/shared/hooks/useVoiceCatalog";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import {
  CardFilterItem,
  CardFilterMenu,
} from "../../shared/CardHeader";
import { CatalogModalToolbar } from "../../shared/CatalogModalToolbar";
import { CatalogModalShell } from "../../shared/CatalogModalShell";
import { CatalogFacetRail, type CatalogFacetGroup } from "../../shared/CatalogFacetRail";
import { CatalogSkeletonGrid } from "../../shared/CatalogSkeletonGrid";
import { VoiceCard } from "./VoiceCard";
import { CloneVoiceCard } from "./CloneVoiceCard";
import { VoicePreviewBanner } from "./VoicePreviewBanner";

export interface VoiceModalProps {
  catalog: UseVoiceCatalogResult;
  /** Opaque backend-resolved DB provider filter ("" = all providers). */
  voiceScope: string;
  /** Backend-resolved `ProviderCaps.clone` for the active engine. */
  canClone: boolean;
  /** Backend-resolved cloud-sync hint for the active engine family. */
  cloudSync: boolean;
  onCommit: (id: string) => void;
  onCancel: () => void;
}

type Scope = "all" | "recent" | "favorites" | "custom";
type Sort = "az" | "za";

function isEnglish(v: UnifiedVoiceDto): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

export const VoiceModal = memo(
  ({ catalog, voiceScope, canClone, cloudSync, onCommit: _onCommit, onCancel: _onCancel }: VoiceModalProps) => {
    const {
      voices,
      loading,
      stagedId,
      stageVoice,
      commitVoice,
      toggleFavorite,
      playingId,
      isSynthesizing,
      previewProgress,
      previewCurrentTime,
      previewDuration,
      previewPrompt,
      setPreviewPrompt,
      togglePreview,
      seekPreview,
      reload,
    } = catalog;

    const interactionState = useSessionStore((s) => s.interactionState);
    const sessionBusy = interactionState !== "Idle";

    const [search, setSearch] = useState("");
    const deferredSearch = useDeferredValue(search);
    const [scope, setScope] = useState<Scope>("all");
    const [gender, setGender] = useState<string | null>(null);
    const [accent, setAccent] = useState<"english" | "multilingual" | null>(null);
    const [sort, setSort] = useState<Sort>("az");
    const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);

    const activeFilterCount = useMemo(() => {
      let count = 0;
      if (scope !== "all") count += 1;
      if (gender !== null) count += 1;
      if (accent !== null) count += 1;
      return count;
    }, [scope, gender, accent]);

    const normalizedScope = (voiceScope || "").toLowerCase();
    const scopedVoices = useMemo(() => {
      if (!normalizedScope) return voices;
      return voices.filter((v) => v.provider_id.toLowerCase() === normalizedScope);
    }, [voices, normalizedScope]);

    const { containerRef, scrollToTop } =
      useLenisScrollContainer<HTMLDivElement>();

    // Reset scroll only once the debounced filter settles — never per keystroke.
    useEffect(() => {
      scrollToTop();
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [deferredSearch, scope, gender, accent, sort]);

    const counts = useMemo(() => {
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
        else c.neutral += 1;
        if (isEnglish(v)) c.english += 1;
        else c.multilingual += 1;
      }
      return c;
    }, [scopedVoices]);

    const filtered = useMemo(() => {
      const q = deferredSearch.trim().toLowerCase();
      let list = scopedVoices;
      if (scope === "recent") list = list.filter((v) => v.last_used_at != null);
      else if (scope === "favorites") list = list.filter((v) => v.is_favorite);
      else if (scope === "custom") list = list.filter((v) => v.source_kind === "custom");

      if (gender) list = list.filter((v) => v.gender === gender);
      if (accent === "english") list = list.filter(isEnglish);
      else if (accent === "multilingual") list = list.filter((v) => !isEnglish(v));

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
      if (sort === "az") {
        sorted.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
      } else {
        sorted.sort((a, b) => b.name.localeCompare(a.name, undefined, { sensitivity: "base" }));
      }
      return sorted;
    }, [scopedVoices, scope, gender, accent, deferredSearch, sort]);

    const staged = (stagedId && scopedVoices.find((v) => v.id === stagedId)) || null;
    const stagedPlaying = staged != null && playingId === staged.id;

    const handleCardPreview = useCallback(
      (v: UnifiedVoiceDto) => togglePreview(v),
      [togglePreview]
    );
    // A pick is a commit: it records the use (feeds the Recent facet) and
    // stops any preview, so the header rate control targets the chosen voice.
    const handleCardSelect = useCallback(
      (id: string) => commitVoice(id),
      [commitVoice]
    );
    const handleCardFavorite = useCallback(
      (id: string) => {
        toggleFavorite(id);
      },
      [toggleFavorite]
    );

    const handleVoiceCreated = useCallback(
      (newVoice: UnifiedVoiceDto) => {
        reload();
        stageVoice(newVoice.id);
        togglePreview(newVoice);
      },
      [reload, stageVoice, togglePreview]
    );

    const handleResetFilters = useCallback(() => {
      setSearch("");
      setScope("all");
      setGender(null);
      setAccent(null);
    }, []);

    const scopeFilters: Array<{ id: Scope; label: string; count: number; Icon: typeof Mic }> = [
      { id: "all", label: VOICE_CATALOG_COPY.allVoices, count: counts.all, Icon: Mic },
      { id: "favorites", label: VOICE_CATALOG_COPY.favorites, count: counts.favorites, Icon: Heart },
      { id: "recent", label: VOICE_CATALOG_COPY.recent, count: counts.recent, Icon: Clock },
      ...(canClone
        ? [{ id: "custom" as Scope, label: VOICE_CATALOG_COPY.cloned, count: counts.custom, Icon: Sparkles }]
        : []),
    ];

    const toolbarNode = (
      <CatalogModalToolbar
        search={{
          value: search,
          placeholder: VOICE_CATALOG_COPY.searchPlaceholder,
          onChange: setSearch,
        }}
        sort={{
          order: sort,
          onToggle: () => setSort((s) => (s === "az" ? "za" : "az")),
        }}
        itemCount={{
          current: filtered.length,
          total: scopedVoices.length,
          entityName: "voices",
        }}
        filter={{
          label: VOICE_CATALOG_COPY.filterAria,
          open: isFilterDropdownOpen,
          activeCount: activeFilterCount,
          onToggle: () => setIsFilterDropdownOpen((v) => !v),
          onClose: () => setIsFilterDropdownOpen(false),
          menu: (
            <CardFilterMenu
              title={VOICE_CATALOG_COPY.filtersTitle}
              resetLabel={activeFilterCount > 0 ? VOICE_CATALOG_COPY.resetFilters : undefined}
              onReset={handleResetFilters}
            >
              <div className="flex flex-col gap-1">
                {scopeFilters.map((item) => (
                  <CardFilterItem
                    key={item.id}
                    icon={item.Icon}
                    label={item.label}
                    count={item.count}
                    active={scope === item.id}
                    onSelect={() => {
                      setScope(item.id);
                      setIsFilterDropdownOpen(false);
                    }}
                  />
                ))}
              </div>
              <div className="mt-1 pt-1.5 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-0.5">
                <span className="px-1 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                  {VOICE_CATALOG_COPY.genderFacet}
                </span>
                {[
                  { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female, icon: User },
                  { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male, icon: User },
                  ...(counts.neutral > 0 ? [{ id: "neutral" as const, label: VOICE_CATALOG_COPY.neutral, count: counts.neutral, icon: CircleDot }] : []),
                ].map((item) => (
                  <CardFilterItem
                    key={item.id}
                    icon={item.icon}
                    label={item.label}
                    count={item.count}
                    active={gender === item.id}
                    onSelect={() => {
                      setGender((prev) => (prev === item.id ? null : item.id));
                      setIsFilterDropdownOpen(false);
                    }}
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
                  count={counts.english}
                  active={accent === "english"}
                  onSelect={() => {
                    setAccent((prev) => (prev === "english" ? null : "english"));
                    setIsFilterDropdownOpen(false);
                  }}
                />
                <CardFilterItem
                  icon={Languages}
                  label={VOICE_CATALOG_COPY.multilingual}
                  count={counts.multilingual}
                  active={accent === "multilingual"}
                  onSelect={() => {
                    setAccent((prev) => (prev === "multilingual" ? null : "multilingual"));
                    setIsFilterDropdownOpen(false);
                  }}
                />
              </div>
            </CardFilterMenu>
          ),
        }}
      />
    );

    const handleSelectFacet = useCallback((id: string) => {
      if (id === "all" || id === "favorites" || id === "recent" || id === "custom") {
        setScope(id as Scope);
      } else if (id === "female" || id === "male" || id === "neutral") {
        setGender((prev) => (prev === id ? null : (id as "female" | "male" | "neutral")));
      } else if (id === "english" || id === "multilingual") {
        setAccent((prev) => (prev === id ? null : (id as "english" | "multilingual")));
      }
    }, []);

    const voiceFacetGroups = useMemo<CatalogFacetGroup[]>(() => [
      {
        id: "scope",
        items: scopeFilters.map((s) => ({
          id: s.id,
          label: s.label,
          count: s.count,
          Icon: s.Icon,
        })),
      },
      {
        id: "gender",
        title: VOICE_CATALOG_COPY.genderFacet,
        items: [
          { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female, Icon: Venus },
          { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male, Icon: Mars },
          ...(counts.neutral > 0
            ? [{ id: "neutral", label: VOICE_CATALOG_COPY.neutral, count: counts.neutral, Icon: CircleUserRound }]
            : []),
        ],
      },
      {
        id: "accent",
        title: VOICE_CATALOG_COPY.accentFacet,
        items: [
          { id: "english", label: VOICE_CATALOG_COPY.english, count: counts.english, Icon: Type },
          { id: "multilingual", label: VOICE_CATALOG_COPY.multilingual, count: counts.multilingual, Icon: Languages },
        ],
      },
    ], [scopeFilters, counts]);

    const facetRailNode = (
      <CatalogFacetRail
        groups={voiceFacetGroups}
        selectedId={gender ?? accent ?? scope}
        onSelect={handleSelectFacet}
        footer={
          canClone ? (
            <p className="mt-auto pt-3 px-2 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/70 border-t border-[rgba(var(--foreground),0.06)]">
              {VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
            </p>
          ) : undefined
        }
      />
    );

    const emptyStateNode = (
      <EmptyState
        icon={AudioLines}
        title={VOICE_CATALOG_COPY.emptyTitle}
        description={
          cloudSync
            ? VOICE_CATALOG_COPY.edgeSyncHint
            : VOICE_CATALOG_COPY.emptyHint
        }
        action={
          <button
            type="button"
            onClick={handleResetFilters}
            className="px-3.5 py-1.5 rounded-xl text-[12.5px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
          >
            {VOICE_CATALOG_COPY.resetFilters}
          </button>
        }
        className="py-12 border-dashed border-[rgba(var(--accent),0.15)] bg-transparent"
      />
    );

    const voicePreviewBannerNode = (
      <VoicePreviewBanner
        voice={staged}
        isPlaying={stagedPlaying}
        isSynthesizing={isSynthesizing && stagedPlaying}
        previewProgress={previewProgress}
        previewCurrentTime={previewCurrentTime}
        previewDuration={previewDuration}
        previewPrompt={previewPrompt}
        sessionBusy={sessionBusy}
        onTogglePreview={togglePreview}
        onToggleFavorite={toggleFavorite}
        onSeekPreview={seekPreview}
        onSetPreviewPrompt={setPreviewPrompt}
      />
    );

    return (
      <>
        <CatalogModalShell
          toolbar={toolbarNode}
          facetRail={facetRailNode}
          scrollRef={containerRef}
          footer={voicePreviewBannerNode}
          isLoading={loading}
          skeletonGrid={<CatalogSkeletonGrid count={6} />}
          isEmpty={filtered.length === 0}
          emptyState={emptyStateNode}
        >
          <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
            {filtered.map((v) => (
              <VoiceCard
                key={v.id}
                voice={v}
                selected={staged?.id === v.id}
                favorited={v.is_favorite}
                previewPlaying={playingId === v.id}
                isSynthesizing={isSynthesizing && playingId === v.id}
                previewProgress={playingId === v.id ? previewProgress : 0}
                previewDisabled={!v.has_preview && sessionBusy}
                onSelect={handleCardSelect}
                onToggleFavorite={handleCardFavorite}
                onPreview={handleCardPreview}
                onSeekPreview={seekPreview}
              />
            ))}
            {canClone && (
              <CloneVoiceCard onCreated={handleVoiceCreated} />
            )}
          </div>
        </CatalogModalShell>
      </>
    );
  }
);

VoiceModal.displayName = "VoiceModal";
