import React, { memo, useCallback } from "react";
import { useLocation } from "react-router-dom";
import { PanelLeft, Bell, HelpCircle, Ghost } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ThemeToggleButton } from "@/shared/ui/ThemeToggleButton";
import { RestoreDefaultsButton } from "@/shared/components/settings/RestoreDefaultsButton";
import { RocketButton } from "@/shared/components/remote/RocketButton";
import { ActiveSessionHeader } from "@/shared/components/home/ActiveSessionHeader";
import { usePanelStateContext } from "@/shared/context/PanelStateContext";
import { useNotificationStore, selectBadgeCount } from "@/store/notificationStore";
import { useSessionStore } from "@/store/sessionStore";
import { useSettingsStore } from "@/store/settingsStore";
import { useVoiceSessionActions } from "@/shared/context/VoiceSessionContext";
import { useViewportResize } from "@/shared/hooks/useViewportResize";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { HOME_CONTROLS_COPY } from "@/data/homeCopy";
import { SESSION_COPY } from "@/data/sessionCopy";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { cn } from "@/shared/lib/utils";

export const AppHeader: React.FC = memo(() => {
  const location = useLocation();
  const pathname = location.pathname;

  const isHome = pathname === "/";
  const isHistory = pathname === "/history";
  const isMemory = pathname === "/memory";
  const isSettings = pathname === "/settings";
  const isMonitoring = pathname === "/monitoring";

  const { isMobile, isCompact, isNarrow } = useViewportResize();

  const { togglePanel, isPanelOpen, closePanel, openPanel, rightPanel } = usePanelStateContext();
  const unreadCount = useNotificationStore(selectBadgeCount);

  const isPrivateModeSetting = useSettingsStore((s) => s.settings?.memory?.private_mode ?? false);
  const sessionIsTemporary = useSessionStore((s) => s.isTemporarySession);
  const isTemporarySession = isPrivateModeSetting || sessionIsTemporary;
  const interactionState = useSessionStore((s) => s.interactionState);
  const isIdle = interactionState === "Idle";
  const { toggleTemporarySession: triggerToggleTemporarySession } = useVoiceSessionActions();

  const sessionsOpen = isPanelOpen("sessions");
  const isNotifsOpen = isPanelOpen("notifications");
  const isHelpOpen = isPanelOpen("help");

  // Suppress clusters on narrow widths (< 480px) when opposing panel is active
  const hideLeftCluster = Boolean(rightPanel) && isNarrow;
  const hideRightCluster = Boolean(sessionsOpen) && isNarrow;

  const toggleTemporarySession = useCallback(() => {
    if (!isIdle) return;
    void triggerToggleTemporarySession();
  }, [isIdle, triggerToggleTemporarySession]);

  // Monitoring page uses its own dedicated self-contained header in both popover and full page modes
  if (isMonitoring) {
    return null;
  }

  return (
    <header
      className={cn(
        "relative w-full z-40 shrink-0 select-none transition-all flex items-center justify-between",
        isMobile
          ? "pt-4 px-4 sm:px-8 pb-2.5 bg-transparent"
          : "h-14 px-5 bg-transparent"
      )}
    >
      {/* ── Left Slot ── */}
      <div
        className={cn(
          "flex items-center gap-2.5 min-w-0 transition-opacity duration-200 z-50",
          hideLeftCluster ? "opacity-0 pointer-events-none invisible" : "pointer-events-auto"
        )}
      >
        {/* Home & Memory get session rails across all layouts */}
        {(isHome || isMemory) && (
          <Tooltip label={SESSION_COPY.railTitle} shortcutId="global.sessions" side="bottom">
            <button
              type="button"
              onClick={() => togglePanel("sessions")}
              aria-label={SESSION_COPY.openRailAriaLabel}
              aria-expanded={sessionsOpen}
              data-edge-trigger="left"
              className={cn(
                "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                sessionsOpen
                  ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)]"
                  : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
              )}
            >
              <PanelLeft size={14} strokeWidth={1.75} />
            </button>
          </Tooltip>
        )}

        {/* Home Active Session Breadcrumb */}
        {isHome && (
          <ActiveSessionHeader
            panelOpen={sessionsOpen}
            isHome={true}
            onOpenPanel={() => togglePanel("sessions")}
          />
        )}

        {/* Home Temporary Session Button (positioned next to session trigger/breadcrumb) */}
        {isHome && (
          <Tooltip
            label={
              isIdle
                ? HOME_CONTROLS_COPY.temporary.toggleTooltip
                : "Temporary session toggle only available when idle"
            }
            side="bottom"
          >
            <button
              type="button"
              onClick={toggleTemporarySession}
              disabled={!isIdle}
              aria-pressed={isTemporarySession}
              data-edge-trigger="left"
              className={cn(
                "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-colors shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
                !isIdle
                  ? "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.25)] text-[rgb(var(--foreground-muted))]/40 opacity-50 cursor-not-allowed pointer-events-none"
                  : isTemporarySession
                  ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.18)] cursor-pointer"
                  : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] cursor-pointer"
              )}
              aria-label={HOME_CONTROLS_COPY.temporary.toggleAriaLabel}
            >
              <Ghost size={14} strokeWidth={1.75} className={cn(isTemporarySession && "text-[rgb(var(--accent))]")} />
            </button>
          </Tooltip>
        )}

        {/* Remote Control Rocket Button */}
        {isHome && <RocketButton />}


        {/* History Title in compact & mobile */}
        {isHistory && (isCompact || isMobile) && (
          <div className="flex flex-col min-w-0">
            <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))] truncate">
              History
            </h1>
            <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider truncate">
              Chronological Log
            </span>
          </div>
        )}

        {/* Settings: Restore Defaults in left corner for full layouts */}
        {isSettings && !isCompact && !isMobile && <RestoreDefaultsButton />}

        {/* Settings Title in compact & mobile */}
        {isSettings && (isCompact || isMobile) && (
          <div className="flex flex-col min-w-0">
            <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))] truncate">
              Settings
            </h1>
            <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider truncate">
              System Configuration
            </span>
          </div>
        )}

      </div>

      {/* ── Center Slot (Reserved for Main layout search in History and Memory, exactly centered at 50vw) ── */}
      <div
        id="app-header-center-slot"
        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex justify-center items-center pointer-events-none z-30 w-full max-w-[min(320px,calc(100vw-280px))]"
      />

      {/* ── Right Slot (Unified: Context Action + Theme + Notifications + Help) ── */}
      <div
        className={cn(
          "flex items-center gap-1.5 shrink-0 z-50 transition-opacity duration-200",
          hideRightCluster ? "opacity-0 pointer-events-none invisible" : "pointer-events-auto"
        )}
      >

        {/* Context Action 2: Settings Reset Settings Defaults Button (left of theme) — small/mobile layouts only */}
        {isSettings && (isCompact || isMobile) && <RestoreDefaultsButton />}

        {/* Standard Theme Toggle */}
        <ThemeToggleButton />

        {/* Standard Notifications Bell */}
        <Tooltip label={NOTIFICATION_COPY.title} shortcutId="global.notifications" side="bottom">
          <button
            type="button"
            onClick={() => togglePanel("notifications")}
            aria-expanded={isNotifsOpen}
            disabled={isHelpOpen}
            data-edge-trigger="right"
            className={cn(
              "relative inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-colors shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              isNotifsOpen
                ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] cursor-pointer"
                : isHelpOpen
                ? "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.25)] text-[rgb(var(--foreground-muted))]/60 opacity-70 cursor-not-allowed pointer-events-none"
                : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] cursor-pointer"
            )}
            aria-label={NOTIFICATION_COPY.bellAriaLabel}
          >
            <Bell size={14} strokeWidth={1.75} />
            {unreadCount > 0 && (
              <span className="absolute -top-1 -right-1 min-w-[16px] h-4 px-1 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-mono text-[10px] font-black leading-none flex items-center justify-center">
                {unreadCount > 99 ? "99+" : unreadCount}
              </span>
            )}
          </button>
        </Tooltip>

        {/* Standard Help Popover */}
        <Tooltip label={LAYOUT_COPY.helpAria} shortcutId="global.help" side="bottom">
          <button
            type="button"
            onClick={() => (isHelpOpen ? closePanel("help") : openPanel("help"))}
            aria-expanded={isHelpOpen}
            disabled={isNotifsOpen}
            data-edge-trigger="right"
            className={cn(
              "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-colors shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              isHelpOpen
                ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] cursor-pointer"
                : isNotifsOpen
                ? "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.25)] text-[rgb(var(--foreground-muted))]/60 opacity-70 cursor-not-allowed pointer-events-none"
                : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] cursor-pointer"
            )}
            aria-label={LAYOUT_COPY.helpAria}
          >
            <HelpCircle size={14} strokeWidth={1.75} />
          </button>
        </Tooltip>
      </div>
    </header>
  );
});

AppHeader.displayName = "AppHeader";
