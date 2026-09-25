import React, { memo, useCallback } from "react";
import { useLocation } from "react-router-dom";
import { cn } from "@/shared/lib/utils";
import { Bell, HelpCircle } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { TemporaryChatIcon } from "@/shared/ui/TemporaryChatIcon";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { useNotificationStore, selectBadgeCount } from "@/store/notificationStore";
import { useSessionStore } from "@/store/sessionStore";
import { setSessionPrivateMode } from "@/services/pipelineService";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { HOME_CONTROLS_COPY } from "@/data/homeCopy";
import { RestoreDefaultsButton } from "@/shared/components/settings/RestoreDefaultsButton";

interface TopRightClusterProps {
  className?: string;
}

export const TopRightCluster: React.FC<TopRightClusterProps> = memo(
  ({ className }) => {
    const { togglePanel, isPanelOpen, closePanel, openPanel } = usePanelStateContext();
    const unreadCount = useNotificationStore(selectBadgeCount);
    const location = useLocation();
    const isHome = location.pathname === "/";
    const isSettings = location.pathname === "/settings";
    const isTemporarySession = useSessionStore((s) => s.isTemporarySession);

    const toggleTemporarySession = useCallback(async () => {
      const next = !useSessionStore.getState().isTemporarySession;
      useSessionStore.getState().setIsTemporarySession(next);
      try {
        await setSessionPrivateMode(next);
      } catch {
        // Best-effort IPC notification
      }
    }, []);

    const isNotifsOpen = isPanelOpen("notifications");
    const isHelpOpen = isPanelOpen("help");

    return (
      <div data-spatial-zone="cluster" className={cn("flex items-center gap-1.5", className)}>
        {isSettings && <RestoreDefaultsButton />}
        {isHome && (
          <Tooltip label={HOME_CONTROLS_COPY.temporary.toggleTooltip} side="bottom">
            <button
              onClick={toggleTemporarySession}
              aria-pressed={isTemporarySession}
              data-edge-trigger="right"
              className={cn(
                "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer",
                isTemporarySession
                  ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] hover:bg-[rgba(var(--accent),0.18)]"
                  : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
              )}
              aria-label={HOME_CONTROLS_COPY.temporary.toggleAriaLabel}
            >
              <TemporaryChatIcon size={14} strokeWidth={1.75} checked={isTemporarySession} />
            </button>
          </Tooltip>
        )}
        <Tooltip label={NOTIFICATION_COPY.title} shortcutId="global.notifications" side="bottom">
          <button
            onClick={() => togglePanel("notifications")}
            aria-expanded={isNotifsOpen}
            disabled={isHelpOpen}
            data-edge-trigger="right"
            className={cn(
              "relative inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
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
        <Tooltip label="Help & guide" shortcutId="global.help" side="bottom">
          <button
            onClick={() => (isHelpOpen ? closePanel("help") : openPanel("help"))}
            aria-expanded={isHelpOpen}
            disabled={isNotifsOpen}
            data-edge-trigger="right"
            className={cn(
              "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
              isHelpOpen
                ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.2)] cursor-pointer"
                : isNotifsOpen
                ? "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.25)] text-[rgb(var(--foreground-muted))]/60 opacity-70 cursor-not-allowed pointer-events-none"
                : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] cursor-pointer"
            )}
            aria-label="Help & guide"
          >
            <HelpCircle size={14} strokeWidth={1.75} />
          </button>
        </Tooltip>
      </div>
    );
  }
);
TopRightCluster.displayName = "TopRightCluster";
