import React, { memo, useMemo, useRef, useEffect, useCallback, lazy, Suspense, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
const VoxOrb = lazy(() =>
  import("@/shared/components/home/AdvancedOrb").then((m) => ({ default: m.VoxOrb }))
);
import { PipelineField } from "@/shared/components/home/PipelineField";
import { OrbitField } from "@/shared/components/home/OrbitField";
import { DynamicStatusBadge } from "@/shared/components/home/DynamicStatusBadge";
import { RestorePulse } from "@/shared/components/home/RestorePulse";
import { SessionPanel } from "@/shared/components/home/SessionPanel";
import { DialogueBubble } from "@/shared/components/home/DialogueBubble";
import { TextInputBar } from "@/shared/components/home/TextInputBar";
import { ActiveTranscript } from "@/shared/components/home/ActiveTranscript";
import { SearchRecallView, DetailPanel, SearchModeOrbHalo, useDetailPanelTurns } from "@/shared/components/home/recall";
import { ErrorBoundary } from "@/shared/components/common";
import { EdgePanel } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/context/PanelStateContext";
import {
  GOVERNOR_LABELS,
  HOME_CONTROLS_COPY,
  ERROR_BANNER_COPY,
} from "@/data/homeCopy";

import { AlertCircle } from "lucide-react";

import { ChapterScrubber, type Chapter } from "@/shared/ui/ChapterScrubber";
import { HomeControls } from "@/shared/components/home/HomeControls";

import { useSessionStore } from "@/store/sessionStore";
import { useRemoteStore } from "@/store/remoteStore";
import { useRecallSearchStore } from "@/store/recallSearchStore";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";
import { useHomePage } from "@/shared/hooks/useHomePage";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { isRealMobileDevice } from "@/layout/breakpoints";
import { stageTop } from "@/layout/stageMetrics";
import { getErrorMessage } from "@/shared/components/remote/QrScannerModal";
import {
  getSessions,
  getCachedSessions,
  type SessionRow,
} from "@/services/persistenceService";
import { getProjects } from "@/services/projectsService";
import { useOverlay } from "@/shared/hooks/useOverlay";

const DialogueTurn = memo(({ turn }: { turn: { user: string; assistant: string; id: number } }) => (
  <React.Fragment>
    {turn.user && (
      <DialogueBubble role="user" content={turn.user} />
    )}
    {turn.assistant && (
      <DialogueBubble role="assistant" content={turn.assistant} />
    )}
  </React.Fragment>
));
DialogueTurn.displayName = "DialogueTurn";

export const Home = memo(() => {
  const {
    interactionState,
    interactionMode,
    isEngaged,
    isSleeping,
    isPaused,
    pttStatus,
    cpuWarning,
    isTemporarySession,
    isTextModeOpen,
    isPlaybackMuted,
    isMicMuted,
    dialogueHistory,
    telemetryRef,
    restoreError,
    dismissRestoreError,
    restoreSignal,
    isLaunching,
    isThinking,
    isMobileScreen,
    engage,
    cancelLaunch,
    disengage,
    pause,
    resume,
    handlePttStart,
    handlePttStop,
    handlePttCancel,
    submitText,
    setTextModeOpen,
    togglePlaybackMute,
    toggleMicMute,
    dialogueScrollRef,
    handleDialogueScroll,
    shouldAutoScrollRef,
    activityDisplay,
    activeActivity,
    isHost,
    isRemoteBlocked,
    remoteFailureReason,
    dismissRemoteFailure,
  } = useHomePage();
  const linkState = useRemoteStore((s) => s.linkState);
  const failureKind = useRemoteStore((s) => s.failureKind);
  const connectedPeer = useRemoteStore((s) => s.connectedPeer);

  const isRemoteActive =
    isHost &&
    (linkState === "Connected" ||
      linkState === "Connecting" ||
      linkState === "Reconnecting");

  const peerDisplayName = useMemo(() => {
    const raw = connectedPeer?.displayName?.trim();
    if (
      !raw ||
      raw.toLowerCase() === "localhost" ||
      raw.toLowerCase() === "localhost.localdomain"
    ) {
      return "PHONE";
    }
    return raw.toUpperCase();
  }, [connectedPeer?.displayName]);
  useMemoryTrace("Home");

  const { isPanelOpen, closePanel } = usePanelStateContext();
  const [searchParams, setSearchParams] = useSearchParams();
  const isSearchModeStore = useRecallSearchStore((s) => s.isSearchMode);
  const setIsSearchModeStore = useRecallSearchStore((s) => s.setIsSearchMode);
  const exitSearchModeStore = useRecallSearchStore((s) => s.exitSearchMode);

  // Search mode is active if URL says so OR if persistent store says so (survives page nav)
  const isSearchMode = searchParams.get("mode") === "search" || isSearchModeStore;

  // Keep URL query param and store in sync
  useEffect(() => {
    if (isSearchMode && searchParams.get("mode") !== "search") {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        next.set("mode", "search");
        return next;
      }, { replace: true });
    }
  }, [isSearchMode, searchParams, setSearchParams]);

  // Prewarm conversation sessions & projects in background idle so EdgePanel & Search Mode open with 0ms IPC lag
  useEffect(() => {
    const timer = setTimeout(() => {
      void getSessions().catch(() => {});
      void getProjects().catch(() => {});
    }, 60);
    return () => clearTimeout(timer);
  }, []);

  const enterSearchMode = useCallback(() => {
    const t0 = performance.now();
    console.info(`[trace-search] enterSearchMode triggered @${t0.toFixed(1)}ms`);
    if (isMobileScreen) {
      closePanel("sessions");
    }
    setIsSearchModeStore(true);
    setSearchParams((prev) => {
      if (prev.get("mode") === "search") return prev;
      const next = new URLSearchParams(prev);
      next.set("mode", "search");
      return next;
    }, { replace: true });
    console.info(`[trace-search] enterSearchMode finished @${performance.now().toFixed(1)}ms (+${(performance.now() - t0).toFixed(1)}ms)`);
  }, [isMobileScreen, closePanel, setIsSearchModeStore, setSearchParams]);

  const exitSearchMode = useCallback(() => {
    const t0 = performance.now();
    console.info(`[trace-search] exitSearchMode triggered @${t0.toFixed(1)}ms`);
    exitSearchModeStore();
    setSearchParams((prev) => {
      if (!prev.has("mode")) return prev;
      const next = new URLSearchParams(prev);
      next.delete("mode");
      return next;
    }, { replace: true });
    console.info(`[trace-search] exitSearchMode finished @${performance.now().toFixed(1)}ms (+${(performance.now() - t0).toFixed(1)}ms)`);
  }, [exitSearchModeStore, setSearchParams]);

  const [homeDrawerSession, setHomeDrawerSession] = useState<SessionRow | null>(null);
  const {
    turns: homeTurns,
    loading: homeTurnsLoading,
    error: homeTurnsError,
    retry: retryHomeTurns,
  } = useDetailPanelTurns(homeDrawerSession);

  const activeSessionId = useSessionStore((s) => s.activeSessionId);

  // Shift + Up global shortcut opens session detail drawer on Home:
  // Active session if currently engaged in one, or latest chronological session if idle.
  const openDetailDrawer = useCallback(async () => {
    if (isSearchMode) return;
    if (activeSessionId) {
      const cached = getCachedSessions()?.find((s) => s.id === activeSessionId);
      if (cached) {
        setHomeDrawerSession(cached);
        return;
      }
    }
    try {
      const list = await getSessions();
      if (list.length > 0) {
        const latest = [...list].sort((a, b) => b.created_at - a.created_at)[0];
        setHomeDrawerSession(latest);
      }
    } catch (e) {
      console.warn("[Home] Failed to load session for drawer:", e);
    }
  }, [isSearchMode, activeSessionId]);

  const closeDetailDrawer = useCallback(() => {
    setHomeDrawerSession(null);
  }, []);

  const drawerHandlers = useMemo(() => ({
    open: openDetailDrawer,
    close: closeDetailDrawer,
  }), [openDetailDrawer, closeDetailDrawer]);
  useRegisterPageDrawer(drawerHandlers);

  // Escape exits Search Mode when detail drawer is not open
  useOverlay({
    onClose: exitSearchMode,
    active: isSearchMode && !homeDrawerSession,
  });

  const contentInnerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll follower: keep pinned to bottom as text streams in or messages are sent
  useEffect(() => {
    const inner = contentInnerRef.current;
    const scrollContainer = dialogueScrollRef.current;
    if (!inner || !scrollContainer) return;

    const observer = new ResizeObserver(() => {
      if (shouldAutoScrollRef.current) {
        scrollContainer.scrollTop = scrollContainer.scrollHeight;
      }
    });
    observer.observe(inner);

    return () => observer.disconnect();
  }, [shouldAutoScrollRef, dialogueScrollRef]);

  // Keyboard shortcuts (Home page): M = mute mic, Shift+M = mute speaker, P = pause/resume, T = text mode, Space = PTT hold, Ctrl+Space = engage/disengage
  useEffect(() => {
    const isEditable = (el: Element | null): boolean => {
      if (!el) return false;
      const tag = el.tagName.toLowerCase();
      return tag === "input" || tag === "textarea" || tag === "select" || el.getAttribute("contenteditable") === "true";
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (isEditable(document.activeElement)) return;
      if (e.repeat) return;
      // Space on a focused button must activate the button, never PTT:
      // native button activation would otherwise race handlePttStart.
      // (Ctrl+Space is an explicit chord and keeps working everywhere.)
      const activeEl = document.activeElement as HTMLElement | null;
      const spaceOnButton =
        e.key === " " && (activeEl?.tagName === "BUTTON" || activeEl?.getAttribute("role") === "button");

      if ((e.key === "m" || e.key === "M") && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        void toggleMicMute();
        return;
      }
      if ((e.key === "m" || e.key === "M") && e.shiftKey) {
        e.preventDefault();
        void togglePlaybackMute();
        return;
      }
      if ((e.key === "p" || e.key === "P") && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        if (isPaused) void resume();
        else void pause();
        return;
      }
      if (e.key === " " && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        if (isRemoteBlocked) return;
        if (isEngaged) void disengage();
        else void engage();
        return;
      }
      if ((e.key === "t" || e.key === "T") && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        if (isEngaged) setTextModeOpen(true);
        return;
      }
      // Escape while the text bar is open belongs to the text bar (it closes it);
      // pausing here as well would fire two actions from one key.
      if (e.key === "Escape" && !isTextModeOpen && (interactionState === "Speaking" || interactionState === "Thinking")) {
        e.preventDefault();
        void pause();
        return;
      }
      if (e.key === " " && !spaceOnButton && !e.ctrlKey && !e.metaKey && interactionMode === "PTT" && isEngaged && !isPaused && !isSleeping && interactionState !== "Error") {
        e.preventDefault();
        void handlePttStart();
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key === " " && !e.ctrlKey && !e.metaKey) {
        const activeEl = document.activeElement as HTMLElement | null;
        const endOnButton =
          activeEl?.tagName === "BUTTON" || activeEl?.getAttribute("role") === "button";
        if (!endOnButton && interactionMode === "PTT" && isEngaged && !isPaused && !isSleeping && interactionState !== "Error") {
          if (pttStatus === "RECORDING") void handlePttStop();
          else void handlePttCancel();
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [toggleMicMute, togglePlaybackMute, isPaused, pause, resume, isEngaged, engage, disengage, setTextModeOpen, interactionMode, isSleeping, interactionState, handlePttStart, handlePttStop, handlePttCancel, pttStatus, isTextModeOpen, isRemoteBlocked]);

  const sessionError = useSessionStore((s) => s.sessionError);
  const clearSessionError = useSessionStore((s) => s.setSessionError);
  // Stable identity: Home re-renders on every streamed transcript token. An
  // inline arrow here would give `SessionPanel` (memo) a new `onClose` prop on
  // every token, defeating memo and re-rendering the whole session list.
  const closeSessions = useCallback(() => closePanel("sessions"), [closePanel]);


  // Bound visible dialogue history to recent turns to prevent unbounded DOM accumulation
  const visibleDialogueTurns = useMemo(() => {
    return dialogueHistory.slice(-10);
  }, [dialogueHistory]);

  const userTurnHistory = useMemo(() => {
    return dialogueHistory.map((d) => d.user).filter(Boolean);
  }, [dialogueHistory]);

  const scrubberChapters = useMemo<Chapter[]>(() => {
    // Full history, oldest at top. The scrubber windows it to `maxVisible` rows
    // and pages older turns with its chevrons, so no turn becomes unreachable.
    return dialogueHistory.map((turn, absIdx) => {
      const title = turn.user ? (turn.user.length > 40 ? `${turn.user.slice(0, 40)}...` : turn.user) : `Turn ${absIdx + 1}`;
      const desc = turn.assistant || undefined;
      return {
        id: `dialogue-${turn.id}`,
        title,
        description: desc,
        meta: `Turn #${absIdx + 1}`,
      };
    });
  }, [dialogueHistory]);


  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Sentient Field Background Energy */}
      <PipelineField state={isSearchMode ? "Sleeping" : interactionState} paused={isSearchMode} />

      {/* ── Restore ingestion pulse (single reverse-flow toward orb) ── */}
      <RestorePulse signal={restoreSignal} />

      {/* ── Orb Stage (Anchored in stage center; celestial rising sun behind cards in Search Mode) ── */}
      <div
        className="absolute z-10 flex items-center justify-center select-none pointer-events-none"
        style={{
          left: "50%",
          top: stageTop(isRealMobileDevice(), 36),
          transform: "translate(-50%, -50%)",
          width: isMobileScreen ? "min(82vw, 50vh)" : "min(65vw, 56vh)",
          height: isMobileScreen ? "min(82vw, 50vh)" : "min(65vw, 56vh)",
          minWidth: isMobileScreen ? 180 : 220,
          minHeight: isMobileScreen ? 180 : 220,
          maxWidth: 580,
          maxHeight: 580,
        }}
      >
        {/* Dynamic Celestial Time Rings & Core Halo — Activates during Search Mode */}
        {isSearchMode && <SearchModeOrbHalo />}

        {/* Subtle dynamic ring behind orb — shown when OrbitField is dormant */}
        {!isEngaged && !isSearchMode && (
          <div
            className="absolute inset-0 rounded-full border border-[rgb(var(--accent))]/10 transition-all duration-1000 scale-90 opacity-60"
          />
        )}
        <div
          className="relative w-full h-full flex items-center justify-center transition-[mask-image,-webkit-mask-image] duration-500 ease-out"
          style={{
            maskImage: isSearchMode
              ? "linear-gradient(to bottom, black 0%, black 35%, rgba(0,0,0,0.65) 50%, rgba(0,0,0,0.2) 65%, transparent 80%)"
              : undefined,
            WebkitMaskImage: isSearchMode
              ? "linear-gradient(to bottom, black 0%, black 35%, rgba(0,0,0,0.65) 50%, rgba(0,0,0,0.2) 65%, transparent 80%)"
              : undefined,
          }}
        >
          <OrbitField
            interactionState={isSearchMode ? "Idle" : (isRemoteActive ? "Sleeping" : interactionState)}
            activity={isSearchMode ? undefined : activeActivity?.name}
            telemetryRef={telemetryRef}
            paused={isPanelOpen("sessions") || isPanelOpen("notifications") || isPanelOpen("help")}
          >
            <ErrorBoundary name="VoxOrb">
              <Suspense fallback={null}>
                <VoxOrb
                  telemetryRef={telemetryRef}
                  interactionState={isSearchMode ? "Idle" : (isRemoteActive ? "Sleeping" : interactionState)}
                  isSleeping={isSleeping || isRemoteActive}
                  paused={isPanelOpen("sessions") || isPanelOpen("notifications") || isPanelOpen("help")}
                />
              </Suspense>
            </ErrorBoundary>
          </OrbitField>
        </div>
      </div>

      {/* ── Search / Recall Mode Overlay (Spatial In-Place Transformation) ── */}
      <AnimatePresence>
        {isSearchMode && (
          <motion.div
            key="search-recall-stage"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.25, ease: "easeOut" }}
            className="absolute inset-0 flex flex-col justify-between pointer-events-none z-20"
          >
            <SearchRecallView onBackToVoice={exitSearchMode} isMobile={isMobileScreen} />
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Voice Mode Controls & Dialogue (Spatial Transition: glides away when Search Mode is active) ── */}
      <div
        className="absolute inset-0 flex flex-col items-center justify-between pointer-events-none"
      >
        {/* ── Top Notification & Error Banners (Positioned neatly directly below AppHeader) ── */}
        {(restoreError || sessionError || remoteFailureReason) && !isSearchMode && (
          <div className="absolute top-2 sm:top-3 left-1/2 -translate-x-1/2 z-[100] flex flex-col items-center gap-2 w-[min(92vw,480px)] pointer-events-none px-2">
            {restoreError && (
              <div className="glass-card px-3.5 py-2 rounded-xl flex items-center justify-between gap-2.5 border border-[rgba(var(--error),0.3)] shadow-lg bg-[rgb(var(--card))]/95 w-full pointer-events-auto animate-fade-in">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <AlertCircle className="text-[rgb(var(--error))] shrink-0" size={15} />
                  <p className="text-[12px] text-[rgb(var(--foreground))]/90 break-words select-text">
                    {restoreError}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => dismissRestoreError()}
                  className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0"
                >
                  {ERROR_BANNER_COPY.dismissButton}
                </button>
              </div>
            )}

            {sessionError && (
              <div role="alert" className="glass-card px-3.5 py-2 rounded-xl flex items-center justify-between gap-2.5 border border-[rgba(var(--error),0.3)] shadow-lg bg-[rgb(var(--card))]/95 w-full pointer-events-auto animate-fade-in">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <AlertCircle className="text-[rgb(var(--error))] shrink-0" size={15} />
                  <p className="text-[12px] text-[rgb(var(--foreground))]/90 break-words select-text">
                    {sessionError}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => clearSessionError(null)}
                  className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0"
                >
                  {ERROR_BANNER_COPY.dismissButton}
                </button>
              </div>
            )}

            {remoteFailureReason && (
              <div role="alert" className="glass-card px-3.5 py-2 rounded-xl flex items-center justify-between gap-2.5 border border-[rgba(var(--danger),0.35)] shadow-lg bg-[rgb(var(--card))]/95 w-full pointer-events-auto animate-fade-in">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <AlertCircle className="text-[rgb(var(--danger))] shrink-0" size={15} />
                  <p className="text-[12px] text-[rgb(var(--foreground))]/90 break-words select-text">
                    {HOME_CONTROLS_COPY.remoteFailurePrefix} {getErrorMessage(failureKind, remoteFailureReason).title}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={dismissRemoteFailure}
                  className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0"
                >
                  {ERROR_BANNER_COPY.dismissButton}
                </button>
              </div>
            )}
          </div>
        )}

        {/* ── Status Header: Fixed top 10% across desktop, adjusted for mobile header padding ── */}
        <motion.div
          animate={{
            y: isSearchMode ? -40 : 0,
            opacity: isSearchMode ? 0 : 1,
          }}
          transition={{ type: "spring", stiffness: 320, damping: 28 }}
          className="absolute left-1/2 -translate-x-1/2 z-30 flex flex-col items-center pointer-events-none text-center"
          style={{
            top: isRealMobileDevice()
              ? "calc(50% - min(41vw, 25vh) - 112px - 22.4px)"
              : "10%",
          }}
        >
          {cpuWarning && (
            <span className="text-[11px] tracking-widest uppercase text-[rgb(var(--accent))]/70 font-semibold px-2 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 mb-1">
              Mode: {GOVERNOR_LABELS[cpuWarning.governor] || cpuWarning.governor}
            </span>
          )}
          {isTemporarySession && (
            <div className="flex items-center text-[9px] font-mono font-semibold tracking-[0.25em] uppercase text-[rgb(var(--accent))]/60 leading-none mb-1 animate-in fade-in duration-200">
              <span>{HOME_CONTROLS_COPY.temporary.activeBadge}</span>
            </div>
          )}
          {isRemoteActive && (
            <div className="flex items-center gap-1.5 text-[10px] font-mono font-semibold tracking-[0.2em] uppercase text-[rgb(var(--accent))]/80 leading-none mb-1.5 animate-in fade-in duration-200">
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
              <span>{`${linkState.toUpperCase()}: ${peerDisplayName}`}</span>
            </div>
          )}
          <DynamicStatusBadge
            label={activityDisplay.label}
            orbState={activityDisplay.orbState}
            shimmer={activityDisplay.shimmer}
          />
        </motion.div>

        {/* ── Side Dialogue Area - Right Only (All transcripts, big screens only) ── */}
        <motion.div
          animate={{
            x: isSearchMode ? 60 : 0,
            opacity: isSearchMode ? 0 : 1,
          }}
          transition={{ duration: 0.22, ease: "easeOut" }}
          className="absolute top-[64px] bottom-[20%] right-0 flex flex-col justify-end items-center pointer-events-none hidden md:flex z-20"
          style={{
            width: "clamp(240px, calc(50vw - 20vw), 360px)",
            pointerEvents: isSearchMode ? "none" : "auto",
          }}
        >
          <div
            ref={dialogueScrollRef}
            onScroll={handleDialogueScroll}
            className="w-full max-h-[85%] overflow-y-auto scrollbar-none flex flex-col items-center gap-4 pointer-events-auto select-text px-4 pb-6"
            style={{
              maskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
              WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
            }}
          >
            <div ref={contentInnerRef} className="w-full flex flex-col items-center gap-4">
              <div className="flex-1 min-h-[4vh]" />
              {/* Dialogue History */}
              {visibleDialogueTurns.map((turn: { user: string; assistant: string; id: number }) => (
                <DialogueTurn key={turn.id} turn={turn} />
              ))}

              {/* Isolated Active Streaming Transcript */}
              <ActiveTranscript />
            </div>
          </div>
        </motion.div>

        {/* ── Dialogue Scrubber (mobile only) ── */}
        {scrubberChapters.length > 0 && !isSearchMode && (
          <div
            className="absolute z-30 pointer-events-auto md:hidden"
            style={{
              right: "max(6px, env(safe-area-inset-right, 6px))",
              top: stageTop(isRealMobileDevice(), 36),
              transform: "translateY(-50%)",
              maxHeight: isMobileScreen ? "calc(min(82vw, 50vh) * 0.75)" : "calc(min(65vw, 56vh) * 0.75)",
              overflow: "visible",
            }}
          >
            <ChapterScrubber
              chapters={scrubberChapters}
              side="left"
              peakLength={38}
              restLength={10}
              rowHeight={7}
              radius={3}
              maxVisible={15}
              currentIndex={scrubberChapters.length - 1}
              label="Dialogue History Scrubber"
            />
          </div>
        )}

        {/* ── Bottom Controls (Smoothly sinks down in Search Mode) ── */}
        <motion.div
          animate={{
            y: isSearchMode ? 80 : 0,
            opacity: isSearchMode ? 0 : 1,
            scale: isSearchMode ? 0.92 : 1,
          }}
          transition={{ type: "spring", stiffness: 320, damping: 28, mass: 0.8 }}
          className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 z-20 flex flex-col items-center gap-3 w-full max-w-md"
          style={{
            top: isMobileScreen
              ? "calc(80% - 58px + min(82vw, 50vh, 580px) * 0.16)"
              : "calc(80% - 58px + min(65vw, 56vh, 580px) * 0.16)",
            pointerEvents: isSearchMode ? "none" : "auto",
          }}
        >
          {isTextModeOpen && isEngaged ? (
            <TextInputBar
              onSubmit={submitText}
              onClose={() => setTextModeOpen(false)}
              isPlaybackMuted={isPlaybackMuted}
              onTogglePlaybackMute={togglePlaybackMute}
              isMicMuted={isMicMuted}
              onToggleMicMute={toggleMicMute}
              initialHistory={userTurnHistory}
            />
          ) : (
            <HomeControls
              restoreError={restoreError}
              dismissRestoreError={dismissRestoreError}
              isEngaged={isEngaged}
              isSleeping={isSleeping}
              isPaused={isPaused}
              interactionMode={interactionMode}
              pttStatus={pttStatus}
              isLaunching={isLaunching}
              isThinking={isThinking}
              pause={pause}
              resume={resume}
              handlePttStart={handlePttStart}
              handlePttStop={handlePttStop}
              setTextModeOpen={setTextModeOpen}
              disengage={disengage}
              engage={engage}
              cancelLaunch={cancelLaunch}
            />
          )}
        </motion.div>
      </div>

      {/* ── Left Edge Rail (Conversations) ──
          No `keepMounted`: it rendered a second, hidden SessionPanel instance
          that refetched on every sessions_changed event, and then remounted the
          panel on every open/close (EdgePanel renders `children` in two
          different tree positions). Panel state that must survive a close
          (view mode, scroll, expanded projects) already lives in
          sessionPanelUiStore/sessionPanelScrollTop. */}
      <EdgePanel side="left" open={isPanelOpen("sessions")} onClose={closeSessions} minimalHeader>
        <ErrorBoundary name="SessionPanel">
          <SessionPanel onClose={closeSessions} onSearch={enterSearchMode} onExitSearch={exitSearchMode} />
        </ErrorBoundary>
      </EdgePanel>

      {/* ── Detail Transcript Drawer for Shift+Up Shortcut ── */}
      {!isSearchMode && (
        <DetailPanel
          open={Boolean(homeDrawerSession)}
          session={homeDrawerSession}
          turns={homeTurns}
          loading={homeTurnsLoading}
          error={homeTurnsError}
          onClose={closeDetailDrawer}
          onRetry={retryHomeTurns}
        />
      )}
    </div>
  );
});

Home.displayName = "Home";
