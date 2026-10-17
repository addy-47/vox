import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  CircleUserRound,
  Clock,
  Heart,
  Languages,
  Mars,
  Mic,
  Plus,
  Sparkles,
  Type,
  Venus,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import type { UnifiedVoiceDto } from "@/services/voicesService";
import type { UseVoiceCatalogResult } from "@/shared/hooks/useVoiceCatalog";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import {
  CatalogFilterItem,
  CatalogFilterMenu,
} from "../../providers/CatalogHeader";
import { CatalogModalToolbar } from "../../providers/CatalogModalToolbar";
import { CatalogModalShell } from "../../providers/CatalogModalShell";
import { CatalogSkeletonGrid } from "../../providers/CatalogSkeletonGrid";
import { VoiceCard } from "./VoiceCard";
import { CloneVoiceDrawer } from "./CloneVoiceDrawer";
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
    const [isCloneDrawerOpen, setIsCloneDrawerOpen] = useState(false);
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
    const handleCardSelect = useCallback(
      (id: string) => stageVoice(id),
      [stageVoice]
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
        action={
          canClone ? (
            <button
              type="button"
              onClick={() => setIsCloneDrawerOpen(true)}
              title={VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
              className="px-2.5 sm:px-3 py-1.5 rounded-lg text-[12px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
            >
              <Plus size={14} strokeWidth={2.5} />
              <span className="hidden sm:inline">{VOICE_CATALOG_COPY.cloneVoice}</span>
            </button>
          ) : undefined
        }
        filter={{
          label: VOICE_CATALOG_COPY.filterAria,
          open: isFilterDropdownOpen,
          activeCount: activeFilterCount,
          onToggle: () => setIsFilterDropdownOpen((v) => !v),
          onClose: () => setIsFilterDropdownOpen(false),
          menu: (
            <CatalogFilterMenu
              title={VOICE_CATALOG_COPY.filtersTitle}
              resetLabel={activeFilterCount > 0 ? VOICE_CATALOG_COPY.resetFilters : undefined}
              onReset={handleResetFilters}
            >
              <div className="flex flex-col gap-1">
                {scopeFilters.map((item) => (
                  <CatalogFilterItem
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
                  { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female },
                  { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male },
                  ...(counts.neutral > 0 ? [{ id: "neutral" as const, label: "Neutral", count: counts.neutral }] : []),
                ].map((item) => (
                  <CatalogFilterItem
                    key={item.id}
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
                <CatalogFilterItem
                  label={VOICE_CATALOG_COPY.english}
                  count={counts.english}
                  active={accent === "english"}
                  onSelect={() => {
                    setAccent((prev) => (prev === "english" ? null : "english"));
                    setIsFilterDropdownOpen(false);
                  }}
                />
                <CatalogFilterItem
                  label={VOICE_CATALOG_COPY.multilingual}
                  count={counts.multilingual}
                  active={accent === "multilingual"}
                  onSelect={() => {
                    setAccent((prev) => (prev === "multilingual" ? null : "multilingual"));
                    setIsFilterDropdownOpen(false);
                  }}
                />
              </div>
            </CatalogFilterMenu>
          ),
        }}
      />
    );

    const facetRailNode = (
      <div className="flex flex-col gap-3.5 w-full h-full overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5">
        {/* Library Section */}
        <div className="flex flex-col gap-1">
          {scopeFilters.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => setScope(item.id)}
              className={cn(
                "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium",
                scope === item.id
                  ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
              )}
            >
              <div className="flex items-center gap-2.5 truncate">
                <item.Icon size={14} className={cn("shrink-0", scope === item.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                <span className="truncate">{item.label}</span>
              </div>
              <span className={cn(
                "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                scope === item.id
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                  : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
              )}>
                {item.count}
              </span>
            </button>
          ))}
        </div>

        {/* Gender Section */}
        <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-1">
          <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold select-none">
            {VOICE_CATALOG_COPY.genderFacet}
          </div>
          {[
            { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female, Icon: Venus },
            { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male, Icon: Mars },
            ...(counts.neutral > 0
              ? [{ id: "neutral" as const, label: "Neutral", count: counts.neutral, Icon: CircleUserRound }]
              : []),
          ].map((item) => {
            const isActive = gender === item.id;
            const Icon = item.Icon;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setGender(isActive ? null : item.id)}
                className={cn(
                  "w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-[12.5px] transition-all cursor-pointer font-medium",
                  isActive
                    ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                )}
              >
                <div className="flex items-center gap-2 truncate">
                  <Icon size={13} className={cn("shrink-0", isActive ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                  <span className="truncate">{item.label}</span>
                </div>
                <span className={cn(
                  "text-[10px] font-mono px-1.5 py-0.5 rounded-full shrink-0",
                  isActive
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-bold"
                    : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]"
                )}>
                  {item.count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Accent / Language Section */}
        <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-1">
          <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold select-none">
            {VOICE_CATALOG_COPY.accentFacet}
          </div>
          {(
            [
              { id: "english", label: VOICE_CATALOG_COPY.english, count: counts.english },
              { id: "multilingual", label: VOICE_CATALOG_COPY.multilingual, count: counts.multilingual },
            ] as const
          ).map((item) => {
            const isActive = accent === item.id;
            const Icon = item.id === "english" ? Type : Languages;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setAccent(isActive ? null : item.id)}
                className={cn(
                  "w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-[12.5px] transition-all cursor-pointer font-medium",
                  isActive
                    ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                )}
              >
                <div className="flex items-center gap-1.5 truncate">
                  <Icon size={13} className={cn("shrink-0", isActive ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                  <span className="truncate">{item.label}</span>
                </div>
                <span className={cn(
                  "text-[10px] font-mono px-1.5 py-0.5 rounded-full shrink-0",
                  isActive
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-bold"
                    : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]"
                )}>
                  {item.count}
                </span>
              </button>
            );
          })}
        </div>

        {canClone && (
          <p className="mt-auto pt-3 px-2 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/70 border-t border-[rgba(var(--foreground),0.06)]">
            {VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
          </p>
        )}
      </div>
    );

    const emptyStateNode = (
      <div className="h-64 flex flex-col items-center justify-center text-center space-y-3">
        <p className="text-[15px] font-bold text-[rgb(var(--foreground))]">
          {VOICE_CATALOG_COPY.emptyTitle}
        </p>
        <p className="text-[13px] text-[rgb(var(--foreground-muted))] max-w-sm">
          {cloudSync
            ? VOICE_CATALOG_COPY.edgeSyncHint
            : VOICE_CATALOG_COPY.emptyHint}
        </p>
        <button
          type="button"
          onClick={handleResetFilters}
          className="mt-2 px-3.5 py-1.5 rounded-xl text-[12.5px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
        >
          {VOICE_CATALOG_COPY.resetFilters}
        </button>
      </div>
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
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3.5">
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
          </div>
        </CatalogModalShell>

        {canClone && (
          <CloneVoiceDrawer
            isOpen={isCloneDrawerOpen}
            onClose={() => setIsCloneDrawerOpen(false)}
            onCreated={handleVoiceCreated}
          />
        )}
      </>
    );
  }
);

VoiceModal.displayName = "VoiceModal";
