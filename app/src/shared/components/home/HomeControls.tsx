import React, { memo } from "react";
import { Power, Mic, Keyboard, Play, Pause, X, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { HOME_CONTROLS_COPY } from "@/data/homeCopy";
import { shortcutKeysSuffix } from "@/data/shortcuts";
import { useRemoteGuard } from "@/shared/hooks/useRemoteGuard";
import { Tooltip } from "@/shared/ui/Tooltip";

export interface HomeControlsProps {
  restoreError: unknown;
  dismissRestoreError: () => void;
  isEngaged: boolean;
  isSleeping: boolean;
  isPaused: boolean;
  interactionMode: string;
  pttStatus: string;
  isLaunching: boolean;
  isThinking: boolean;
  pause: () => void;
  resume: () => void;
  handlePttStart: () => Promise<void>;
  handlePttStop: () => Promise<void>;
  setTextModeOpen: (open: boolean) => void;
  disengage: () => void;
  engage: () => void;
  cancelLaunch: () => void;
}

export const HomeControls = memo(({
  restoreError,
  dismissRestoreError,
  isEngaged,
  isSleeping,
  isPaused,
  interactionMode,
  pttStatus,
  isLaunching,
  isThinking,
  pause,
  resume,
  handlePttStart,
  handlePttStop,
  setTextModeOpen,
  disengage,
  engage,
  cancelLaunch,
}: HomeControlsProps) => {
  const { isBlocked, guardProps, tooltip } = useRemoteGuard();

  return (
    <div className="flex items-center justify-center gap-6 z-20">
      {restoreError ? (
        /* Error Recovery State */
        <div className="flex flex-col items-center gap-3">
          <Tooltip label={HOME_CONTROLS_COPY.error.reconnectTooltip + shortcutKeysSuffix("home.reconnect")} side="top">
            <button
              onClick={dismissRestoreError}
              className="flex items-center gap-2 px-6 py-3 rounded-full bg-[rgba(var(--foreground),0.06)] hover:bg-[rgba(var(--foreground),0.12)] text-[rgb(var(--foreground))] border border-[rgba(var(--foreground),0.15)] transition-all cursor-pointer group"
              aria-label={HOME_CONTROLS_COPY.error.reconnectAriaLabel}
            >
              <RotateCcw size={20} className="transition-transform group-hover:-rotate-45" />
              <span className="text-xs font-mono font-bold tracking-[0.2em] uppercase">
                {HOME_CONTROLS_COPY.error.reconnectLabel}
              </span>
            </button>
          </Tooltip>
        </div>
      ) : isEngaged ? (
        /* Engaged Controls */
        <React.Fragment>
          {/* Pause / Resume */}
          <Tooltip label={tooltip ?? ((isPaused || isSleeping) ? HOME_CONTROLS_COPY.passive.resumeTooltip + shortcutKeysSuffix("home.pause-resume") : HOME_CONTROLS_COPY.passive.pauseTooltip + shortcutKeysSuffix("home.pause-resume"))} side="top">
            <button
              onClick={isBlocked ? undefined : ((isPaused || isSleeping) ? resume : pause)}
              disabled={isBlocked}
              tabIndex={isBlocked ? -1 : 0}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                (isPaused || isSleeping)
                  ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--accent))]",
                guardProps.className
              )}
              aria-label={(isPaused || isSleeping) ? HOME_CONTROLS_COPY.passive.resumeAriaLabel : HOME_CONTROLS_COPY.passive.pauseAriaLabel}
            >
              {(isPaused || isSleeping) ? <Play size={28} /> : <Pause size={28} />}
            </button>
          </Tooltip>

          {/* PTT Mic Button */}
          {interactionMode === "PTT" && (
            <Tooltip label={tooltip ?? (HOME_CONTROLS_COPY.ptt.micTooltip + shortcutKeysSuffix("home.ptt"))} side="top">
              <button
                onPointerDown={(e) => {
                  if (isBlocked) return;
                  e.currentTarget.setPointerCapture(e.pointerId);
                  void handlePttStart();
                }}
                onPointerUp={(e) => {
                  if (isBlocked) return;
                  try {
                    e.currentTarget.releasePointerCapture(e.pointerId);
                  } catch {
                    // ignore if already released
                  }
                  void handlePttStop();
                }}
                disabled={isBlocked || isPaused || isSleeping}
                tabIndex={isBlocked || isPaused || isSleeping ? -1 : 0}
                className={cn(
                  "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                  pttStatus === "RECORDING"
                    ? "bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))]"
                    : "text-[rgb(var(--accent))]",
                  (isPaused || isSleeping) && "opacity-40 cursor-not-allowed hover:bg-transparent hover:scale-100",
                  guardProps.className
                )}
                aria-label={HOME_CONTROLS_COPY.ptt.micAriaLabel}
              >
                <Mic size={28} className={cn(pttStatus === "RECORDING" && "animate-pulse-slow")} />
              </button>
            </Tooltip>
          )}

          {/* Text Mode Toggle */}
          <Tooltip label={tooltip ?? (HOME_CONTROLS_COPY.textMode.toggleTooltip + shortcutKeysSuffix("home.text-mode"))} side="top">
            <button
              onClick={isBlocked ? undefined : () => setTextModeOpen(true)}
              disabled={isBlocked}
              tabIndex={isBlocked ? -1 : 0}
              className={cn(
                "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                guardProps.className
              )}
              aria-label={HOME_CONTROLS_COPY.textMode.toggleAriaLabel}
            >
              <Keyboard size={26} />
            </button>
          </Tooltip>

          {/* Disengage */}
          <div className="relative flex flex-col items-center">
            <Tooltip label={tooltip ?? HOME_CONTROLS_COPY.engage.stopAriaLabel} side="top">
              <button
                onClick={isBlocked ? undefined : disengage}
                tabIndex={isBlocked ? -1 : 0}
                className={cn(
                  "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/15 hover:bg-[rgb(var(--accent))]/25 hover:scale-105 active:scale-95 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                  isThinking && "engage-btn-loading border-transparent",
                  isLaunching && "animate-spin",
                  guardProps.className
                )}
                disabled={isBlocked || isLaunching}
                aria-disabled={guardProps["aria-disabled"]}
                aria-label={HOME_CONTROLS_COPY.engage.stopAriaLabel}
              >
                {isLaunching ? (
                  <Power size={28} className="animate-pulse-slow" />
                ) : (
                  <X size={28} />
                )}
              </button>
            </Tooltip>
          </div>
        </React.Fragment>
      ) : (
        /* Idle: Engage */
        <React.Fragment>
          {/* Engage */}
          <div className="relative flex flex-col items-center">
            <Tooltip label={tooltip ?? (isLaunching ? "Tap to cancel" : HOME_CONTROLS_COPY.engage.ariaLabel)} side="top">
              <button
                onClick={isBlocked ? undefined : (isLaunching ? cancelLaunch : engage)}
                tabIndex={isBlocked ? -1 : 0}
                className={cn(
                  "flex items-center justify-center w-14 h-14 rounded-full transition-all duration-500 border border-[rgb(var(--accent))]/25 bg-transparent hover:bg-[rgb(var(--accent))]/10 hover:scale-105 active:scale-95 text-[rgb(var(--accent))] cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))] focus-visible:ring-offset-2 focus-visible:ring-offset-[rgb(var(--background))] focus-visible:scale-105 focus-visible:shadow-[0_0_24px_rgba(var(--accent),0.4)]",
                  isLaunching && "animate-spin border-[rgb(var(--accent))]/60",
                  guardProps.className
                )}
                disabled={guardProps.disabled}
                aria-disabled={guardProps["aria-disabled"]}
                aria-label={isLaunching ? "Cancel launching session" : HOME_CONTROLS_COPY.engage.ariaLabel}
              >
                {isLaunching ? (
                  <X size={28} className="animate-pulse-slow text-[rgb(var(--error))]" />
                ) : (
                  <Power size={28} className="transition-transform duration-700" />
                )}
              </button>
            </Tooltip>
          </div>
        </React.Fragment>
      )}
    </div>
  );
});

HomeControls.displayName = "HomeControls";
