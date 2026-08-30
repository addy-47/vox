import React, { memo, useMemo } from "react";
import { VoxOrb, PipelineField, StatusCapsule, TestClipsPopover } from "@/shared/components/home";
import { ActiveTranscript } from "@/shared/components/home/ActiveTranscript";
import { ErrorBoundary } from "@/shared/components/common";
import { GOVERNOR_LABELS } from "@/data/homeCopy";
import { Power, Mic, FlaskConical, Play, Pause, X, AlertCircle } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { AnimatePresence, motion } from "framer-motion";
import {
  useHomePage,
  toStatusLabel,
  isDotActive,
} from "@/shared/hooks/useHomePage";
import ReactMarkdown from "react-markdown";

const MarkdownComponents = {
  h1: ({node, ...props}: any) => <h1 className="text-[13px] font-bold mt-1 mb-0.5" {...props} />,
  h2: ({node, ...props}: any) => <h2 className="text-[13px] font-bold mt-1 mb-0.5" {...props} />,
  h3: ({node, ...props}: any) => <h3 className="text-[12px] font-bold mt-1 mb-0.5" {...props} />,
  p: ({node, ...props}: any) => <p className="mb-1 last:mb-0 inline-block w-full" {...props} />,
  ul: ({node, ...props}: any) => <ul className="list-disc list-inside mb-1 pl-1" {...props} />,
  ol: ({node, ...props}: any) => <ol className="list-decimal list-inside mb-1 pl-1" {...props} />,
  li: ({node, ...props}: any) => <li className="ml-0" {...props} />,
  code: ({node, ...props}: any) => <code className="bg-[rgba(var(--foreground),0.06)] px-1 rounded font-mono text-[12px]" {...props} />,
};

const DialogueTurn = memo(({ turn }: { turn: { user: string; assistant: string; id: number } }) => (
  <React.Fragment>
    {turn.user && (
      <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--foreground-muted))] font-normal text-[13px] leading-relaxed prose prose-invert select-text p-3 rounded-2xl bg-[rgb(var(--card))]/80 border border-[rgba(var(--border),0.12)]">
        <span className="text-[11px] tracking-widest text-[rgb(var(--foreground-muted))] uppercase block mb-1 font-bold">
          USER
        </span>
        <ReactMarkdown components={MarkdownComponents}>{turn.user}</ReactMarkdown>
      </div>
    )}
    {turn.assistant && (
      <div className="w-full max-w-[280px] break-words text-left text-[rgb(var(--accent))] font-medium text-[13px] leading-relaxed prose prose-invert select-text p-3 rounded-2xl bg-[rgb(var(--card))]/90 border border-[rgba(var(--accent),0.2)]">
        <span className="text-[11px] tracking-widest text-[rgb(var(--accent))]/80 uppercase block mb-1 font-bold">
          VOX
        </span>
        <ReactMarkdown components={MarkdownComponents}>{turn.assistant}</ReactMarkdown>
      </div>
    )}
  </React.Fragment>
));
DialogueTurn.displayName = "DialogueTurn";

export const Home = memo(() => {
  const navigate = useNavigate();
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
    testMode,
    setTestMode,
    testingClip,
    dialogueHistory,
    telemetryRef,
    errorAlert,
    setErrorAlert,
    dialogueScrollRef,
    isLaunching,
    isThinking,
    isMobileScreen,
    testButtonRef,
    testPanelRef,
    engage,
    disengage,
    pause,
    resume,
    handlePttStart,
    handlePttStop,
    handlePttCancel,
    handleTestClip,
  } = useHomePage();

  // Test-clip menu participates in the global overlay stack (Escape / outside-click).
  useOverlay({
    onClose: () => setTestMode(false),
    ref: testPanelRef,
    dismissOnOutside: true,
    active: testMode && !isEngaged,
  });

  const statusLabel = toStatusLabel(
    interactionState,
    isEngaged,
    isSleeping,
    pttStatus,
    isPaused
  );
  const dotActive = isDotActive(isEngaged, interactionState, pttStatus, isSleeping);

  // Bound visible dialogue history to recent turns to prevent unbounded DOM accumulation
  const visibleDialogueTurns = useMemo(() => {
    return dialogueHistory.slice(-10);
  }, [dialogueHistory]);

  const isPttActive = isEngaged && !testingClip && interactionMode === "PTT" && !isPaused;

  return (
    <div className="relative flex-1 flex flex-col items-center justify-between h-full w-full overflow-hidden bg-transparent select-none">
      {/* Sentient Field Background Energy */}
      <PipelineField state={interactionState} />

      {/* Floating Error Toast */}
      <AnimatePresence>
        {errorAlert && (
          <motion.div
            initial={{ opacity: 0, x: 50, scale: 0.95 }}
            animate={{ opacity: 1, x: 0, scale: 1 }}
            exit={{ opacity: 0, x: 50, scale: 0.95 }}
            className="absolute top-4 left-4 right-4 md:left-auto md:right-4 z-[100] md:max-w-sm pointer-events-auto"
          >
            <div className="glass-card p-4 rounded-xl flex items-start gap-3 border border-red-500/30 shadow-2xl bg-black/40 backdrop-blur-md">
              <AlertCircle className="text-red-400 shrink-0 mt-0.5" size={18} />
              <div className="flex-1 flex flex-col gap-1.5 min-w-0">
                <span className="text-xs font-bold tracking-wider uppercase text-red-400 text-left">Connection Error</span>
                <p className="text-[12px] text-[rgb(var(--foreground))]/90 leading-relaxed font-light break-words select-text text-left">
                  {errorAlert}
                </p>
                <div className="flex gap-3 mt-1 justify-start">
                  <button
                    onClick={() => {
                      setErrorAlert(null);
                      navigate("/settings");
                    }}
                    className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    Configure Settings
                  </button>
                  <button
                    onClick={() => setErrorAlert(null)}
                    className="text-[11px] font-black uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer"
                  >
                    Dismiss
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Top-right: Status Capsule (single, clean, centered on mobile) ── */}
      <div className="absolute top-[10%] md:top-4 left-1/2 -translate-x-1/2 md:left-auto md:translate-x-0 md:right-5 z-30 flex items-center gap-2 pointer-events-none">
        {cpuWarning && (
          <span className="text-[11px] tracking-widest uppercase text-[rgb(var(--accent))]/70 font-semibold px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20">
            Mode: {GOVERNOR_LABELS[cpuWarning.governor] || cpuWarning.governor}
          </span>
        )}
        <StatusCapsule
          label={statusLabel}
          dotActive={dotActive}
          testing={!!testingClip}
        />
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
            isEngaged ? "scale-100 opacity-100 animate-field-pulse" : "scale-90 opacity-60"
          )}
        />
        <div className="relative w-full h-full flex items-center justify-center">
          <ErrorBoundary name="VoxOrb">
            <VoxOrb
              telemetryRef={telemetryRef}
              interactionState={interactionState}
              isSleeping={isSleeping}
              isTesting={!!testingClip}
            />
          </ErrorBoundary>
        </div>
      </div>

      {/* ── Bottom Controls (positioned cleanly above EdgeNav top edge at 72px) ── */}
      <div 
        className="absolute left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-4 w-full max-w-md pointer-events-auto"
        style={{
          bottom: "calc(72px + clamp(12px, 2.5vh, 28px))"
        }}
      >
        {/* Buttons */}
        <div className="flex items-center gap-4 relative">
          {/* Passive Mode Pause / Resume Button (Hidden in PTT Mode) */}
          {isEngaged && !testingClip && interactionMode !== "PTT" && (
            <button
              onClick={isPaused ? resume : pause}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95",
                isPaused
                  ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--accent))]"
              )}
              aria-label={isPaused ? "Resume Vox" : "Pause Vox"}
            >
              {isPaused ? <Play size={28} /> : <Pause size={28} />}
            </button>
          )}

          {/* PTT Hold-to-Talk Mic Button */}
          {isEngaged && !testingClip && interactionMode === "PTT" && (
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
              aria-label="Hold to Talk (Push-To-Talk)"
            >
              <Mic size={28} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
            </button>
          )}

          {/* Primary Engage / Disengage Button */}
          <div className="relative flex flex-col items-center">
            {!isEngaged && hasCachedSession && (
              <span className="absolute -top-7 text-[11px] tracking-widest text-[rgb(var(--accent))]/85 uppercase animate-pulse whitespace-nowrap bg-[rgb(var(--accent))]/5 px-2 py-0.5 rounded-full border border-[rgb(var(--accent))]/15">
                Resume Session
              </span>
            )}
            <button
              onClick={isEngaged ? disengage : engage}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95",
                isEngaged && isThinking && "engage-btn-loading border-transparent",
                isLaunching && "animate-spin",
                isEngaged
                  ? "border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/15"
                  : "bg-transparent text-[rgb(var(--accent))]"
              )}
              disabled={isLaunching}
              aria-label={isEngaged ? "Stop Vox" : (hasCachedSession ? "Resume Vox Session" : "Engage Vox")}
            >
              {isLaunching ? (
                <Power size={28} className="animate-pulse-slow" />
              ) : isEngaged ? (
                <X size={28} />
              ) : (
                <Power
                  size={28}
                  className="transition-transform duration-700"
                />
              )}
            </button>
          </div>
        </div>
      </div>

      {/* ── Test Mode — bottom-right, hidden when engaged ──────────────── */}
      <AnimatePresence>
        {!isEngaged && (
          <motion.div
            key="test-mode-container"
            initial={{ opacity: 0, scale: 0.85, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.85, y: 10 }}
            transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
            className="hidden md:block fixed bottom-4 right-4 z-50"
          >
            <button
              ref={testButtonRef}
              onClick={() => setTestMode(!testMode)}
              className={cn(
                "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 cursor-pointer glass-card",
                testMode
                  ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border-[rgb(var(--accent))]/60"
                  : "bg-transparent border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10"
              )}
              aria-label="Test Mode"
            >
              <FlaskConical size={22} />
            </button>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Test Mode Panel ──────────────── */}
      <AnimatePresence>
        {testMode && !isEngaged && (
          <TestClipsPopover
            panelRef={testPanelRef}
            onSelectClip={handleTestClip}
            onClose={() => setTestMode(false)}
            testingClip={testingClip}
          />
        )}
      </AnimatePresence>
    </div>
  );
});

Home.displayName = "Home";
