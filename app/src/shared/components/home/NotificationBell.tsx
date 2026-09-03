import { memo, useEffect, useRef } from "react";
import { Bell } from "lucide-react";
import { AnimatePresence } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { useNotificationStore } from "@/store/notificationStore";
import { NotificationsPopover } from "./NotificationsPopover";

export const NotificationBell = memo(() => {
  const notifications = useNotificationStore((s) => s.notifications);
  const isOpen = useNotificationStore((s) => s.isOpen);
  const setIsOpen = useNotificationStore((s) => s.setIsOpen);
  const fetchNotifications = useNotificationStore((s) => s.fetchNotifications);
  const initListeners = useNotificationStore((s) => s.initListeners);

  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const unreadCount = notifications.filter((n) => n.status === "unread").length;

  useEffect(() => {
    let isMounted = true;
    let cleanupListeners: (() => void) | null = null;

    fetchNotifications();

    initListeners().then((cleanup) => {
      if (isMounted) {
        cleanupListeners = cleanup;
      } else {
        cleanup();
      }
    });

    return () => {
      isMounted = false;
      if (cleanupListeners) cleanupListeners();
    };
  }, [fetchNotifications, initListeners]);

  useEffect(() => {
    if (!isOpen) return;

    const handlePointerDown = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node) &&
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    window.addEventListener("mousedown", handlePointerDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [isOpen, setIsOpen]);

  return (
    <div ref={containerRef} className="relative inline-flex items-center pointer-events-auto">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        aria-label={NOTIFICATION_COPY.bellAriaLabel}
        className={cn(
          "relative p-2 rounded-xl transition-all duration-200 border cursor-pointer flex items-center justify-center",
          isOpen
            ? "border-[rgba(var(--accent),0.4)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] shadow-[0_0_15px_rgba(var(--accent),0.15)]"
            : "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
        )}
      >
        <Bell size={16} strokeWidth={1.75} className="shrink-0" />

        {unreadCount > 0 && (
          <span className="absolute -top-1 -right-1 min-w-[16px] h-4 px-1 rounded-full bg-[rgb(var(--accent))] text-black font-mono text-[10px] font-black leading-none flex items-center justify-center shadow-sm">
            {unreadCount > 99 ? "99+" : unreadCount}
          </span>
        )}
      </button>

      <AnimatePresence>
        {isOpen && (
          <NotificationsPopover
            panelRef={popoverRef}
            onClose={() => setIsOpen(false)}
          />
        )}
      </AnimatePresence>
    </div>
  );
});

NotificationBell.displayName = "NotificationBell";
