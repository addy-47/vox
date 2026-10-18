import React, { memo, useMemo, useRef, useEffect, lazy, Suspense } from "react";
const VoxOrb = lazy(() =>
  import("@/shared/components/home/AdvancedOrb").then((m) => ({ default: m.VoxOrb }))
);
import { PipelineField } from "@/shared/components/home/PipelineField";
import { DynamicStatusBadge } from "@/shared/components/home/DynamicStatusBadge";
import { RestorePulse } from "@/shared/components/home/RestorePulse";
import { SessionPanel } from "@/shared/components/home/SessionPanel";
import { DialogueBubble } from "@/shared/components/home/DialogueBubble";
import { TextInputBar } from "@/shared/components/home/TextInputBar";
import { ActiveTranscript } from "@/shared/components/home/ActiveTranscript";
import { ErrorBoundary } from "@/shared/components/common";
import { EdgePanel } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/context/PanelStateContext";
import {
  GOVERNOR_LABELS,
  HOME_CONTROLS_COPY,
  ERROR_BANNER_COPY,
} from "@/data/homeCopy";
import { shortcutKeysSuffix } from "@/data/shortcuts";

import { Power, Mic, Keyboard, Play, Pause, X, AlertCircle, RotateCcw, Ghost } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ChapterScrubber, type Chapter } from "@/shared/ui/ChapterScrubber";


import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { useSessionStore } from "@/store/sessionStore";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";
import { useHomePage } from "@/shared/hooks/useHomePage";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { isRealMobileDevice } from "@/layout/breakpoints";

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
    transcript,
    assistantText,
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
  } = useHomePage();
  useMemoryTrace("Home");

  const { openProfiler, closeProfiler } = useProfilerDrawer();
  const drawerHandlers = useMemo(() => ({
    open: openProfiler,
    close: closeProfiler,
  }), [openProfiler, closeProfiler]);
  useRegisterPageDrawer(drawerHandlers);

  const contentInnerRef = useRef<HTMLDivElement>(null);

  // Diagnostic telemetry: log Home render timing and layout lifecycle
  // (cyan-flash RCA: full path logging — mount/unmount, orb mount, state,
  //  theme vars, and cached theme. Remove once the flash source is found.)
  const mountStateRef = useRef({ interactionState, isEngaged });
  useEffect(() => {
    mountStateRef.current = { interactionState, isEngaged };
  }, [interactionState, isEngaged]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const t0 = performance.now();
    const root = document.documentElement;
    const cs = getComputedStyle(root);
    console.info(`[trace-home] Home mounted @${t0.toFixed(1)}ms, docTheme=${root.getAttribute("data-theme") || "none"}, cssAccent=[${cs.getPropertyValue("--accent").trim() || "EMPTY"}], cssGlowDark=[${cs.getPropertyValue("--glow-dark").trim() || "EMPTY"}], inlineAccent=[${root.style.getPropertyValue("--accent").trim() || "EMPTY"}], interactionState=${mountStateRef.current.interactionState}, isEngaged=${mountStateRef.current.isEngaged}`);
    const t1 = performance.now();
    console.info(`[trace-home] mount-paint-ready @${t1.toFixed(1)}ms (+${(t1 - t0).toFixed(1)}ms after mount effect)`);
    return () => {
      console.info(`[trace-home] Home unmounted after ${(performance.now() - t0).toFixed(1)}ms`);
    };
  }, []);

  // Auto-scroll follower: keep pinned to bottom as text streams in or messages are sent
  useEffect(() => {
    const inner = contentInnerRef.current;
    const scrollContainer = dialogueScrollRef.current;
    if (!inner || !scrollContainer) return;

    const observer = new ResizeObserver(() => {
      if (shouldAutoScrollRef.current && scrollContainer) {
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
      const spaceOnButton =
        e.key === " " && (document.activeElement as HTMLElement | null)?.tagName === "BUTTON";

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
        const endOnButton =
          (document.activeElement as HTMLElement | null)?.tagName === "BUTTON";
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
  }, [toggleMicMute, togglePlaybackMute, isPaused, pause, resume, isEngaged, engage, disengage, setTextModeOpen, interactionMode, isSleeping, interactionState, handlePttStart, handlePttStop, handlePttCancel, pttStatus, isTextModeOpen]);

  const { isPanelOpen, closePanel } = usePanelStateContext();
  const sessionError = useSessionStore((s) => s.sessionError);
  const clearSessionError = useSessionStore((s) => s.setSessionError);
  const closeSessions = () => closePanel("sessions");


  // Bound visible dialogue history to recent turns to prevent unbounded DOM accumulation
  const visibleDialogueTurns = useMemo(() => {
    return dialogueHistory.slice(-10);
  }, [dialogueHistory]);

  const scrubberChapters = useMemo<Chapter[]>(() => {
    // Show latest conversation turns in chronological order (oldest at top, latest at bottom).
    // Bound to the 15 most recent turns so scrubber ticks fit comfortably within the 75% orb height cap.
    const recent = dialogueHistory.slice(-15);
    const startIdx = Math.max(0, dialogueHistory.length - 15);
    return recent.map((turn, relIdx) => {
      const absIdx = startIdx + relIdx;
      const title = turn.user ? (turn.user.length > 40 ? `${turn.user.slice(0, 40)}...` : turn.user) : `Turn ${absIdx + 1}`;
      const desc = turn.assistant || undefined;
      return {
        id: `dialogue-${turn.id ?? absIdx}`,
        title,
        description: desc,
        meta: `Turn #${absIdx + 1}`,
      };
    });
  }, [dialogueHistory]);


  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />




      {/* ── Restore ingestion pulse (single reverse-flow toward orb) ── */}
      <RestorePulse signal={restoreSignal} />

      {/* ── Restore error toast ── */}
      {restoreError && (
        <div className="absolute top-16 left-1/2 -translate-x-1/2 z-[100] pointer-events-auto">
          <div className="glass-card px-4 py-2.5 rounded-xl flex items-center gap-2.5 border border-[rgba(var(--error),0.3)] shadow-lg bg-[rgb(var(--card))]/95">
            <AlertCircle className="text-[rgb(var(--error))] shrink-0" size={16} />
            <p className="text-[12px] text-[rgb(var(--foreground))]/90 break-words select-text">
              {restoreError}
            </p>
            <button
              onClick={() => dismissRestoreError()}
              className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer"
            >
              {ERROR_BANNER_COPY.dismissButton}
            </button>
          </div>
        </div>
      )}

      {/* ── Session error banner (failed Engage) ── */}
      {sessionError && (
        <div role="alert" className="absolute top-[116px] left-1/2 -translate-x-1/2 z-[100] pointer-events-auto">
          <div className="glass-card px-4 py-2.5 rounded-xl flex items-center gap-2.5 border border-[rgba(var(--error),0.3)] shadow-lg bg-[rgb(var(--card))]/95">
            <AlertCircle className="text-[rgb(var(--error))] shrink-0" size={16} />
            <p className="text-[12px] text-[rgb(var(--foreground))]/90 break-words select-text">
              {sessionError}
            </p>
            <button
              onClick={() => clearSessionError(null)}
              className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer"
            >
              {ERROR_BANNER_COPY.dismissButton}
            </button>
          </div>
        </div>
      )}

      {/* ── Status Capsule: Fixed top 10% across desktop, adjusted for mobile header padding ── */}
      <div
        className="absolute left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 pointer-events-none"
        style={{
          top: isRealMobileDevice()
            ? "calc(50% - min(41vw, 25vh) - 112px - 22.4px)"
            : "10%",
        }}
      >
        {cpuWarning && (
          <span className="text-[11px] tracking-widest uppercase text-[rgb(var(--accent))]/70 font-semibold px-2 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20">
            Mode: {GOVERNOR_LABELS[cpuWarning.governor] || cpuWarning.governor}
          </span>
        )}
        {isTemporarySession && (
          <div className="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[10px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.15)] animate-in fade-in zoom-in-95 duration-200">
            <Ghost size={11} strokeWidth={2} />
            <span>{HOME_CONTROLS_COPY.temporary.activeBadge}</span>
          </div>
        )}
        <DynamicStatusBadge
          label={activityDisplay.label}
          orbState={activityDisplay.orbState}
          shimmer={activityDisplay.shimmer}
        />
      </div>

      {/* ── Side Dialogue Area - Right Only (All transcripts, big screens only) ── */}
      <div
        className="absolute top-[64px] bottom-[20%] right-0 flex flex-col justify-end items-center pointer-events-none hidden md:flex z-20"
        style={{ width: "clamp(240px, calc(50vw - 20vw), 360px)" }}
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
            <ActiveTranscript transcript={transcript} assistantText={assistantText} />
          </div>
        </div>
      </div>

      {/* ── Orb Stage (Centered in stage distance between top edge & EdgeNav) ── */}
      <div
        className="absolute z-10 flex items-center justify-center select-none pointer-events-none"
        style={{
          left: "50%",
          top: isRealMobileDevice() ? "calc(50% - 22.4px)" : "calc(50% - 36px)",
          transform: "translate(-50%, -50%)",
          width: isMobileScreen ? "min(82vw, 50vh)" : "min(65vw, 56vh)",
          height: isMobileScreen ? "min(82vw, 50vh)" : "min(65vw, 56vh)",
          minWidth: isMobileScreen ? 180 : 220,
          minHeight: isMobileScreen ? 180 : 220,
          maxWidth: 580,
          maxHeight: 580,
        }}
      >
        {/* Subtle dynamic ring behind orb */}
        <div
          className={cn(
            "absolute inset-0 rounded-full border border-[rgb(var(--accent))]/10 transition-all duration-1000",
            isEngaged && interactionState !== "Error" ? "scale-100 opacity-100 animate-field-pulse" : "scale-90 opacity-60"
          )}
        />
        {/* Orb directly renders on Home mount with zero loader flash */}
        <div className="relative w-full h-full flex items-center justify-center">
          <ErrorBoundary name="VoxOrb">
            <Suspense fallback={null}>
              <VoxOrb
                telemetryRef={telemetryRef}
                interactionState={interactionState}
                isSleeping={isSleeping}
                paused={isPanelOpen("sessions") || isPanelOpen("notifications") || isPanelOpen("help")}
              />
            </Suspense>
          </ErrorBoundary>
        </div>
      </div>

      {/* ── Compact & Mobile Dialogue Scrubber (Vertically centered, right edge of screen, ≤75% orb height) ── */}
      {scrubberChapters.length > 0 && (
        <div
          className="absolute z-30 pointer-events-auto md:hidden"
          style={{
            right: "max(6px, env(safe-area-inset-right, 6px))",
            top: isRealMobileDevice() ? "calc(50% - 22.4px)" : "calc(50% - 36px)",
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
            currentIndex={scrubberChapters.length - 1}
            label="Dialogue History Scrubber"
          />
        </div>
      )}

      {/* ── Bottom Controls (Positioned 60% from Sphere bottom edge & 40% from EdgeNav top edge) ── */}
      <div
        className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 z-20 flex flex-col items-center gap-3 w-full max-w-md pointer-events-auto"
        style={{
          top: isMobileScreen
            ? "calc(80% - 58px + min(82vw, 50vh, 580px) * 0.16)"
            : "calc(80% - 58px + min(65vw, 56vh, 580px) * 0.16)",
        }}
      >
        {/* Text Input Bar — replaces button cluster when open */}
        {isTextModeOpen && isEngaged ? (
          <TextInputBar
            onSubmit={submitText}
            onClose={() => setTextModeOpen(false)}
            isPlaybackMuted={isPlaybackMuted}
            onTogglePlaybackMute={togglePlaybackMute}
            isMicMuted={isMicMuted}
            onToggleMicMute={toggleMicMute}
            initialHistory={dialogueHistory.map((d) => d.user).filter(Boolean)}
          />
        ) : (
          <div className="flex items-center gap-4 relative">
            {/* Error State */}
            {interactionState === "Error" ? (
              <div className="relative flex flex-col items-center">
                <button
                  onClick={resume}
                  tabIndex={0}
                  className="flex items-center justify-center gap-2.5 px-6 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/50 bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] shadow-[0_0_24px_rgba(var(--accent),0.25)] cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105"
                  aria-label={HOME_CONTROLS_COPY.error.reconnectAriaLabel}
                  title={HOME_CONTROLS_COPY.error.reconnectTooltip + shortcutKeysSuffix("home.reconnect")}
                >
                  <RotateCcw size={20} className="transition-transform group-hover:-rotate-45" />
                  <span className="text-xs font-mono font-bold tracking-[0.2em] uppercase">
                    {HOME_CONTROLS_COPY.error.reconnectLabel}
                  </span>
                </button>
              </div>
            ) : isEngaged ? (
              /* Engaged Controls */
              <React.Fragment>
                {/* Pause / Resume */}
                <button
                  onClick={(isPaused || isSleeping) ? resume : pause}
                  tabIndex={0}
                  className={cn(
                    "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                    (isPaused || isSleeping)
                      ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--accent))]"
                  )}
                  aria-label={(isPaused || isSleeping) ? HOME_CONTROLS_COPY.passive.resumeAriaLabel : HOME_CONTROLS_COPY.passive.pauseAriaLabel}
                  title={(isPaused || isSleeping) ? HOME_CONTROLS_COPY.passive.resumeTooltip + shortcutKeysSuffix("home.pause-resume") : HOME_CONTROLS_COPY.passive.pauseTooltip + shortcutKeysSuffix("home.pause-resume")}
                >
                  {(isPaused || isSleeping) ? <Play size={28} /> : <Pause size={28} />}
                </button>

                {/* PTT Mic Button */}
                {interactionMode === "PTT" && (
                  <button
                    onPointerDown={() => { void handlePttStart(); }}
                    onPointerUp={() => { void handlePttStop(); }}
                    onPointerLeave={() => { if (pttStatus === "RECORDING") void handlePttCancel(); }}
                    disabled={isPaused || isSleeping}
                    tabIndex={isPaused || isSleeping ? -1 : 0}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                      pttStatus === "RECORDING"
                        ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                        : "text-[rgb(var(--accent))]",
                      (isPaused || isSleeping) && "opacity-40 cursor-not-allowed hover:bg-transparent hover:scale-100"
                    )}
                    aria-label={HOME_CONTROLS_COPY.ptt.micAriaLabel}
                    title={HOME_CONTROLS_COPY.ptt.micTooltip + shortcutKeysSuffix("home.ptt")}
                  >
                    <Mic size={28} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
                  </button>
                )}

                {/* Text Mode Toggle */}
                <button
                  onClick={() => setTextModeOpen(true)}
                  tabIndex={0}
                  className="flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]"
                  aria-label={HOME_CONTROLS_COPY.textMode.toggleAriaLabel}
                  title={HOME_CONTROLS_COPY.textMode.toggleTooltip + shortcutKeysSuffix("home.text-mode")}
                >
                  <Keyboard size={26} />
                </button>

                {/* Disengage */}
                <div className="relative flex flex-col items-center">
                  <button
                    onClick={disengage}
                    tabIndex={0}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                      isThinking && "engage-btn-loading border-transparent",
                      isLaunching && "animate-spin"
                    )}
                    disabled={isLaunching}
                    aria-label={HOME_CONTROLS_COPY.engage.stopAriaLabel}
                  >
                    {isLaunching ? (
                      <Power size={28} className="animate-pulse-slow" />
                    ) : (
                      <X size={28} />
                    )}
                  </button>
                </div>
              </React.Fragment>
            ) : (
              /* Idle: Engage */
              <React.Fragment>
                {/* Engage */}
                <div className="relative flex flex-col items-center">
                  <button
                    onClick={isLaunching ? cancelLaunch : engage}
                    tabIndex={0}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                      isLaunching && "animate-spin border-[rgb(var(--accent))]/60"
                    )}
                    aria-label={isLaunching ? "Cancel launching session" : HOME_CONTROLS_COPY.engage.ariaLabel}
                    title={isLaunching ? "Tap to cancel" : undefined}
                  >
                    {isLaunching ? (
                      <X size={28} className="animate-pulse-slow text-[rgb(var(--error))]" />
                    ) : (
                      <Power size={28} className="transition-transform duration-700" />
                    )}
                  </button>
                </div>
              </React.Fragment>
            )}
          </div>
        )}
      </div>

      {/* ── Left Edge Rail (Conversations) ── */}
      <EdgePanel side="left" open={isPanelOpen("sessions")} onClose={closeSessions} minimalHeader>
        <ErrorBoundary name="SessionPanel">
          <SessionPanel onClose={closeSessions} />
        </ErrorBoundary>
      </EdgePanel>
    </div>
  );
});

Home.displayName = "Home";

