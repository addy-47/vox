import {
  memo,
  useCallback,
  useDeferredValue,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  Check,
  Clock,
  Edit3,
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
  onCommit: (id: string) => void;
  onCancel: () => void;
}

type Scope = "all" | "recent" | "favorites" | "custom";
type Sort = "popular" | "az";

function isEnglish(v: UnifiedVoiceDto): boolean {
  return v.locale.toLowerCase().startsWith("en");
}

function popularity(a: UnifiedVoiceDto, b: UnifiedVoiceDto): number {
  if (a.is_favorite !== b.is_favorite) return a.is_favorite ? -1 : 1;
  const ra = a.last_used_at ?? 0;
  const rb = b.last_used_at ?? 0;
  if (ra !== rb) return rb - ra;
  return a.name.localeCompare(b.name);
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const VoiceCatalogModalView = memo(
  ({ catalog, onCommit, onCancel }: VoiceCatalogModalViewProps) => {
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
    const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
    const [gender, setGender] = useState<string | null>(null);
    const [accent, setAccent] = useState<"english" | "multilingual" | null>(null);
    const [sort, setSort] = useState<Sort>("popular");
    const [isCloneDrawerOpen, setIsCloneDrawerOpen] = useState(false);
    const [isEditingPrompt, setIsEditingPrompt] = useState(false);
    const [customPromptInput, setCustomPromptInput] = useState(previewPrompt);

    const { containerRef } = useLenisScrollContainer<HTMLDivElement>();
    const scrubberRef = useRef<HTMLDivElement>(null);

    const counts = useMemo(() => {
      const c = {
        all: voices.length,
        recent: 0,
        favorites: 0,
        custom: 0,
        female: 0,
        male: 0,
        english: 0,
        multilingual: 0,
      };
      for (const v of voices) {
        if (v.last_used_at != null) c.recent += 1;
        if (v.is_favorite) c.favorites += 1;
        if (v.source_kind === "custom") c.custom += 1;
        if (v.gender === "female") c.female += 1;
        else if (v.gender === "male") c.male += 1;
        if (isEnglish(v)) c.english += 1;
        else c.multilingual += 1;
      }
      return c;
    }, [voices]);

    const providerList = useMemo(() => {
      const set = new Set<string>();
      for (const v of voices) {
        if (v.provider_id) set.add(v.provider_id);
      }
      return Array.from(set).sort();
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
      if (sort === "popular") sorted.sort(popularity);
      else sorted.sort((a, b) => a.name.localeCompare(b.name));
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
      (id: string) => toggleFavorite(id),
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

    const scopeTabs = [
      { id: "all", label: "All Voices", count: counts.all, Icon: Users },
      { id: "favorites", label: "Favorites", count: counts.favorites, Icon: Star },
      { id: "recent", label: "Recent", count: counts.recent, Icon: Clock },
      { id: "custom", label: "Cloned", count: counts.custom, Icon: Sparkles },
    ] as const;

    return (
      <div className="flex flex-col h-full min-h-0 gap-3">
        {/* Top Control Bar */}
        <div className="flex items-center gap-2.5 shrink-0 flex-wrap">
          {/* Search Bar */}
          <div className="flex-1 min-w-[200px] flex items-center gap-2 px-3 py-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
            <Search size={15} className="text-[rgb(var(--foreground-muted))] shrink-0" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search voices by name, accent, tag, or locale..."
              aria-label="Search voices"
              className="w-full min-w-0 bg-transparent border-none outline-none text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            {search && (
              <button
                type="button"
                onClick={() => setSearch("")}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer shrink-0"
              >
                <X size={14} />
              </button>
            )}
          </div>

          {/* Scope Filter Tabs */}
          <div className="flex items-center gap-1 p-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0">
            {scopeTabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setScope(tab.id)}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-[12px] font-medium transition-all cursor-pointer flex items-center gap-1.5",
                  scope === tab.id
                    ? "bg-[rgb(var(--card))] text-[rgb(var(--foreground))] font-bold shadow-xs border border-[rgba(var(--foreground),0.08)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.02)]"
                )}
              >
                <tab.Icon size={13} className="shrink-0" />
                <span>{tab.label}</span>
                <span className="text-[10px] font-mono opacity-70">({tab.count})</span>
              </button>
            ))}
          </div>

          {/* Sort Selector */}
          <div className="flex items-center gap-1 px-1 py-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0">
            {(["popular", "az"] as const).map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setSort(s)}
                className={cn(
                  "px-2.5 py-1 rounded-lg text-[11.5px] font-mono transition-all cursor-pointer",
                  sort === s
                    ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                {s === "popular" ? "Popular" : "A–Z"}
              </button>
            ))}
          </div>

          {/* Clone Voice Header Button */}
          <button
            type="button"
            onClick={() => setIsCloneDrawerOpen(true)}
            className="px-3.5 py-2 rounded-xl text-[12.5px] font-bold bg-[rgb(var(--accent))]/15 border border-[rgb(var(--accent))]/40 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
          >
            <Plus size={15} strokeWidth={2.5} />
            <span>Clone Voice</span>
          </button>
        </div>

        {/* Main Content Area */}
        <div className="flex-1 min-h-0 flex gap-3.5 overflow-hidden">
          {/* Left Facet Sidebar */}
          <div className="w-[170px] shrink-0 hidden md:flex flex-col gap-2 overflow-y-auto custom-scrollbar pr-1">
            {/* Providers Facet */}
            <div className="px-2 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 font-semibold">
              Providers
            </div>
            <button
              type="button"
              onClick={() => setSelectedProvider(null)}
              className={cn(
                "w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12.5px] transition-all cursor-pointer",
                selectedProvider === null
                  ? "bg-[rgb(var(--accent))]/12 text-[rgb(var(--accent))] font-bold"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
              )}
            >
              <span>All Providers</span>
              <span className="text-[10px] font-mono opacity-70">{counts.all}</span>
            </button>
            {providerList.map((p) => {
              const count = voices.filter((v) => v.provider_id === p).length;
              return (
                <button
                  key={p}
                  type="button"
                  onClick={() =>
                    setSelectedProvider(selectedProvider === p ? null : p)
                  }
                  className={cn(
                    "w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12.5px] capitalize transition-all cursor-pointer",
                    selectedProvider === p
                      ? "bg-[rgb(var(--accent))]/12 text-[rgb(var(--accent))] font-bold"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  <span className="truncate">{p.replace("_", " ")}</span>
                  <span className="text-[10px] font-mono opacity-70">{count}</span>
                </button>
              );
            })}

            {/* Gender Facet (Female & Male only) */}
            <div className="mt-3 px-2 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 font-semibold">
              Gender
            </div>
            {(
              [
                { id: "female", label: "Female", count: counts.female },
                { id: "male", label: "Male", count: counts.male },
              ] as const
            ).map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setGender(gender === item.id ? null : item.id)}
                className={cn(
                  "w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12.5px] transition-all cursor-pointer",
                  gender === item.id
                    ? "bg-[rgb(var(--accent))]/12 text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                )}
              >
                <span>{item.label}</span>
                <span className="text-[10px] font-mono opacity-70">{item.count}</span>
              </button>
            ))}

            {/* Language & Accent Facet */}
            <div className="mt-3 px-2 text-[10.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 font-semibold">
              Language
            </div>
            {(
              [
                { id: "english", label: "English", count: counts.english },
                { id: "multilingual", label: "Multilingual", count: counts.multilingual },
              ] as const
            ).map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setAccent(accent === item.id ? null : item.id)}
                className={cn(
                  "w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-[12.5px] transition-all cursor-pointer",
                  accent === item.id
                    ? "bg-[rgb(var(--accent))]/12 text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                )}
              >
                <div className="flex items-center gap-1.5 truncate">
                  <Languages size={12} className="opacity-60 shrink-0" />
                  <span className="truncate">{item.label}</span>
                </div>
                <span className="text-[10px] font-mono opacity-70">{item.count}</span>
              </button>
            ))}
          </div>

          {/* Voice Cards Grid with Lenis Dynamic Scroll */}
          <div
            ref={containerRef}
            className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar pr-1.5"
          >
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
                  {VOICE_CATALOG_COPY.emptyHint}
                </p>
                <button
                  type="button"
                  onClick={() => {
                    setSearch("");
                    setScope("all");
                    setSelectedProvider(null);
                    setGender(null);
                    setAccent(null);
                  }}
                  className="mt-2 px-3.5 py-1.5 rounded-xl text-[12.5px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
                >
                  {VOICE_CATALOG_COPY.resetFilters}
                </button>
              </div>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3">
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

                {/* Trailing Custom Clone Card Skeleton */}
                <VoiceCard
                  isCloneSkeleton={true}
                  onCloneClick={() => setIsCloneDrawerOpen(true)}
                />
              </div>
            )}
          </div>
        </div>

        {/* Sticky Bottom Preview & Action Bar */}
        {staged && (
          <div className="shrink-0 rounded-2xl border border-[rgba(var(--foreground),0.1)] bg-[rgb(var(--card))] shadow-xl p-3.5 flex flex-col gap-2.5">
            {/* Top row: controls + voice info + metadata chips */}
            <div className="flex items-center gap-3.5 min-w-0">
              {/* Play / Pause button */}
              <button
                type="button"
                disabled={!staged.has_preview && sessionBusy}
                onClick={() => togglePreview(staged)}
                aria-label={stagedPlaying ? "Pause preview" : `Preview ${staged.name}`}
                className={cn(
                  "w-11 h-11 rounded-full flex items-center justify-center shrink-0 transition-all",
                  !staged.has_preview && sessionBusy
                    ? "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
                    : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 active:scale-95 shadow-md cursor-pointer"
                )}
              >
                {isSynthesizing && stagedPlaying ? (
                  <Loader2 size={18} className="animate-spin" />
                ) : stagedPlaying ? (
                  <Pause size={18} />
                ) : (
                  <Play size={18} className="ml-0.5" />
                )}
              </button>

              {/* Voice Name & Sample Description */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-bold text-[14.5px] text-[rgb(var(--foreground))] truncate">
                    {staged.name}
                  </span>
                  {staged.is_favorite && (
                    <Heart
                      size={13}
                      className="text-[rgb(var(--accent))] shrink-0"
                      fill="currentColor"
                    />
                  )}
                  <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] uppercase border border-[rgba(var(--foreground),0.08)]">
                    {staged.provider_id}
                  </span>
                </div>
                <div className="flex items-center gap-2 mt-0.5">
                  <p className="text-[11.5px] text-[rgb(var(--foreground-muted))]/80 truncate">
                    {staged.source_kind === "custom" ? (
                      <span className="text-[rgb(var(--foreground-muted))] italic">
                        Recorded reference speech sample
                      </span>
                    ) : isEditingPrompt ? (
                      <span className="text-[rgb(var(--accent))] font-medium">
                        Custom prompt active
                      </span>
                    ) : (
                      previewPrompt || staged.description || VOICE_CATALOG_COPY.fallbackDescription
                    )}
                  </p>
                  {staged.source_kind !== "custom" && (
                    <button
                      type="button"
                      onClick={() => setIsEditingPrompt(!isEditingPrompt)}
                      title="Customize preview sample text"
                      className="p-1 rounded text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                    >
                      <Edit3 size={12} />
                    </button>
                  )}
                </div>
              </div>

              {/* Metadata chips */}
              <div className="hidden sm:flex items-center gap-4 shrink-0 text-[11.5px] px-2 py-1 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)]">
                <span className="flex flex-col">
                  <span className="text-[rgb(var(--foreground-muted))]/60 font-mono uppercase text-[9px]">
                    Language
                  </span>
                  <span className="font-bold text-[rgb(var(--foreground))]">
                    {staged.locale}
                  </span>
                </span>
                <span className="flex flex-col capitalize">
                  <span className="text-[rgb(var(--foreground-muted))]/60 font-mono uppercase text-[9px]">
                    Gender
                  </span>
                  <span className="font-bold text-[rgb(var(--foreground))]">
                    {staged.gender}
                  </span>
                </span>
                <span className="flex flex-col">
                  <span className="text-[rgb(var(--foreground-muted))]/60 font-mono uppercase text-[9px]">
                    Style
                  </span>
                  <span className="font-bold text-[rgb(var(--foreground))]">
                    {staged.tags[0] || staged.accent || "Standard"}
                  </span>
                </span>
              </div>
            </div>

            {/* Prompt edit box if opened */}
            {isEditingPrompt && (
              <div className="flex items-center gap-2 p-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] animate-fade-in">
                <input
                  type="text"
                  value={customPromptInput}
                  onChange={(e) => setCustomPromptInput(e.target.value)}
                  placeholder="Type any phrase for the voice to preview (max 120 chars)..."
                  maxLength={120}
                  className="flex-1 bg-transparent border-none outline-none text-[12.5px] text-[rgb(var(--foreground))]"
                />
                <button
                  type="button"
                  onClick={handleSavePrompt}
                  className="px-3 py-1 rounded-lg text-[11.5px] font-bold bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] cursor-pointer hover:brightness-110"
                >
                  Apply & Hear
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

            {/* Middle row: Interactive scrubber with live time */}
            <div className="flex items-center gap-3">
              <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))] w-10 text-right shrink-0">
                {formatTime(previewCurrentTime)}
              </span>

              <div
                ref={scrubberRef}
                onClick={handleScrubberClick}
                className="flex-1 h-3 rounded-full bg-[rgba(var(--foreground),0.08)] hover:bg-[rgba(var(--foreground),0.12)] relative cursor-pointer flex items-center transition-all group"
              >
                <div
                  style={{ width: `${Math.round(previewProgress * 100)}%` }}
                  className="h-full rounded-full bg-[rgb(var(--accent))] transition-all relative"
                >
                  <span className="absolute right-0 top-1/2 -translate-y-1/2 w-3.5 h-3.5 rounded-full bg-white shadow-md scale-0 group-hover:scale-100 transition-transform" />
                </div>
              </div>

              <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))] w-10 shrink-0">
                {formatTime(previewDuration)}
              </span>
            </div>

            {/* Bottom action row: Discard vs Select */}
            <div className="flex items-center justify-end gap-2.5 pt-1">
              <button
                type="button"
                onClick={onCancel}
                className="px-4 py-1.5 rounded-xl text-[12.5px] font-bold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] border border-[rgba(var(--foreground),0.1)] hover:border-[rgba(var(--foreground),0.2)] transition-all cursor-pointer"
              >
                {VOICE_CATALOG_COPY.cancel}
              </button>
              <button
                type="button"
                onClick={() => {
                  commitVoice(staged.id);
                  onCommit(staged.id);
                }}
                className="px-5 py-1.5 rounded-xl text-[12.5px] font-bold bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 active:scale-95 transition-all cursor-pointer flex items-center gap-1.5 shadow-md"
              >
                <Check size={14} strokeWidth={2.5} />
                <span>{VOICE_CATALOG_COPY.selectVoice}</span>
              </button>
            </div>
          </div>
        )}

        {/* Clone Voice Modal Drawer */}
        <CloneVoiceDrawer
          isOpen={isCloneDrawerOpen}
          onClose={() => setIsCloneDrawerOpen(false)}
          onCreated={handleVoiceCreated}
        />
      </div>
    );
  }
);

VoiceCatalogModalView.displayName = "VoiceCatalogModalView";
