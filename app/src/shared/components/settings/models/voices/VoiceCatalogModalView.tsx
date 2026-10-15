import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  ArrowDownAZ,
  ArrowUpZA,
  Clock,
  Filter,
  Heart,
  Languages,
  Loader2,
  Mic,
  Plus,
  Search,
  Sparkles,
  X,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { useSessionStore } from "@/store/sessionStore";
import type { UnifiedVoiceDto } from "@/services/voiceService";
import type { UseVoiceCatalogResult } from "@/shared/hooks/useVoiceCatalog";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { VoiceCard } from "./VoiceCard";
import { CloneVoiceDrawer } from "./CloneVoiceDrawer";
import { VoicePreviewBanner } from "./VoicePreviewBanner";

export interface VoiceCatalogModalViewProps {
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

export const VoiceCatalogModalView = memo(
  ({ catalog, voiceScope, canClone, cloudSync, onCommit: _onCommit, onCancel: _onCancel }: VoiceCatalogModalViewProps) => {
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
    const filterDropdownRef = useRef<HTMLDivElement>(null);

    // Close compact filter dropdown on outside pointerdown or Escape
    useEffect(() => {
      if (!isFilterDropdownOpen) return;
      const handleClickOutside = (e: MouseEvent | TouchEvent) => {
        if (filterDropdownRef.current && !filterDropdownRef.current.contains(e.target as Node)) {
          setIsFilterDropdownOpen(false);
        }
      };
      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === "Escape") {
          setIsFilterDropdownOpen(false);
        }
      };
      document.addEventListener("pointerdown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
      return () => {
        document.removeEventListener("pointerdown", handleClickOutside);
        document.removeEventListener("keydown", handleKeyDown);
      };
    }, [isFilterDropdownOpen]);

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

    const staged = scopedVoices.find((v) => v.id === stagedId) || scopedVoices[0] || null;
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

    return (
      <div className="flex flex-col h-full min-h-0 gap-3.5">
        {/* Top Toolbar: Search + Gender Chips + Sort Icon + Count + Clone Action */}
        {/* Top Toolbar: Search + Filter Dropdown (compact) + Gender Chips (desktop) + Sort + Count + Clone */}
        <div className="flex items-center justify-between gap-2.5 shrink-0 flex-wrap">
          {/* Mobile/Compact Filter Dropdown Trigger */}
          <div ref={filterDropdownRef} className="relative md:hidden shrink-0">
            <button
              type="button"
              onClick={() => setIsFilterDropdownOpen((v) => !v)}
              className={cn(
                "p-2 rounded-xl border transition-all cursor-pointer flex items-center gap-1.5",
                isFilterDropdownOpen || activeFilterCount > 0
                  ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.35)] shadow-xs"
                  : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))]"
              )}
              aria-label={VOICE_CATALOG_COPY.filterAria}
              aria-expanded={isFilterDropdownOpen}
            >
              <Filter size={15} />
              {activeFilterCount > 0 && (
                <span className="w-4 h-4 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[9.5px] font-mono font-bold flex items-center justify-center leading-none">
                  {activeFilterCount}
                </span>
              )}
            </button>

            {/* Compact Filter Menu Popover */}
            {isFilterDropdownOpen && (
              <div className="absolute left-0 top-full mt-2 z-50 w-72 max-h-[75vh] overflow-y-auto rounded-xl bg-[rgba(var(--card),0.96)] border border-[rgba(var(--foreground),0.12)] shadow-2xl p-3.5 flex flex-col gap-3 custom-scrollbar animate-fade-in">
                <div className="flex items-center justify-between pb-2 border-b border-[rgba(var(--foreground),0.06)]">
                  <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">
                    {VOICE_CATALOG_COPY.filtersTitle}
                  </span>
                  {activeFilterCount > 0 && (
                    <button
                      type="button"
                      onClick={() => {
                        setScope("all");
                        setGender(null);
                        setAccent(null);
                      }}
                      className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline cursor-pointer"
                    >
                      {VOICE_CATALOG_COPY.resetFilters}
                    </button>
                  )}
                </div>

                {/* Library Scopes */}
                <div className="flex flex-col gap-1">
                  {scopeFilters.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => setScope(item.id)}
                      className={cn(
                        "w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-[12.5px] transition-all cursor-pointer font-medium",
                        scope === item.id
                          ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                          : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                      )}
                    >
                      <div className="flex items-center gap-2 truncate">
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

                {/* Gender Options */}
                <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-1">
                  <div className="px-1 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                    {VOICE_CATALOG_COPY.genderFacet}
                  </div>
                  {(
                    [
                      { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female },
                      { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male },
                      ...(counts.neutral > 0
                        ? [{ id: "neutral" as const, label: "Neutral", count: counts.neutral }]
                        : []),
                    ]
                  ).map((item) => {
                    const isActive = gender === item.id;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => setGender(isActive ? null : item.id)}
                        className={cn(
                          "w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-[12px] transition-all cursor-pointer font-medium",
                          isActive
                            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold"
                            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                        )}
                      >
                        <span>{item.label}</span>
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

                {/* Accent / Language Options */}
                <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-1">
                  <div className="px-1 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                    {VOICE_CATALOG_COPY.accentFacet}
                  </div>
                  {(
                    [
                      { id: "english", label: VOICE_CATALOG_COPY.english, count: counts.english },
                      { id: "multilingual", label: VOICE_CATALOG_COPY.multilingual, count: counts.multilingual },
                    ] as const
                  ).map((item) => {
                    const isActive = accent === item.id;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => setAccent(isActive ? null : item.id)}
                        className={cn(
                          "w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-[12px] transition-all cursor-pointer font-medium",
                          isActive
                            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold"
                            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                        )}
                      >
                        <div className="flex items-center gap-1.5 truncate">
                          <Languages size={13} className={cn("shrink-0", isActive ? "text-[rgb(var(--accent))]" : "opacity-60")} />
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
              </div>
            )}
          </div>

          {/* Search Box */}
          <div className="flex-1 min-w-[140px] sm:min-w-[180px] flex items-center gap-2 px-3.5 py-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
            <Search size={14} className="text-[rgb(var(--foreground-muted))] shrink-0" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={VOICE_CATALOG_COPY.searchPlaceholder}
              aria-label={VOICE_CATALOG_COPY.searchAria}
              className="w-full bg-transparent border-none outline-none text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            {search && (
              <button
                type="button"
                onClick={() => setSearch("")}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
              >
                <X size={13} />
              </button>
            )}
          </div>



          {/* Sort button */}
          <button
            type="button"
            onClick={() => setSort((s) => (s === "az" ? "za" : "az"))}
            className="p-2 rounded-xl text-[12px] font-medium bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] hover:border-[rgb(var(--accent))]/40 hover:text-[rgb(var(--accent))] flex items-center justify-center cursor-pointer shrink-0 transition-all"
            title={sort === "az" ? "Sort: A–Z" : "Sort: Z–A"}
            aria-label="Toggle alphabetical sort order"
          >
            {sort === "az" ? (
              <ArrowDownAZ size={15} className="text-[rgb(var(--accent))]" />
            ) : (
              <ArrowUpZA size={15} className="text-[rgb(var(--accent))]" />
            )}
          </button>

          <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0 select-none">
            <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{filtered.length}</span> of {scopedVoices.length} {VOICE_CATALOG_COPY.showingOf}</span>
            <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{filtered.length}</span>/{scopedVoices.length}</span>
          </div>

          {canClone && (
            <button
              type="button"
              onClick={() => setIsCloneDrawerOpen(true)}
              title={VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
              className="px-2.5 sm:px-3 py-1.5 rounded-lg text-[12px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
            >
              <Plus size={14} strokeWidth={2.5} />
              <span className="hidden sm:inline">{VOICE_CATALOG_COPY.cloneVoice}</span>
            </button>
          )}
        </div>

        {/* Main Content Area */}
        <div className="flex-1 min-h-0 flex gap-4 overflow-hidden">
          {/* Facet Sidebar: clean categories, gender, languages with count pills (NO sort section) */}
          <div className="w-[200px] shrink-0 hidden md:flex flex-col gap-3.5 overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5">
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
              <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                {VOICE_CATALOG_COPY.genderFacet}
              </div>
              {(
                [
                  { id: "female", label: VOICE_CATALOG_COPY.female, count: counts.female },
                  { id: "male", label: VOICE_CATALOG_COPY.male, count: counts.male },
                  ...(counts.neutral > 0
                    ? [{ id: "neutral" as const, label: "Neutral", count: counts.neutral }]
                    : []),
                ]
              ).map((item) => {
                const isActive = gender === item.id;
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
                    <span>{item.label}</span>
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
              <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                {VOICE_CATALOG_COPY.accentFacet}
              </div>
              {(
                [
                  { id: "english", label: VOICE_CATALOG_COPY.english, count: counts.english },
                  { id: "multilingual", label: VOICE_CATALOG_COPY.multilingual, count: counts.multilingual },
                ] as const
              ).map((item) => {
                const isActive = accent === item.id;
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
                    <span className="truncate">{item.label}</span>
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
              <p className="mt-3 px-2 text-[11px] leading-relaxed text-[rgb(var(--foreground-muted))]/70">
                {VOICE_CATALOG_COPY.cloneVoiceChatterboxNote}
              </p>
            )}
          </div>

          {/* Voice Cards Grid: 3 cards per row, generous breathing room */}
          <div
            ref={containerRef}
            className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar pr-1.5"
          >
            {/* Stable Lenis content wrapper: keeps virtual height measurement
                correct across filter/empty-state swaps. */}
            <div className="min-h-full w-full">
            {loading ? (
              <div className="h-64 flex flex-col items-center justify-center text-[13px] text-[rgb(var(--foreground-muted))] gap-2">
                <Loader2 size={24} className="animate-spin text-[rgb(var(--accent))]" />
                <span>Loading unified voices…</span>
              </div>
            ) : filtered.length === 0 ? (
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
            ) : (
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
            )}
            </div>
          </div>
        </div>

        {/* Decoupled Bottom Voice Preview Banner */}
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

        {canClone && (
          <CloneVoiceDrawer
            isOpen={isCloneDrawerOpen}
            onClose={() => setIsCloneDrawerOpen(false)}
            onCreated={handleVoiceCreated}
          />
        )}
      </div>
    );
  }
);

VoiceCatalogModalView.displayName = "VoiceCatalogModalView";
