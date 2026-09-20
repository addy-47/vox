import React, { useEffect, useState, useRef } from "react";
import { motion } from "framer-motion";
import { useSessionStore } from "@/store/sessionStore";
import { onSessionsChanged } from "@/services/eventsService";
import { getSessions, resolveSessionTitle } from "@/services/historyService";

interface ActiveSessionHeaderProps {
  panelOpen: boolean;
  isHome: boolean;
  onOpenPanel: () => void;
}

/**
 * Header breadcrumb displaying the current project and session title when
 * the session panel is closed on the Home page.
 *
 * Automatically types in the session title with a typewriter effect when
 * the backend persists / updates the session title while on the Home screen.
 */
export const ActiveSessionHeader: React.FC<ActiveSessionHeaderProps> = ({
  panelOpen,
  isHome,
  onOpenPanel,
}) => {
  const activeSessionLabel = useSessionStore((s) => s.activeSessionLabel);
  const setActiveSessionLabel = useSessionStore((s) => s.setActiveSessionLabel);

  const { projectName, sessionTitle } = activeSessionLabel;

  // ── Typewriter effect state ──
  const [displayedTitle, setDisplayedTitle] = useState(() => sessionTitle || "");
  const [isTyping, setIsTyping] = useState(false);
  const previousTitleRef = useRef<string | null>(sessionTitle || null);

  // Typewriter effect triggered when sessionTitle arrives or changes
  useEffect(() => {
    if (!sessionTitle) {
      setDisplayedTitle("");
      setIsTyping(false);
      previousTitleRef.current = null;
      return;
    }

    // If panel is open, keep title fully displayed in memory without typewriter
    if (panelOpen) {
      previousTitleRef.current = sessionTitle;
      setDisplayedTitle(sessionTitle);
      setIsTyping(false);
      return;
    }

    // Panel closed and title has not changed: keep full title stable
    if (previousTitleRef.current === sessionTitle) {
      setDisplayedTitle(sessionTitle);
      setIsTyping(false);
      return;
    }

    // Genuine new title arriving while on Home with panel closed: start typewriter
    previousTitleRef.current = sessionTitle;
    setDisplayedTitle("");
    setIsTyping(true);

    let currentIndex = 0;
    const interval = window.setInterval(() => {
      currentIndex++;
      setDisplayedTitle(sessionTitle.slice(0, currentIndex));
      if (currentIndex >= sessionTitle.length) {
        window.clearInterval(interval);
        setIsTyping(false);
      }
    }, 28);

    return () => window.clearInterval(interval);
  }, [sessionTitle, panelOpen]);

  // ── Listen for backend session updates to dynamically update title ──
  useEffect(() => {
    let isMounted = true;

    const unlisten = onSessionsChanged(async () => {
      const currentActiveId = useSessionStore.getState().activeSessionId;
      if (!currentActiveId || !isMounted) return;

      try {
        const sessions = await getSessions();
        if (!isMounted) return;

        const found = sessions.find((s) => s.id === currentActiveId);
        if (found && isMounted) {
          const resolved = resolveSessionTitle(found);
          const currentLabel = useSessionStore.getState().activeSessionLabel;
          if (resolved && resolved !== currentLabel.sessionTitle) {
            setActiveSessionLabel({
              projectName: currentLabel.projectName,
              sessionTitle: resolved,
            });
          }
        }
      } catch (e) {
        console.warn("[ActiveSessionHeader] Failed to refresh session title:", e);
      }
    });

    return () => {
      isMounted = false;
      unlisten();
    };
  }, [setActiveSessionLabel]);

  // Only hide when not on Home or no label at all.
  if (!isHome || (!projectName && !sessionTitle)) {
    return null;
  }

  return (
    <motion.div
      initial={false}
      animate={{
        opacity: panelOpen ? 0 : 1,
      }}
      whileHover={panelOpen ? undefined : { opacity: 0.82 }}
      transition={{ duration: 0.15, ease: "easeOut" }}
      style={{
        pointerEvents: panelOpen ? "none" : "auto",
      }}
      onClick={panelOpen ? undefined : onOpenPanel}
      onKeyDown={panelOpen ? undefined : (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onOpenPanel(); } }}
      role="button"
      tabIndex={0}
      aria-label={`${projectName ? `${projectName} — ` : ""}${sessionTitle || "Session"}`}
      className="flex flex-col gap-0 select-none cursor-pointer pl-1"
    >
      {projectName && (
        <span className="font-semibold text-[rgb(var(--accent))] truncate max-w-[200px] text-[13px] leading-tight">
          {projectName}
        </span>
      )}

      {sessionTitle && (
        <span className="font-medium text-[rgb(var(--foreground))] truncate max-w-[200px] text-[12px] leading-tight inline-flex items-center">
          <span className="truncate">{displayedTitle}</span>
          {isTyping && (
            <span className="inline-block w-1 h-3 ml-0.5 bg-[rgb(var(--accent))] animate-pulse shrink-0" />
          )}
        </span>
      )}
    </motion.div>
  );
};

