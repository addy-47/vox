import React from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Ghost, X, AlertCircle, RotateCcw, Hand, ArrowLeft } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import {
  VoiceRippleNode,
  DetailPanel,
  CentralClockNode,
  OrbitCarousel,
  MonthDayCard,
  HistoryListView,
  HistorySearchBar,
} from "@/shared/components/history";
import { useHistory } from "@/shared/hooks/useHistory";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { BREAKPOINT_OPPOSITE_COLLISION_MAX } from "@/layout/breakpoints";
import { EmptyState, OrbitalLoader, ErrorBoundary } from "@/shared/components/common";
import { ThemeToggleButton, BottomDockFeather } from "@/shared/ui";
import { HISTORY_COPY } from "@/data/historyCopy";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { formatDateTime, resolveSessionTitle, type SessionRow } from "@/services/historyService";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { getThemeTransitioning, subscribeThemeTransition } from "@/shared/theme";

export const History: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const setDisplayMode = useHistoryFilterStore((s) => s.setDisplayMode);
  const setDrillDownSession = useHistoryFilterStore((s) => s.setDrillDownSession);
  const { isPanelOpen, rightPanel } = usePanelStateContext();
  const { isProfilerOpen } = useProfilerDrawer();
  const [isThemeTransitioning, setIsThemeTransitioning] = React.useState(getThemeTransitioning);
  React.useEffect(() => {
    return subscribeThemeTransition(() => {
      setIsThemeTransitioning(getThemeTransitioning());
    });
  }, []);
  const {
    sessions,
    showLoading,
    error,
    selectedSession,
    setSelectedSession,
    turns,
    turnsLoading,
    turnsError,
    deleteError,
    setDeleteError,
    confirmDeleteId,
    view,
    currentGroup,
    currentMonthGroup,
    currentWindow,
    currentWindowSessions,
    currentMonthWindow,
    sessionById,
    ringRadius,
    dayWindowIndex,
    monthWindowIndex,
    dayWindows,
    monthWindows,
    totalMonths,
    monthIndex,
    isOrbitViewport,
    effectiveView,
    dayMetaLabel,
    monthMetaLabel,
    dayTurnsCount,
    monthTurnsCount,
    dayTimeSpan,
    hintText,
    containerRef,
    loadSessions,
    handleViewChange,
    handleDrillIntoDay,
    retryFetchTurns,
    handleDelete,
    handleCancelDelete,
    handlePrevDate,
    handleNextDate,
    handlePrevMonth,
    handleNextMonth,
    handleDragState,
    handleStageClick,
    formatDayHeroLabel,
    formatDayHeroParts,
    formatWeekdayLabel,
    formatMonthFullLabel,
    formatDayYearLabel,
    formatMonthHeroLabel,
    formatMonthYearLabel,
  } = useHistory();

  React.useEffect(() => {
    setDisplayMode(isOrbitViewport ? "orbit" : "list");
  }, [isOrbitViewport, setDisplayMode]);

  const orbitStageRef = React.useRef<HTMLElement | null>(null);
  const setOrbitStageRef = React.useCallback((el: HTMLElement | null) => {
    orbitStageRef.current = el;
  }, []);

  // Unified toggle selection: click a session to open its detail; re-click the
  // same session (or Escape / backdrop / close) to dismiss it.
  const handleSelectSession = React.useCallback(
    (session: SessionRow) => {
      setSelectedSession((prev) => (prev?.id === session.id ? null : session));
    },
    [setSelectedSession]
  );

  const handleBackToList = React.useCallback(() => {
    setSelectedSession(null);
    if (location.search.includes("sessionId")) {
      navigate("/history", { replace: true });
    }
  }, [setSelectedSession, location.search, navigate]);

  useOverlay({
    onClose: handleBackToList,
    active: !isOrbitViewport && Boolean(selectedSession),
  });

  const handleSearchSelectSession = React.useCallback(
    (session: SessionRow) => {
      const date = new Date(session.created_at);
      const dayKey = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
      handleDrillIntoDay(dayKey);
      setSelectedSession(session);
    },
    [handleDrillIntoDay, setSelectedSession]
  );

  const drawerHandlers = React.useMemo(() => ({
    open: () => {
      if (selectedSession) return;
      if (currentWindowSessions.length > 0) {
        setSelectedSession(currentWindowSessions[0]);
      } else if (sessions.length > 0) {
        setSelectedSession(sessions[0]);
      }
    },
    close: () => {
      setSelectedSession(null);
    },
  }), [selectedSession, currentWindowSessions, sessions, setSelectedSession]);
  useRegisterPageDrawer(drawerHandlers);

  // Arrow-key session card navigation: ArrowLeft / ArrowRight cycles sessions and orbits the ring
  React.useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      const tag = activeEl?.tagName.toLowerCase();
      const isEditable =
        tag === "input" ||
        tag === "textarea" ||
        tag === "select" ||
        activeEl?.getAttribute("contenteditable") === "true";
      if (isEditable) return;
      if (e.shiftKey || e.ctrlKey || e.metaKey) return;

      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        if (!isOrbitViewport || currentWindowSessions.length === 0) return;
        e.preventDefault();
        e.stopPropagation();

        const currentId = selectedSession?.id;
        const currentIndex = currentWindowSessions.findIndex((s) => s.id === currentId);

        let nextIndex: number;
        if (currentIndex === -1) {
          nextIndex = e.key === "ArrowRight" ? 0 : currentWindowSessions.length - 1;
        } else {
          nextIndex =
            e.key === "ArrowRight"
              ? (currentIndex + 1) % currentWindowSessions.length
              : (currentIndex - 1 + currentWindowSessions.length) % currentWindowSessions.length;
        }

        setSelectedSession(currentWindowSessions[nextIndex]);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOrbitViewport, currentWindowSessions, selectedSession, setSelectedSession]);

  const monthNodeIds = React.useMemo(() => {
    return (currentMonthWindow?.days ?? []).map((d) => d.dayKey);
  }, [currentMonthWindow?.days]);

  // Publish the drill-down row, never un-publish on cleanup. A nulling cleanup
  // made the store oscillate null -> row -> null on every in-place session
  // refresh, remounting the drill-down subtree and its top-right triggers.
  React.useEffect(() => {
    setDrillDownSession(!isOrbitViewport && selectedSession ? selectedSession : null);
  }, [isOrbitViewport, selectedSession, setDrillDownSession]);

  const dayNodeIds = React.useMemo(() => {
    return currentWindowSessions.map((s) => String(s.id));
  }, [currentWindowSessions]);

  const monthWindowProgress = React.useMemo(
    () => ({
      index: monthWindowIndex,
      count: monthWindows.length,
    }),
    [monthWindowIndex, monthWindows.length]
  );

  const dayWindowProgress = React.useMemo(
    () => ({
      index: dayWindowIndex,
      count: dayWindows.length,
    }),
    [dayWindowIndex, dayWindows.length]
  );

  const renderMonthNode = React.useCallback(
    (dayKey: string) => {
      const day = currentMonthGroup?.days.find((d) => d.dayKey === dayKey);
      if (!day) return null;
      return <MonthDayCard day={day} onOpen={handleDrillIntoDay} />;
    },
    [currentMonthGroup?.days, handleDrillIntoDay]
  );

  const renderDayNode = React.useCallback(
    (id: string) => {
      const session = sessionById.get(id);
      if (!session) return null;
      return (
        <VoiceRippleNode
          session={session}
          isSelected={selectedSession?.id === session.id}
          isConfirmingDelete={confirmDeleteId === session.id}
          onSelect={handleSelectSession}
          onDelete={handleDelete}
          onCancelDelete={handleCancelDelete}
        />
      );
    },
    [sessionById, selectedSession?.id, confirmDeleteId, handleSelectSession, handleDelete, handleCancelDelete]
  );

  return (
    <div
      ref={containerRef}
      onClick={isOrbitViewport ? handleStageClick : undefined}
      className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* ── Top Bar Search: Active in Orbit View; suppressed in List View (which has dedicated fixed search) ── */}
      {isOrbitViewport && (
        <div className="absolute top-4 left-24 right-32 z-30 pointer-events-auto flex justify-center">
          <HistorySearchBar
            sessions={sessions}
            onSelectSession={handleSearchSelectSession}
            className="w-full max-w-[280px]"
          />
        </div>
      )}

      {/* ── Top Left: Theme Toggle (Orbit view only; list view renders theme toggle on the right header) ── */}
      {isOrbitViewport && (
        <div
          className={cn(
            "absolute top-4 left-5 z-[60] transition-opacity duration-200",
            Boolean(rightPanel) && (typeof window !== "undefined" && window.innerWidth < BREAKPOINT_OPPOSITE_COLLISION_MAX)
              ? "opacity-0 pointer-events-none invisible"
              : "pointer-events-auto"
          )}
        >
          <ThemeToggleButton />
        </div>
      )}

      {/* Delete Error Notification Banner */}
      <AnimatePresence>
        {deleteError && (
          <motion.div
            initial={{ opacity: 0, y: -20, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -20, scale: 0.95 }}
            className="absolute top-16 left-1/2 -translate-x-1/2 z-[100] max-w-md w-full px-4 pointer-events-auto"
          >
            <div className="glass-card p-3.5 rounded-xl flex items-center justify-between gap-3 border border-red-500/30 shadow-2xl bg-[rgb(var(--card))]/90 backdrop-blur-md text-left">
              <div className="flex items-center gap-2.5 min-w-0">
                <AlertCircle className="text-red-400 shrink-0" size={16} />
                <span className="text-[12px] font-medium text-[rgb(var(--foreground))] truncate">
                  {deleteError}
                </span>
              </div>
              <button
                onClick={() => setDeleteError(null)}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-1 rounded-lg transition-colors cursor-pointer"
                aria-label={HISTORY_COPY.dismissError}
              >
                <X size={14} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Main Canvas Stage: Direct on Ambient Field (Mounted underneath loader for instant warm render) ── */}
      {error ? (
        <div className="absolute inset-0 flex items-center justify-center p-8 z-20">
          <div className="max-w-sm w-full glass-card p-6 rounded-2xl flex flex-col items-center text-center gap-4 border border-red-500/20">
            <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center text-red-400">
              <AlertCircle size={20} />
            </div>
            <div className="space-y-1">
              <h3 className="font-display text-[14px] font-bold text-[rgb(var(--foreground))]">
                {HISTORY_COPY.failedTitle}
              </h3>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
                {error}
              </p>
            </div>
            <button
              onClick={loadSessions}
              className="px-4 py-2 rounded-xl glass-card border border-[rgba(var(--accent),0.3)] text-[12px] font-bold text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors flex items-center gap-2 cursor-pointer"
            >
              <RotateCcw size={14} />
              {HISTORY_COPY.retry}
            </button>
          </div>
        </div>
      ) : sessions.length === 0 && !showLoading ? (
        <div className="absolute inset-0 flex items-center justify-center p-8 z-20">
          <EmptyState
            icon={Ghost}
            title={HISTORY_COPY.noMemoriesTitle}
            description={HISTORY_COPY.noMemoriesDesc}
            className="max-w-sm border-0 bg-transparent"
          />
        </div>
      ) : (
        /* Interactive Orbit/Stage Content */
        <ErrorBoundary name="HistoryStage">
          {effectiveView === "month" && isOrbitViewport ? (
            // ── Month View (Calendar Orbit) — Exactly vertically centered matching Home.tsx Orb ──
            <div
              ref={setOrbitStageRef}
              className="absolute left-1/2 flex items-center justify-center z-20"
              style={{
                top: "calc(50% - 36px)",
                transform: "translate(-50%, -50%)",
                width: "100%",
                height: "100%",
              }}
            >
              <OrbitCarousel
                nodeIds={monthNodeIds}
                radius={ringRadius}
                selectedId={null}
                paused={Boolean(selectedSession) || isProfilerOpen || isPanelOpen("help") || isPanelOpen("notifications") || isThemeTransitioning}
                onDragStateChange={handleDragState}
                renderNode={renderMonthNode}
                blurTargetRef={orbitStageRef}
              />

              <CentralClockNode
                variant="month"
                view={view}
                onViewChange={handleViewChange}
                primaryLabel={formatMonthHeroLabel(currentMonthGroup.monthKey)}
                secondaryLabel={formatMonthYearLabel(currentMonthGroup.monthKey)}
                monthFullLabel={formatMonthFullLabel(currentMonthGroup.monthKey)}
                metaLabel={monthMetaLabel}
                sessionsCount={currentMonthGroup.totalSessions}
                memoriesCount={monthTurnsCount}
                timeSpanLabel={currentMonthWindow?.label}
                windowLabel={currentMonthWindow?.label}
                windowProgress={monthWindowProgress}
                canPrev={
                  monthWindowIndex < monthWindows.length - 1 ||
                  monthIndex < totalMonths - 1
                }
                canNext={monthWindowIndex > 0 || monthIndex > 0}
                onPrev={handlePrevMonth}
                onNext={handleNextMonth}
              />
            </div>
          ) : isOrbitViewport ? (
            // ── Day View — Exactly vertically centered matching Home.tsx Orb ──
            <div
              ref={setOrbitStageRef}
              className="absolute left-1/2 flex items-center justify-center z-20"
              style={{
                top: "calc(50% - 36px)",
                transform: "translate(-50%, -50%)",
                width: "100%",
                height: "100%",
              }}
            >
              <OrbitCarousel
                nodeIds={dayNodeIds}
                radius={ringRadius}
                selectedId={selectedSession ? String(selectedSession.id) : null}
                paused={Boolean(selectedSession) || isProfilerOpen || isPanelOpen("help") || isPanelOpen("notifications") || isThemeTransitioning}
                onDragStateChange={handleDragState}
                renderNode={renderDayNode}
                blurTargetRef={orbitStageRef}
              />

              <CentralClockNode
                variant="day"
                view={view}
                onViewChange={handleViewChange}
                primaryLabel={currentWindow?.dateSpanLabel || formatDayHeroLabel(currentGroup.dayKey)}
                secondaryLabel={formatDayYearLabel(currentGroup.dayKey)}
                dayHeroParts={formatDayHeroParts(currentGroup.dayKey)}
                dateSpanLabel={currentWindow?.dateSpanLabel}
                weekdayLabel={formatWeekdayLabel(currentGroup.dayKey)}
                metaLabel={dayMetaLabel}
                sessionsCount={currentWindowSessions.length}
                memoriesCount={dayTurnsCount}
                timeSpanLabel={dayTimeSpan || currentWindow?.label}
                windowLabel={currentWindow?.label}
                windowProgress={dayWindowProgress}
                canPrev={dayWindowIndex < dayWindows.length - 1}
                canNext={dayWindowIndex > 0}
                onPrev={handlePrevDate}
                onNext={handleNextDate}
              />
            </div>
          ) : (
            // ── Compact Viewport: In-Place Session Drill-Down or Full Scrollable List ──
            <div className="w-full h-full flex flex-col min-h-0 overflow-hidden relative">
              <div className={cn("w-full h-full flex flex-col min-h-0", selectedSession ? "hidden" : "flex")}>
                <HistoryListView
                  dayLabel={
                    currentWindow?.dateSpanLabel
                      ? HISTORY_COPY.sessionsInWindow(currentWindow.dateSpanLabel)
                      : HISTORY_COPY.allSessionsLabel
                  }
                  sessions={sessions}
                  selectedSession={selectedSession}
                  confirmDeleteId={confirmDeleteId}
                  onSelect={handleSelectSession}
                  onDelete={handleDelete}
                  onCancelDelete={handleCancelDelete}
                />
              </div>

              {selectedSession && (
                <div
                  onClick={(e) => e.stopPropagation()}
                  className="absolute inset-0 z-20 w-full h-full flex flex-col min-h-0 overflow-hidden bg-[rgb(var(--background))] select-text animate-in fade-in duration-200"
                >
                  {/* Top Navigation Bar: Simple Arrow Back + Wrapped Title + Session Actions (pr-28 sm:pr-32 reserves clear space for TopRightCluster) */}
                  <div className="shrink-0 px-4 sm:px-6 pt-4 pb-3 border-b border-[rgba(var(--border),0.12)] bg-[rgb(var(--background))]/80 backdrop-blur-md flex items-start justify-between gap-3 pr-28 sm:pr-32">
                    <div className="flex items-start gap-3 min-w-0 flex-1">
                      <button
                        type="button"
                        onClick={handleBackToList}
                        className="flex items-center justify-center w-8 h-8 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer shrink-0 mt-0.5"
                        aria-label={HISTORY_COPY.allSessionsLabel}
                      >
                        <ArrowLeft size={18} />
                      </button>
                      <div className="flex flex-col min-w-0 flex-1">
                        <h2 className="text-[14px] sm:text-[15px] font-semibold text-[rgb(var(--foreground))] leading-snug break-words">
                          {resolveSessionTitle(selectedSession)}
                        </h2>
                        <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1">
                          <span>{formatDateTime(selectedSession.created_at)}</span>
                          <span>·</span>
                          <span>
                            {selectedSession.turn_count}{" "}
                            {selectedSession.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Full Transcript Body (full viewport width, no gaps on sides) */}
                  <div className="relative flex-1 min-h-0 w-full flex flex-col overflow-hidden">
                    <div className="flex-1 min-h-0 w-full overflow-y-auto px-4 sm:px-8 py-5 pb-28 custom-scrollbar">
                      <ErrorBoundary name="HistoryPanelDetail">
                        <DetailPanel
                          open
                          variant="inline"
                          session={selectedSession}
                          turns={turns}
                          loading={turnsLoading}
                          error={turnsError}
                          onClose={handleBackToList}
                          onRetry={retryFetchTurns}
                        />
                      </ErrorBoundary>
                    </div>

                    {/* Full-width bottom dock feather dissolve identical to parent HistoryListView */}
                    <BottomDockFeather className="absolute bottom-0 left-0 right-0 h-[72px] pointer-events-none z-10" />
                  </div>
                </div>
              )}
            </div>
          )}
        </ErrorBoundary>
      )}

      {/* Smooth, Ethereal Cross-fade Loader Overlay (Vertically aligned with CentralClockNode) */}
      <AnimatePresence>
        {showLoading && (
          <motion.div
            initial={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
            className="absolute inset-0 z-40 flex items-center justify-center bg-transparent pointer-events-none select-none"
          >
            <div
              className="absolute left-1/2 flex items-center justify-center pointer-events-none"
              style={{
                top: "calc(50% - 36px)",
                transform: "translate(-50%, -50%)",
              }}
            >
              <OrbitalLoader size="md" />
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Floating Micro-interaction Hint */}
      {!showLoading && sessions.length > 0 && isOrbitViewport && (
        <div className="absolute bottom-24 left-1/2 -translate-x-1/2 z-30 flex flex-col items-center gap-2 pointer-events-none">
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))] opacity-75">
            <Hand size={12} className="text-[rgb(var(--accent))]" />
            <span>{HISTORY_COPY.dragHint}</span>
            <span>•</span>
            <span>{hintText}</span>
          </div>
        </div>
      )}

      {/* Slide-up Detail Transcript Panel (wide viewports only; compact uses
          the in-place drill-down view above. Backdrop + Escape handled internally.) */}
      {isOrbitViewport && (
        <ErrorBoundary name="HistoryDetailPanel">
          <DetailPanel
            open={!!selectedSession}
            session={selectedSession}
            turns={turns}
            loading={turnsLoading}
            error={turnsError}
            onClose={handleBackToList}
            onRetry={retryFetchTurns}
          />
        </ErrorBoundary>
      )}
    </div>
  );
};
