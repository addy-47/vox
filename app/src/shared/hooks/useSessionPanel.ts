import { useState, useEffect, useCallback, useMemo } from "react";
import {
  getSessions,
  sortSessionsNewestFirst,
  updateSession,
  type SessionRow,
} from "@/services/historyService";
import { getProjects, createProject, type ProjectRow } from "@/services/projectService";
import { onSessionsChanged } from "@/services/eventsService";
import { useVoiceSession } from "@/shared/context/VoiceSessionContext";
import { SESSION_COPY } from "@/data/sessionCopy";

interface ProjectGroup {
  project: ProjectRow;
  sessions: SessionRow[];
}

interface UseSessionPanelReturn {
  pinnedSessions: SessionRow[];
  projects: ProjectGroup[];
  uncategorizedSessions: SessionRow[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  createNewSession: () => Promise<void>;
  createNewProject: (name: string) => Promise<void>;
  togglePin: (sessionId: number) => Promise<void>;
  selectSession: (sessionId: number) => Promise<void>;
}

export function useSessionPanel(): UseSessionPanelReturn {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const { selectSession, startNewConversation } = useVoiceSession();

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [sessionData, projectData] = await Promise.all([
        getSessions().catch((e) => {
          console.warn("[SessionPanel] getSessions error:", e);
          return [] as SessionRow[];
        }),
        getProjects().catch((e) => {
          console.warn("[SessionPanel] getProjects error:", e);
          return [] as ProjectRow[];
        }),
      ]);
      setSessions(sortSessionsNewestFirst(sessionData ?? []));
      setProjects(Array.isArray(projectData) ? projectData : []);
    } catch (e: unknown) {
      console.warn("[SessionPanel] refresh error:", e);
      setSessions([]);
      setProjects([]);
      if (e instanceof Error) setError(e.message);
      else setError(SESSION_COPY.sessionsFailed);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let isMounted = true;
    refresh().catch(() => { if (isMounted) setLoading(false); });

    const unlisten = onSessionsChanged(() => {
      if (!isMounted) return;
      refresh().catch(() => {});
    });

    return () => { isMounted = false; unlisten(); };
  }, [refresh]);

  const pinnedSessions = useMemo(
    () => (Array.isArray(sessions) ? sessions.filter((s) => Boolean(s?.is_pinned)) : []),
    [sessions]
  );

  const nonPinnedSessions = useMemo(
    () => (Array.isArray(sessions) ? sessions.filter((s) => !s?.is_pinned) : []),
    [sessions]
  );

  const projectsWithSessions = useMemo((): ProjectGroup[] => {
    if (!Array.isArray(projects)) return [];
    return projects.map((project) => ({
      project,
      sessions: nonPinnedSessions.filter((s) => s?.project_id === project?.id),
    }));
  }, [projects, nonPinnedSessions]);

  const uncategorizedSessions = useMemo(
    () =>
      nonPinnedSessions.filter((s) => {
        if (!Array.isArray(projects) || projects.length === 0) return true;
        const hasProject = projects.some((p) => p?.id === s?.project_id);
        return !hasProject || !s?.project_id;
      }),
    [nonPinnedSessions, projects]
  );

  const createNewSession = useCallback(async () => {
    try {
      await startNewConversation();
    } catch (e) {
      console.error("[SessionPanel] Failed to create session:", e);
    }
  }, [startNewConversation]);

  const createNewProject = useCallback(async (name: string) => {
    try {
      await createProject(name);
    } catch (e) {
      console.error("[SessionPanel] Failed to create project:", e);
    }
  }, []);

  const togglePin = useCallback(async (sessionId: number) => {
    try {
      const session = sessions.find((s) => s.id === sessionId);
      if (session) {
        await updateSession(sessionId, {
          isPinned: !session.is_pinned,
        });
      }
    } catch (e) {
      console.error("[SessionPanel] Failed to toggle pin:", e);
    }
  }, [sessions]);

  const selectSessionHandle = useCallback(async (sessionId: number) => {
    try {
      await selectSession(sessionId);
    } catch (e) {
      console.error("[SessionPanel] Failed to select session:", e);
    }
  }, [selectSession]);

  return {
    pinnedSessions,
    projects: projectsWithSessions,
    uncategorizedSessions,
    loading,
    error,
    refresh,
    createNewSession,
    createNewProject,
    togglePin,
    selectSession: selectSessionHandle,
  };
}
