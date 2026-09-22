import React, { useEffect, useState, useRef } from "react";
import { motion } from "framer-motion";
import { useSessionStore } from "@/store/sessionStore";
import { onSessionsChanged } from "@/services/eventsService";
import { getSessions, resolveSessionTitle } from "@/services/historyService";
import { getProjects } from "@/services/projectService";
import { getRuntimeSnapshot } from "@/services/pipelineService";

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

  // ── Listen for backend session updates to dynamically update title and project ──
  useEffect(() => {
    let isMounted = true;

    const unlisten = onSessionsChanged(async () => {
      let currentActiveId = useSessionStore.getState().activeSessionId;
      if (!currentActiveId) {
        try {
          const snap = await getRuntimeSnapshot();
          if (snap && snap.conversation_id > 0) {
            currentActiveId = snap.conversation_id;
            useSessionStore.getState().setActiveSessionId(snap.conversation_id);
          }
        } catch {
          // ignore
        }
      }
      if (!currentActiveId || !isMounted) {
        console.info("[Header] sessions_changed: early return", { currentActiveId, isMounted });
        return;
      }

      // 60ms delay to ensure asynchronous SQLite worker commit finishes
      await new Promise((resolve) => setTimeout(resolve, 60));
      if (!isMounted) return;

      try {
        const [sessions, projects] = await Promise.all([
          getSessions(),
          getProjects().catch(() => []),
        ]);
        if (!isMounted) return;

        const found = sessions.find((s) => s.id === currentActiveId);
        if (!found) {
          console.info("[Header] sessions_changed: active session not in list", currentActiveId);
        }
        if (found && isMounted) {
          const resolvedTitle = resolveSessionTitle(found);
          const resolvedProjectName = found.project_id
            ? projects.find((p) => p.id === found.project_id)?.name ?? null
            : null;

          const currentLabel = useSessionStore.getState().activeSessionLabel;
          if (
            resolvedTitle !== currentLabel.sessionTitle ||
            resolvedProjectName !== currentLabel.projectName
          ) {
            console.info(
              "[Header] label update:",
              currentLabel,
              "->",
              { sessionTitle: resolvedTitle, projectName: resolvedProjectName }
            );
            setActiveSessionLabel({
              projectName: resolvedProjectName,
              sessionTitle: resolvedTitle,
            });
          } else {
            console.info("[Header] sessions_changed: no label change", JSON.stringify(resolvedTitle));
          }
        }
      } catch (e) {
        console.warn("[ActiveSessionHeader] Failed to refresh session label:", e);
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

