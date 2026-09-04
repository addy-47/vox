import React from "react";
import { Ghost, X, AlertCircle, RotateCcw, Hand } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import {
  VoiceRippleNode,
  DetailPanel,
  CentralClockNode,
  OrbitCarousel,
  MonthDayCard,
  HistoryListView,
  useHistory,
} from "@/shared/components/history";
import { EmptyState, OrbitalLoader, ErrorBoundary } from "@/shared/components/common";
import { NotificationBell } from "@/shared/components/home/NotificationBell";
import { HelpTriggerButton } from "@/shared/components/help/HelpTriggerButton";
import { HISTORY_COPY } from "@/data/historyCopy";
import type { SessionRow } from "@/services/historyService";

export const History: React.FC = () => {
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
    currentDateSessions,
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
    totalDates,
    totalMonths,
    dateIndex,
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

  // Unified toggle selection: click a session to open its detail; re-click the
  // same session (or Escape / backdrop / close) to dismiss it.
  const handleSelectSession = React.useCallback(
    (session: SessionRow) => {
      setSelectedSession((prev) => (prev?.id === session.id ? null : session));
    },
    [setSelectedSession]
  );

  const monthNodeIds = React.useMemo(() => {
    return (currentMonthWindow?.days ?? []).map((d) => d.dayKey);
  }, [currentMonthWindow?.days]);

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
      onClick={handleStageClick}
      className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none"
    >
      {/* ── Top-right: Notification Bell ── */}
      <div className="absolute top-4 right-5 z-30 flex items-center gap-1.5 pointer-events-none">
        <HelpTriggerButton deepLink="page:history" className="pointer-events-auto" />
        <NotificationBell />
      </div>

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
                paused={false}
                onDragStateChange={handleDragState}
                renderNode={renderMonthNode}
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
                canPrev={monthWindowIndex > 0 || monthIndex > 0}
                canNext={
                  monthWindowIndex < monthWindows.length - 1 ||
                  monthIndex < totalMonths - 1
                }
                onPrev={handlePrevMonth}
                onNext={handleNextMonth}
              />
            </div>
          ) : isOrbitViewport ? (
            // ── Day View — Exactly vertically centered matching Home.tsx Orb ──
            <div
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
                paused={!!selectedSession}
                onDragStateChange={handleDragState}
                renderNode={renderDayNode}
              />

              <CentralClockNode
                variant="day"
                view={view}
                onViewChange={handleViewChange}
                primaryLabel={formatDayHeroLabel(currentGroup.dayKey)}
                secondaryLabel={formatDayYearLabel(currentGroup.dayKey)}
                dayHeroParts={formatDayHeroParts(currentGroup.dayKey)}
                weekdayLabel={formatWeekdayLabel(currentGroup.dayKey)}
                metaLabel={dayMetaLabel}
                sessionsCount={currentDateSessions.length}
                memoriesCount={dayTurnsCount}
                timeSpanLabel={dayTimeSpan || currentWindow?.label}
                windowLabel={currentWindow?.label}
                windowProgress={dayWindowProgress}
                canPrev={dayWindowIndex > 0 || dateIndex > 0}
                canNext={
                  dayWindowIndex < dayWindows.length - 1 ||
                  dateIndex < totalDates - 1
                }
                onPrev={handlePrevDate}
                onNext={handleNextDate}
              />
            </div>
          ) : (
            // ── Mobile Responsive Fallback List ──
            <HistoryListView
              dayLabel={currentGroup.dayLabel}
              sessions={currentDateSessions}
              selectedSession={selectedSession}
              confirmDeleteId={confirmDeleteId}
              canPrevDate={dateIndex > 0}
              canNextDate={dateIndex < totalDates - 1}
              onPrevDate={handlePrevDate}
              onNextDate={handleNextDate}
              onSelect={handleSelectSession}
              onDelete={handleDelete}
              onCancelDelete={handleCancelDelete}
            />
          )}
        </ErrorBoundary>
      )}

      {/* Smooth, Ethereal Cross-fade Loader Overlay (Matching MemoryGraph Gold Standard) */}
      <AnimatePresence>
        {showLoading && (
          <motion.div
            initial={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.55, ease: [0.16, 1, 0.3, 1] }}
            className="absolute inset-0 z-40 flex flex-col items-center justify-center bg-[rgb(var(--background))]/85 backdrop-blur-2xl pointer-events-none select-none"
          >
            <OrbitalLoader
              size="md"
              title="Loading conversations..."
              subtitle="Accessing local voice history"
              statusText="SYNCHRONIZING ORBIT"
            />
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

      {/* Slide-up Detail Transcript Panel (shared Drawer: backdrop + Escape handled internally) */}
      <ErrorBoundary name="HistoryDetailPanel">
        <DetailPanel
          open={!!selectedSession}
          session={selectedSession}
          turns={turns}
          loading={turnsLoading}
          error={turnsError}
          onClose={() => setSelectedSession(null)}
          onRetry={retryFetchTurns}
        />
      </ErrorBoundary>
    </div>
  );
};
