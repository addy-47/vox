import React, { useEffect } from "react";
import { motion } from "framer-motion";
import { useSessionStore } from "@/store/sessionStore";
import { onSessionsChanged } from "@/services/eventsService";
import { getSessions, resolveSessionTitle } from "@/services/historyService";
import { getProjects } from "@/services/projectService";
import { getRuntimeSnapshot } from "@/services/monitoringService";

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
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const activeSessionLabel = useSessionStore((s) => s.activeSessionLabel);
  const setActiveSessionLabel = useSessionStore((s) => s.setActiveSessionLabel);

  const { projectName, sessionTitle } = activeSessionLabel;



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
      if (!currentActiveId) {
        setActiveSessionLabel({ projectName: null, sessionTitle: null });
        return;
      }
      if (!isMounted) return;

      // NOTE: no artificial delay here. sessions_changed is coalesced with a
      // 250ms trailing edge in eventsService, which already covers the SQLite
      // worker commit lag the old 60ms sleep was waiting out.
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

  // Only hide when not on Home, or no active session, or no label at all.
  if (!isHome || !activeSessionId || (!projectName && !sessionTitle)) {
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
          <span className="truncate">{sessionTitle}</span>
        </span>
      )}
    </motion.div>
  );
};

