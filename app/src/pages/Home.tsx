import React, { memo, useMemo } from "react";
import { VoxOrb, PipelineField, StatusCapsule, RestorePulse, SessionPanel } from "@/shared/components/home";
import { TextInputBar } from "@/shared/components/home/TextInputBar";
import { ActiveTranscript } from "@/shared/components/home/ActiveTranscript";
import { ErrorBoundary } from "@/shared/components/common";
import { EdgePanel } from "@/shared/ui";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import {
  GOVERNOR_LABELS,
  HOME_CONTROLS_COPY,
  ERROR_BANNER_COPY,
  DIALOGUE_COPY,
} from "@/data/homeCopy";

import { Power, Mic, Keyboard, Play, Pause, X, AlertCircle, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";


import {
  useHomePage,
  toStatusLabel,
  isDotActive,
} from "@/shared/hooks/useHomePage";
import { Markdown } from "@/shared/ui/Markdown";

const DialogueTurn = memo(({ turn }: { turn: { user: string; assistant: string; id: number } }) => (
  <React.Fragment>
    {turn.user && (
      <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--foreground-muted))] font-normal text-[13px] leading-relaxed select-text p-3 rounded-2xl bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.12)]">
        <span className="text-[11px] tracking-widest text-[rgb(var(--foreground-muted))] uppercase block mb-1 font-bold">
          {DIALOGUE_COPY.userBadge}
        </span>
        <Markdown content={turn.user} variant="bubble" />
      </div>
    )}
    {turn.assistant && (
      <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed select-text p-3 rounded-2xl bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.2)]">
        <span className="text-[11px] tracking-widest text-[rgb(var(--accent))]/80 uppercase block mb-1 font-bold">
          {DIALOGUE_COPY.assistantBadge}
        </span>
        <Markdown content={turn.assistant} variant="bubble" />
      </div>
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
    hasCachedSession,
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
    dialogueScrollRef,
    isLaunching,
    isThinking,
    isMobileScreen,
    engage,
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
  } = useHomePage();

  const { isPanelOpen, closePanel } = usePanelStateContext();
  const closeSessions = () => closePanel("sessions");

  const statusLabel = toStatusLabel(
    interactionState,
    isEngaged,
    isSleeping,
    isPaused
  );
  const dotActive = isDotActive(isEngaged, interactionState, isSleeping);

  // Bound visible dialogue history to recent turns to prevent unbounded DOM accumulation
  const visibleDialogueTurns = useMemo(() => {
    return dialogueHistory.slice(-10);
  }, [dialogueHistory]);

  const isPttActive = isEngaged && interactionMode === "PTT" && !isPaused && interactionState !== "Error";

  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />




      {/* ── Restore ingestion pulse (single reverse-flow toward orb) ── */}
      <RestorePulse signal={restoreSignal} />

      {/* ── Restore error toast ── */}
      {restoreError && (
        <div className="absolute top-16 left-1/2 -translate-x-1/2 z-[100] pointer-events-auto">
          <div className="glass-card px-4 py-2.5 rounded-xl flex items-center gap-2.5 border border-red-500/30 shadow-2xl bg-black/40 backdrop-blur-md">
            <AlertCircle className="text-red-400 shrink-0" size={16} />
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

      {/* ── Status Capsule: Centered directly above the Orb (matching mobile on all viewports) ── */}
      <div className="absolute top-[10%] left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 pointer-events-none">
        {cpuWarning && (
          <span className="text-[11px] tracking-widest uppercase text-[rgb(var(--accent))]/70 font-semibold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20">
            Mode: {GOVERNOR_LABELS[cpuWarning.governor] || cpuWarning.governor}
          </span>
        )}
        {isTemporarySession && !isEngaged ? (
          <div
            role="status"
            aria-live="polite"
            aria-label="Vox Status: Temporary"
            className="flex items-center gap-2 px-3 py-1.5 rounded-full border border-[rgba(var(--border),0.25)] bg-[rgb(var(--foreground-muted))]/[0.06] dark:bg-[rgba(10,12,14,0.40)] dark:backdrop-blur-md pointer-events-none grayscale"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--foreground-muted))] opacity-30" />
            <span className="text-[11px] font-mono font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {HOME_CONTROLS_COPY.temporary.activeBadge}
            </span>
          </div>
        ) : (
          <StatusCapsule
            label={statusLabel}
            dotActive={dotActive}
          />
        )}
      </div>

      {/* ── Side Dialogue Area - Right Only (All transcripts, big screens only) ── */}
      <div
        className="absolute top-[64px] bottom-[20%] right-0 flex flex-col justify-end items-center pointer-events-none hidden md:flex z-20"
        style={{ width: "clamp(240px, calc(50vw - 20vw), 360px)" }}
      >
        <div
          ref={dialogueScrollRef}
          className="w-full max-h-[85%] overflow-y-auto scrollbar-none flex flex-col items-center gap-4 pointer-events-auto select-text px-4 pb-6"
          style={{
            maskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
            WebkitMaskImage: "linear-gradient(to bottom, transparent 0%, black 15%, black 85%, transparent 100%)",
          }}
        >
          <div className="flex-1 min-h-[4vh]" />
          {/* Dialogue History */}
          {visibleDialogueTurns.map((turn: { user: string; assistant: string; id: number }) => (
            <DialogueTurn key={turn.id} turn={turn} />
          ))}

          {/* Isolated Active Streaming Transcript */}
          <ActiveTranscript transcript={transcript} assistantText={assistantText} />
        </div>
      </div>

      {/* ── Orb Stage (Vertically centered in stage distance between top edge & EdgeNav) ── */}
      <div
        className={cn(
          "absolute z-10 overflow-hidden flex items-center justify-center select-none",
          isPttActive ? "pointer-events-auto cursor-pointer" : "pointer-events-none"
        )}
        onPointerDown={isPttActive ? () => handlePttStart() : undefined}
        onPointerUp={isPttActive ? () => handlePttStop() : undefined}
        onPointerLeave={isPttActive ? () => { if (pttStatus === "RECORDING") handlePttCancel(); } : undefined}
        style={{
          left: "50%",
          top: "calc(50% - 36px)",
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
        <div className="relative w-full h-full flex items-center justify-center">
          <ErrorBoundary name="VoxOrb">
            <VoxOrb
              telemetryRef={telemetryRef}
              interactionState={interactionState}
              isSleeping={isSleeping}
              isTesting={false}
            />
          </ErrorBoundary>
        </div>
      </div>

      {/* ── Bottom Controls ── */}
      <div
        className="absolute left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-3 w-full max-w-md pointer-events-auto"
        style={{ bottom: "calc(72px + clamp(12px, 2.5vh, 28px))" }}
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
          />
        ) : (
          <div className="flex items-center gap-4 relative">
            {/* Error State */}
            {interactionState === "Error" ? (
              <div className="relative flex flex-col items-center">
                <button
                  onClick={resume}
                  className="flex items-center justify-center gap-2.5 px-6 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/50 bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] shadow-[0_0_24px_rgba(var(--accent),0.25)] cursor-pointer"
                  aria-label={HOME_CONTROLS_COPY.error.reconnectAriaLabel}
                  title={HOME_CONTROLS_COPY.error.reconnectTooltip}
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
                  onClick={isPaused ? resume : pause}
                  className={cn(
                    "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer",
                    isPaused
                      ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--accent))]"
                  )}
                  aria-label={isPaused ? HOME_CONTROLS_COPY.passive.resumeAriaLabel : HOME_CONTROLS_COPY.passive.pauseAriaLabel}
                  title={isPaused ? HOME_CONTROLS_COPY.passive.resumeTooltip : HOME_CONTROLS_COPY.passive.pauseTooltip}
                >
                  {isPaused ? <Play size={28} /> : <Pause size={28} />}
                </button>

                {/* PTT Mic Button */}
                {interactionMode === "PTT" && (
                  <button
                    onPointerDown={() => handlePttStart()}
                    onPointerUp={() => handlePttStop()}
                    onPointerLeave={() => { if (pttStatus === "RECORDING") handlePttCancel(); }}
                    disabled={isPaused}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer",
                      pttStatus === "RECORDING"
                        ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                        : "text-[rgb(var(--accent))]",
                      isPaused && "opacity-40 cursor-not-allowed hover:bg-transparent hover:scale-100"
                    )}
                    aria-label={HOME_CONTROLS_COPY.ptt.micAriaLabel}
                    title={HOME_CONTROLS_COPY.ptt.micTooltip}
                  >
                    <Mic size={28} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
                  </button>
                )}

                {/* Text Mode Toggle */}
                <button
                  onClick={() => setTextModeOpen(true)}
                  className="flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer"
                  aria-label={HOME_CONTROLS_COPY.textMode.toggleAriaLabel}
                  title={HOME_CONTROLS_COPY.textMode.toggleTooltip}
                >
                  <Keyboard size={26} />
                </button>

                {/* Disengage */}
                <div className="relative flex flex-col items-center">
                  <button
                    onClick={disengage}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 hover:scale-105 active:scale-95 cursor-pointer",
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
                  {hasCachedSession && (
                    <span className="absolute -top-7 text-[11px] tracking-widest text-[rgb(var(--accent))]/85 uppercase animate-pulse whitespace-nowrap bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded-full border border-[rgb(var(--accent))]/15">
                      {HOME_CONTROLS_COPY.engage.resumeBadge}
                    </span>
                  )}
                  <button
                    onClick={engage}
                    className={cn(
                      "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer",
                      isLaunching && "animate-spin"
                    )}
                    disabled={isLaunching}
                    aria-label={hasCachedSession ? HOME_CONTROLS_COPY.engage.resumeAriaLabel : HOME_CONTROLS_COPY.engage.ariaLabel}
                  >
                    {isLaunching ? (
                      <Power size={28} className="animate-pulse-slow" />
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

