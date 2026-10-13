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
  ArrowUpDown,
  Clock,
  Edit3,
  Filter,
  Globe,
  Heart,
  Languages,
  Loader2,
  Pause,
  Play,
  Plus,
  Search,
  Sparkles,
  Star,
  Users,
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
type Sort = "popular" | "az";

function isEnglish(v: UnifiedVoiceDto): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

function tagLabel(tag?: string | null): string {
  if (!tag) return "";
  const t = tag.trim();
  if (!t) return t;
  return t.charAt(0).toUpperCase() + t.slice(1);
}

function getVoiceWaveform(voiceId: string, count = 44): number[] {
  let hash = 0;
  for (let i = 0; i < voiceId.length; i++) {
    hash = (hash << 5) - hash + voiceId.charCodeAt(i);
    hash |= 0;
  }
  const bars: number[] = [];
  for (let i = 0; i < count; i++) {
    const x = Math.sin((i + 1) * 0.45 + Math.abs(hash % 41)) * 10000;
    const rand = x - Math.floor(x);
    const envelope = Math.sin((i / (count - 1)) * Math.PI);
    const height = Math.max(0.2, Math.min(1.0, 0.22 + rand * 0.7 * (0.35 + envelope * 0.65)));
    bars.push(height);
  }
  return bars;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const VoiceCatalogModalView = memo(
  ({ catalog, voiceScope: _voiceScope, canClone, cloudSync, onCommit: _onCommit, onCancel: _onCancel }: VoiceCatalogModalViewProps) => {
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
    const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
    const [gender, setGender] = useState<string | null>(null);
    const [accent, setAccent] = useState<"english" | "multilingual" | null>(null);
    const [sort, setSort] = useState<Sort>("popular");
    const [isCloneDrawerOpen, setIsCloneDrawerOpen] = useState(false);
    const [isEditingPrompt, setIsEditingPrompt] = useState(false);
    const [customPromptInput, setCustomPromptInput] = useState(previewPrompt);
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

    // Stable snapshot of favorited voice IDs on mount/load. Prevents cards
    // from jumping to the top in real time when toggling heart, eliminating layout shifts.
    const initialFavoritesRef = useRef<Set<string>>(new Set());
    const initializedRef = useRef(false);

    useEffect(() => {
      if (!initializedRef.current && voices.length > 0) {
        initialFavoritesRef.current = new Set(
          voices.filter((v) => v.is_favorite).map((v) => v.id)
        );
        initializedRef.current = true;
      }
    }, [voices]);

    // Canonical smooth scroll (SSOT hook — inertial wheel glide, native touch
    // momentum). The inner content wrapper is the Lenis content element.
    const { containerRef, scrollToTop } =
      useLenisScrollContainer<HTMLDivElement>();
    const scrubberRef = useRef<HTMLDivElement>(null);

    // Reset scroll only once the debounced filter settles — never per keystroke.
    useEffect(() => {
      scrollToTop();
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [deferredSearch, scope, selectedProvider, gender, accent, sort]);

    const counts = useMemo(() => {
      const c = {
        all: voices.length,
        recent: 0,
        favorites: 0,
        custom: 0,
        female: 0,
        male: 0,
        neutral: 0,
        english: 0,
        multilingual: 0,
      };
      for (const v of voices) {
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
    }, [voices]);

    const filtered = useMemo(() => {
      const q = deferredSearch.trim().toLowerCase();
      let list = voices;
      if (scope === "recent") list = list.filter((v) => v.last_used_at != null);
      else if (scope === "favorites") list = list.filter((v) => v.is_favorite);
      else if (scope === "custom") list = list.filter((v) => v.source_kind === "custom");

      if (selectedProvider) list = list.filter((v) => v.provider_id === selectedProvider);
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
      if (sort === "popular") {
        sorted.sort((a, b) => {
          const isFavA = initialFavoritesRef.current.has(a.id);
          const isFavB = initialFavoritesRef.current.has(b.id);
          if (isFavA !== isFavB) return isFavA ? -1 : 1;
          const ra = a.last_used_at ?? 0;
          const rb = b.last_used_at ?? 0;
          if (ra !== rb) return rb - ra;
          return a.name.localeCompare(b.name);
        });
      } else {
        sorted.sort((a, b) => a.name.localeCompare(b.name));
      }
      return sorted;
    }, [voices, scope, selectedProvider, gender, accent, deferredSearch, sort]);

    const staged = voices.find((v) => v.id === stagedId) || voices[0] || null;
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
        if (initialFavoritesRef.current.has(id)) {
          initialFavoritesRef.current.delete(id);
        } else {
          initialFavoritesRef.current.add(id);
        }
        toggleFavorite(id);
      },
      [toggleFavorite]
    );

    const handleScrubberClick = useCallback(
      (e: React.MouseEvent<HTMLDivElement>) => {
        if (!scrubberRef.current) return;
        const rect = scrubberRef.current.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const fraction = Math.max(0, Math.min(1, clickX / rect.width));
        seekPreview(fraction);
      },
      [seekPreview]
    );

    const handleVoiceCreated = useCallback(
      (newVoice: UnifiedVoiceDto) => {
        reload();
        stageVoice(newVoice.id);
        togglePreview(newVoice);
      },
      [reload, stageVoice, togglePreview]
    );

    const handleSavePrompt = useCallback(() => {
      setPreviewPrompt(customPromptInput);
      setIsEditingPrompt(false);
      if (staged && stagedPlaying) {
        togglePreview(staged);
      }
    }, [customPromptInput, setPreviewPrompt, staged, stagedPlaying, togglePreview]);

    const handleResetFilters = useCallback(() => {
      setSearch("");
      setScope("all");
      setSelectedProvider(null);
      setGender(null);
      setAccent(null);
    }, []);

    const scopeFilters: Array<{ id: Scope; label: string; count: number; Icon: typeof Users }> = [
      { id: "all", label: VOICE_CATALOG_COPY.allVoices, count: counts.all, Icon: Users },
      { id: "favorites", label: VOICE_CATALOG_COPY.favorites, count: counts.favorites, Icon: Star },
      { id: "recent", label: VOICE_CATALOG_COPY.recent, count: counts.recent, Icon: Clock },
      ...(canClone
        ? [{ id: "custom" as Scope, label: VOICE_CATALOG_COPY.cloned, count: counts.custom, Icon: Sparkles }]
        : []),
    ];

    const waveformBars = useMemo(
      () => (staged ? getVoiceWaveform(staged.id, 46) : []),
      [staged]
    );

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
              <div className="absolute left-0 top-full mt-2 z-50 w-72 max-h-[75vh] overflow-y-auto rounded-2xl bg-[rgba(var(--card),0.96)] backdrop-blur-xl border border-[rgba(var(--foreground),0.12)] shadow-2xl p-3.5 flex flex-col gap-3 custom-scrollbar animate-fade-in">
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
            onClick={() => setSort((s) => (s === "popular" ? "az" : "popular"))}
            className="px-2.5 sm:px-3 py-1.5 rounded-xl text-[12px] font-medium bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] hover:border-[rgb(var(--accent))]/40 flex items-center gap-1.5 cursor-pointer shrink-0 transition-all"
            title={sort === "popular" ? "Sort: Popular first" : "Sort: Alphabetical (A–Z)"}
          >
            <ArrowUpDown size={13} className="text-[rgb(var(--accent))]" />
            <span className="font-sans font-medium hidden sm:inline">{sort === "popular" ? VOICE_CATALOG_COPY.sortPopular : "A–Z"}</span>
          </button>

          <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0">
            <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{filtered.length}</span> of {voices.length} {VOICE_CATALOG_COPY.showingOf}</span>
            <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{filtered.length}</span>/{voices.length}</span>
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
                    previewDisabled={!v.has_preview && sessionBusy}
                    onSelect={handleCardSelect}
                    onToggleFavorite={handleCardFavorite}
                    onPreview={handleCardPreview}
                  />
                ))}
              </div>
            )}
            </div>
          </div>
        </div>

        {/* Longer, Spacious Bottom Preview Banner */}
        {staged && (
          <div className="shrink-0 rounded-2xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--card),0.75)] backdrop-blur-xl p-4 sm:p-5 flex flex-col md:flex-row items-center gap-5 sm:gap-6 shadow-lg">
            {/* Left: Prominent Circular Play Sphere with accent glow */}
            <button
              type="button"
              disabled={!staged.has_preview && sessionBusy}
              onClick={() => togglePreview(staged)}
              aria-label={stagedPlaying ? "Pause preview" : `Preview ${staged.name}`}
              className={cn(
                "w-16 h-16 rounded-full flex items-center justify-center shrink-0 transition-all shadow-md",
                !staged.has_preview && sessionBusy
                  ? "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/30 cursor-not-allowed"
                  : stagedPlaying
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-[0_0_24px_rgba(var(--accent),0.45)] scale-105"
                    : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] hover:bg-[rgb(var(--accent))] hover:text-[rgb(var(--accent-foreground))] hover:shadow-[0_0_16px_rgba(var(--accent),0.3)] hover:scale-105 active:scale-95 cursor-pointer"
              )}
            >
              {isSynthesizing && stagedPlaying ? (
                <Loader2 size={24} className="animate-spin" />
              ) : stagedPlaying ? (
                <Pause size={24} />
              ) : (
                <Play size={24} className="ml-1" />
              )}
            </button>

            {/* Middle: Voice Title, Description, Meta Tags, and Waveform Scrubber */}
            <div className="flex-1 min-w-0 flex flex-col gap-2.5 w-full">
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-baseline gap-2.5 min-w-0">
                  <span className="font-bold text-[17px] text-[rgb(var(--foreground))] truncate font-sans tracking-tight">
                    {staged.name}
                  </span>
                  <span className="text-[13px] text-[rgb(var(--foreground-muted))] truncate hidden sm:inline leading-relaxed">
                    {staged.description || VOICE_CATALOG_COPY.fallbackDescription}
                  </span>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  {staged.source_kind !== "custom" && (
                    <button
                      type="button"
                      onClick={() => {
                        setCustomPromptInput(previewPrompt);
                        setIsEditingPrompt(true);
                      }}
                      title="Customize sample sentence"
                      className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--foreground),0.04)] transition-all cursor-pointer"
                    >
                      <Edit3 size={14} />
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => handleCardFavorite(staged.id)}
                    aria-label={staged.is_favorite ? "Remove favorite" : "Add favorite"}
                    className={cn(
                      "p-1.5 rounded-lg transition-transform hover:scale-110 cursor-pointer",
                      staged.is_favorite
                        ? "text-[rgb(var(--accent))]"
                        : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--accent))]"
                    )}
                  >
                    <Heart size={16} fill={staged.is_favorite ? "currentColor" : "none"} strokeWidth={staged.is_favorite ? 2 : 1.75} />
                  </button>
                </div>
              </div>

              {/* Tag Badges Row */}
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-full bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.06)] leading-none whitespace-nowrap">
                  {tagLabel(staged.gender)}
                </span>
                <span className="text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-full bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] border border-[rgba(var(--foreground),0.06)] leading-none whitespace-nowrap">
                  {staged.accent || staged.locale}
                </span>
                <span className="text-[11px] font-medium font-sans px-2.5 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.18)] leading-none whitespace-nowrap">
                  {tagLabel(staged.tags.find((t) => t.toLowerCase() !== "embedded" && t.toLowerCase() !== "fast") || "Natural")}
                </span>
              </div>

              {isEditingPrompt && (
                <div className="flex items-center gap-2 animate-fade-in py-1">
                  <input
                    type="text"
                    value={customPromptInput}
                    onChange={(e) => setCustomPromptInput(e.target.value)}
                    placeholder="Type phrase to synthesize..."
                    maxLength={120}
                    autoFocus
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSavePrompt();
                      else if (e.key === "Escape") setIsEditingPrompt(false);
                    }}
                    className="flex-1 min-w-0 bg-transparent border-b border-[rgb(var(--accent))] text-[12.5px] text-[rgb(var(--foreground))] outline-none py-1"
                  />
                  <button
                    type="button"
                    onClick={handleSavePrompt}
                    className="px-2.5 py-1 text-[11px] font-bold rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] cursor-pointer"
                  >
                    Apply
                  </button>
                  <button
                    type="button"
                    onClick={() => setIsEditingPrompt(false)}
                    className="p-1 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer"
                  >
                    <X size={14} />
                  </button>
                </div>
              )}

              {/* Waveform Scrubber: skeleton gray when no preview, maps to voice/prompt when preview exists */}
              <div className="flex items-center gap-3 w-full">
                <span className="font-mono text-[11px] tabular-nums text-[rgb(var(--foreground-muted))] w-9 text-right shrink-0">
                  {formatTime(previewCurrentTime)}
                </span>

                <div
                  ref={scrubberRef}
                  onClick={staged.has_preview || !sessionBusy ? handleScrubberClick : undefined}
                  className={cn(
                    "flex-1 h-8 flex items-center gap-[3px] px-1 rounded-lg transition-all select-none",
                    !staged.has_preview && !stagedPlaying
                      ? "opacity-35 cursor-not-allowed"
                      : "cursor-pointer hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                  title={!staged.has_preview && !stagedPlaying ? "No preview synthesized yet" : "Click to seek preview"}
                >
                  {waveformBars.map((height, i) => {
                    const barProgress = i / waveformBars.length;
                    const isPlayed = previewProgress > 0 && barProgress <= previewProgress;
                    const isSkeleton = !staged.has_preview && !stagedPlaying;
                    return (
                      <div
                        key={i}
                        style={{ height: `${Math.round(isSkeleton ? 26 : height * 100)}%` }}
                        className={cn(
                          "flex-1 rounded-full min-w-[2px] max-w-[4px] transition-all duration-75",
                          isSkeleton
                            ? "bg-[rgba(var(--foreground),0.2)]"
                            : isPlayed
                              ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.4)]"
                              : "bg-[rgba(var(--foreground),0.18)] hover:bg-[rgba(var(--foreground),0.35)]"
                        )}
                      />
                    );
                  })}
                </div>

                <span className="font-mono text-[11px] tabular-nums text-[rgb(var(--foreground-muted))] w-9 shrink-0">
                  {formatTime(previewDuration)}
                </span>
              </div>
            </div>

            {/* Right: Rich Metadata Indicators (Language, Gender, Style) with vertical divider */}
            <div className="hidden lg:flex items-center gap-6 border-l border-[rgba(var(--foreground),0.08)] pl-5 shrink-0 text-[12.5px]">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-full bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Globe size={16} />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase leading-none">Language</span>
                  <span className="font-medium text-[rgb(var(--foreground))] leading-tight mt-1">{staged.locale || "English"}</span>
                </div>
              </div>

              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-full bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Users size={16} />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase leading-none">Gender</span>
                  <span className="font-medium text-[rgb(var(--foreground))] capitalize leading-tight mt-1">{tagLabel(staged.gender) || "Neutral"}</span>
                </div>
              </div>

              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-full bg-[rgba(var(--foreground),0.04)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                  <Sparkles size={16} />
                </div>
                <div className="flex flex-col">
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 uppercase leading-none">Style</span>
                  <span className="font-medium text-[rgb(var(--foreground))] capitalize leading-tight mt-1">
                    {tagLabel(staged.tags.find((t) => t.toLowerCase() !== "embedded" && t.toLowerCase() !== "fast") || staged.accent || "Natural")}
                  </span>
                </div>
              </div>
            </div>
          </div>
        )}

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
