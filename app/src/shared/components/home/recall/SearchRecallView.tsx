import { memo, useRef, useEffect, useState, useCallback } from "react";
import { Search, ChevronDown, Check, X } from "lucide-react";
import { motion } from "framer-motion";
import { RecallArcCarousel } from "./RecallArcCarousel";
import { RecallTimelineScrubber } from "./RecallTimelineScrubber";
import { RecallSearchModal } from "./RecallSearchModal";
import { DetailPanel } from "./DetailPanel";
import { useRecallSearch, type SortOrder } from "./useRecallSearch";
import { type SessionRow } from "@/services/persistenceService";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { SESSION_COPY } from "@/data/sessionCopy";
import { isRealMobileDevice } from "@/layout/breakpoints";

export interface SearchRecallViewProps {
  onBackToVoice: () => void;
  /** Optional pre-selected session id (e.g. from Shift+Up or URL) */
  initialSessionId?: number | null;
  /** Optional explicit mobile override (matches Home.tsx isMobileScreen) */
  isMobile?: boolean;
}

export const SearchRecallView = memo(({
  onBackToVoice,
  initialSessionId,
  isMobile: explicitIsMobile,
}: SearchRecallViewProps) => {
  const {
    sessions,
    filteredSessions,
    carouselItems,
    searchQuery,
    setSearchQuery,
    sortOrder,
    setSortOrder,
    activeIndex,
    selectIndex,
    nextCard,
    prevCard,
    selectedSession,
    setSelectedSession,
    openSessionDetail,
    closeSessionDetail,
    turns,
    turnsLoading,
    turnsError,
    retryFetchTurns,
    handleDeleteSession,
    projectNameMap,
  } = useRecallSearch();

  const { isMobile: hookIsMobile } = useViewportResize();
  const isMobile = explicitIsMobile ?? hookIsMobile;

  const searchInputRef = useRef<HTMLInputElement>(null);
  const sortMenuRef = useRef<HTMLDivElement>(null);
  const [sortMenuOpen, setSortMenuOpen] = useState(false);

  useOverlay({
    active: sortMenuOpen,
    onClose: () => setSortMenuOpen(false),
    ref: sortMenuRef,
    dismissOnOutside: true,
  });

  // If initialSessionId is provided, select it once data loads
  useEffect(() => {
    if (!initialSessionId || sessions.length === 0) return;
    const found = sessions.find((s) => s.id === initialSessionId);
    if (found) {
      setSelectedSession(found);
    }
  }, [initialSessionId, sessions, setSelectedSession]);

  // Keyboard shortcuts: '/' focuses search, Shift+Up / Enter opens center card, Left/Right navigates
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      const tag = activeEl?.tagName.toLowerCase();
      const isInput = tag === "input" || tag === "textarea" || activeEl?.getAttribute("contenteditable") === "true";

      if (e.key === "/" && !isInput && !selectedSession) {
        e.preventDefault();
        searchInputRef.current?.focus();
        return;
      }

      if (isInput) return;

      if ((e.shiftKey && e.key === "ArrowUp") || e.key === "Enter") {
        e.preventDefault();
        const activeCard = carouselItems[activeIndex] as SessionRow | undefined;
        if (activeCard) {
          openSessionDetail(activeCard);
        }
        return;
      }

      if (e.key === "ArrowLeft") {
        e.preventDefault();
        prevCard();
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        nextCard();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [prevCard, nextCard, selectedSession, carouselItems, activeIndex, openSessionDetail]);

  // Register Search Input with overlay stack so Escape clears/blurs it first
  const [isSearchFocused, setIsSearchFocused] = useState(false);
  const handleSearchEscape = useCallback(() => {
    if (searchQuery) {
      setSearchQuery("");
      return;
    }
    searchInputRef.current?.blur();
    setIsSearchFocused(false);
  }, [searchQuery, setSearchQuery]);

  useOverlay({
    onClose: handleSearchEscape,
    active: isSearchFocused,
  });

  useEffect(() => {
    const t0 = performance.now();
    console.info(
      `[trace-search] SearchRecallView mounted @${t0.toFixed(1)}ms. Sessions: ${sessions.length}, Filtered: ${filteredSessions.length}`
    );
    return () => {
      console.info(`[trace-search] SearchRecallView unmounted @${performance.now().toFixed(1)}ms`);
    };
  }, [sessions.length, filteredSessions.length]);

  if (isMobile) {
    return (
      <>
        <RecallSearchModal
          open={true}
          onClose={onBackToVoice}
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          sortOrder={sortOrder}
          setSortOrder={setSortOrder}
          sessions={filteredSessions}
          totalCount={sessions.length}
          onSelectSession={openSessionDetail}
          projectNameMap={projectNameMap}
        />
        {/* Transcript Detail Drawer */}
        <DetailPanel
          open={Boolean(selectedSession)}
          session={selectedSession}
          turns={turns}
          loading={turnsLoading}
          error={turnsError}
          onRetry={retryFetchTurns}
          onClose={closeSessionDetail}
        />
      </>
    );
  }

  return (
    <div className="absolute inset-0 z-30 w-full h-full pointer-events-none select-none">
      {/* ── Top Header Controls (Glides down smoothly directly above the Orb, matching DynamicStatusBadge) ── */}
      <motion.div
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -16 }}
        transition={{ type: "spring", stiffness: 320, damping: 28, mass: 0.8 }}
        className="absolute left-1/2 -translate-x-1/2 z-40 w-full max-w-md px-4 flex flex-col items-center gap-1.5 pointer-events-auto"
        style={{
          top: isRealMobileDevice()
            ? "calc(50% - min(41vw, 25vh) - 112px - 22.4px)"
            : "clamp(36px, 8.5vh, 68px)",
        }}
      >
        {/* Floating Frosted Glass Search Capsule */}
        <div className="relative w-full max-w-md">
          <div className="flex items-center gap-2.5 px-3.5 py-2 rounded-full border border-[rgba(var(--border),0.07)] bg-[rgba(var(--card),0.65)] backdrop-blur-xl shadow-[0_4px_20px_rgba(0,0,0,0.15),inset_0_1px_0_rgba(255,255,255,0.06)] focus-within:border-[rgba(var(--accent),0.6)] focus-within:shadow-[0_0_24px_rgba(var(--accent),0.20)] transition-all duration-200 w-full shrink-0">
            <Search size={14} className="text-[rgb(var(--accent))] shrink-0 opacity-80" />
            <input
              ref={searchInputRef}
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onFocus={() => setIsSearchFocused(true)}
              onBlur={() => setIsSearchFocused(false)}
              placeholder={SESSION_COPY.searchConversationsPlaceholder}
              className="flex-1 min-w-0 bg-transparent text-[12.5px] font-sans text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
            />
            {searchQuery ? (
              <button
                type="button"
                onClick={() => setSearchQuery("")}
                className="p-1 rounded-full text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer shrink-0 transition-colors"
                aria-label="Clear search"
              >
                <X size={12} />
              </button>
            ) : (
              <kbd className="shrink-0 px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.10)] text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 leading-none select-none">
                /
              </kbd>
            )}
          </div>

          {/* Subtitle: Count & Sort options */}
          <div className="flex items-center justify-center gap-2 mt-1.5 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]/70">
            <span>
              {filteredSessions.length} {filteredSessions.length === 1 ? "conversation" : "conversations"}
            </span>
            <span>·</span>

            {/* Sort Dropdown */}
            <div ref={sortMenuRef} className="relative">
              <button
                type="button"
                onClick={() => setSortMenuOpen((prev) => !prev)}
                className="flex items-center gap-1 text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
              >
                <span>
                  {sortOrder === "newest"
                    ? SESSION_COPY.sortNewest
                    : sortOrder === "oldest"
                    ? SESSION_COPY.sortOldest
                    : sortOrder === "turns"
                    ? SESSION_COPY.sortTurns
                    : SESSION_COPY.sortUncompacted}
                </span>
                <ChevronDown size={12} />
              </button>

              {sortMenuOpen && (
                <div className="absolute top-full mt-1.5 left-1/2 -translate-x-1/2 rounded-xl glass-card border border-[rgba(var(--border),0.15)] bg-[rgb(var(--card))]/95 shadow-xl backdrop-blur-md py-1 min-w-[130px] z-50 animate-in fade-in zoom-in-95">
                  {(["newest", "oldest", "turns", "uncompacted"] as SortOrder[]).map((option) => (
                    <button
                      key={option}
                      type="button"
                      onClick={() => {
                        setSortOrder(option);
                        setSortMenuOpen(false);
                      }}
                      className="w-full flex items-center justify-between px-3 py-1.5 text-left text-[12px] text-[rgb(var(--foreground))] hover:bg-[rgba(var(--accent),0.08)] cursor-pointer"
                    >
                      <span>
                        {option === "newest"
                          ? SESSION_COPY.sortNewest
                          : option === "oldest"
                          ? SESSION_COPY.sortOldest
                          : option === "turns"
                          ? SESSION_COPY.sortTurns
                          : SESSION_COPY.sortUncompacted}
                      </span>
                      {sortOrder === option && <Check size={12} className="text-[rgb(var(--accent))]" />}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      </motion.div>

      {/* ── Middle Stage: Arc Carousel positioned to overlap bottom third of Orb ── */}
      <motion.div
        initial={{ opacity: 0, y: 60, scale: 0.96 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 50, scale: 0.96 }}
        transition={{ type: "spring", stiffness: 300, damping: 28, mass: 0.85 }}
        className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 flex items-center justify-center pointer-events-auto z-20 w-full"
        style={{
          top: "calc(50% + clamp(48px, 6.5vh, 66px))",
        }}
      >
        {carouselItems.length > 0 ? (
          <RecallArcCarousel
            items={carouselItems}
            activeIndex={activeIndex}
            projectNameMap={projectNameMap}
            onSelectIndex={selectIndex}
            onLiveIndexChange={selectIndex}
            onOpenSession={openSessionDetail}
            onDeleteSession={handleDeleteSession}
          />
        ) : (
          <div className="flex flex-col items-center justify-center p-8 text-center text-[rgb(var(--foreground-muted))]">
            <p className="text-[13px]">{SESSION_COPY.noMatchingConversations}</p>
          </div>
        )}
      </motion.div>

      {/* ── Bottom Stage: Timeline Scrubber (65–75% width, positioned in golden pocket between cards & dock) + Back to Voice Mode ── */}
      <motion.div
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: 20 }}
        transition={{ type: "spring", stiffness: 320, damping: 28, mass: 0.8 }}
        className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 z-30 flex flex-col items-center w-[72%] max-w-4xl min-w-[320px] px-4 pointer-events-auto"
        style={{
          top: isRealMobileDevice()
            ? "calc(74% - 30px + min(82vw, 50vh, 580px) * 0.10)"
            : "calc(77% - 30px + min(65vw, 56vh, 580px) * 0.10)",
        }}
      >
        <RecallTimelineScrubber
          items={carouselItems}
          activeIndex={activeIndex}
          onSelectIndex={selectIndex}
        />

        {/* Subtle Back to Voice Mode text button with Esc keyboard hint */}
        <button
          type="button"
          onClick={onBackToVoice}
          className="mt-3.5 flex items-center gap-1.5 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer select-none group pt-5"
        >
          <span>← {SESSION_COPY.backToVoiceMode}</span>
          <kbd className="px-1.5 py-0.5 rounded border border-[rgba(var(--foreground),0.12)] bg-[rgba(var(--foreground),0.04)] text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 group-hover:text-[rgb(var(--foreground-muted))] transition-colors">
            Esc
          </kbd>
        </button>
      </motion.div>

      {/* ── Transcript Detail Drawer ── */}
      <DetailPanel
        open={Boolean(selectedSession)}
        session={selectedSession}
        turns={turns}
        loading={turnsLoading}
        error={turnsError}
        onClose={closeSessionDetail}
        onRetry={retryFetchTurns}
      />
    </div>
  );
});

SearchRecallView.displayName = "SearchRecallView";
