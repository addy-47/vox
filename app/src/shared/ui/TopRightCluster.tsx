import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Bell, HelpCircle } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { usePanelStateContext } from "@/shared/hooks/usePanelState";
import { useNotificationStore, selectBadgeCount } from "@/store/notificationStore";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";

interface TopRightClusterProps {
  className?: string;
}

export const TopRightCluster: React.FC<TopRightClusterProps> = memo(
  ({ className }) => {
    const { openPanel } = usePanelStateContext();
    const unreadCount = useNotificationStore(selectBadgeCount);

    return (
      <div className={cn("flex items-center gap-1.5", className)}>
        <Tooltip label={NOTIFICATION_COPY.title} side="bottom">
          <button
            onClick={() => openPanel("notifications")}
            className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
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
        <Tooltip label="Help & guide" side="bottom">
          <button
            onClick={() => openPanel("help")}
            className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
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
