import { useState, useEffect, useCallback, useMemo } from "react";
import { useSessionStore } from "@/store/sessionStore";
import {
  getSessions,
  sortSessionsNewestFirst,
  updateSession,
  type SessionRow,
} from "@/services/historyService";
import {
  getProjects,
  createProject,
  renameProject,
  deleteProject,
  type ProjectRow,
} from "@/services/projectService";
import { onSessionsChanged } from "@/services/eventsService";
import { useVoiceSession } from "@/shared/context/VoiceSessionContext";
import { SESSION_COPY } from "@/data/sessionCopy";

export interface ProjectGroup {
  project: ProjectRow;
  sessions: SessionRow[];
}

interface UseSessionPanelReturn {
  pinnedSessions: SessionRow[];
  projects: ProjectGroup[];
  allProjects: ProjectRow[];
  uncategorizedSessions: SessionRow[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  createNewSession: (projectId?: string) => Promise<void>;
  createNewProject: (name: string) => Promise<void>;
  renameProject: (projectId: string, newName: string) => Promise<void>;
  deleteProject: (projectId: string) => Promise<void>;
  togglePin: (sessionId: number) => Promise<void>;
  selectSession: (sessionId: number) => Promise<void>;
  renameSession: (sessionId: number, newTitle: string) => Promise<void>;
  moveSessionToProject: (sessionId: number, projectId: string | null) => Promise<void>;
  deleteSession: (sessionId: number) => Promise<void>;
  reorderProjects: (orderedIds: string[]) => void;
}

export function useSessionPanel(): UseSessionPanelReturn {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [projectOrder, setProjectOrder] = useState<string[]>(() => {
    try {
      const saved = localStorage.getItem("vox_projects_order");
      return saved ? JSON.parse(saved) : [];
    } catch {
      return [];
    }
  });

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

  // Sort projects according to projectOrder
  const sortedProjects = useMemo(() => {
    if (!Array.isArray(projects)) return [];
    if (projectOrder.length === 0) return projects;
    return [...projects].sort((a, b) => {
      const idxA = projectOrder.indexOf(a.id);
      const idxB = projectOrder.indexOf(b.id);
      if (idxA !== -1 && idxB !== -1) return idxA - idxB;
      if (idxA !== -1) return -1;
      if (idxB !== -1) return 1;
      return b.updated_at - a.updated_at;
    });
  }, [projects, projectOrder]);

  const projectsWithSessions = useMemo((): ProjectGroup[] => {
    return sortedProjects.map((project) => ({
      project,
      sessions: nonPinnedSessions.filter((s) => s?.project_id === project?.id),
    }));
  }, [sortedProjects, nonPinnedSessions]);

  const uncategorizedSessions = useMemo(
    () =>
      nonPinnedSessions.filter((s) => {
        if (!Array.isArray(projects) || projects.length === 0) return true;
        const hasProject = projects.some((p) => p?.id === s?.project_id);
        return !hasProject || !s?.project_id;
      }),
    [nonPinnedSessions, projects]
  );

  const createNewSession = useCallback(async (projectId?: string) => {
    try {
      await startNewConversation(projectId);
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

  const renameProjectHandle = useCallback(
    async (projectId: string, newName: string) => {
      try {
        await renameProject(projectId, newName);
        await refresh();
        const activeLabel = useSessionStore.getState().activeSessionLabel;
        const activeSession = sessions.find((s) => s.id === useSessionStore.getState().activeSessionId);
        if (activeSession?.project_id === projectId) {
          useSessionStore.getState().setActiveSessionLabel({
            sessionTitle: activeLabel.sessionTitle,
            projectName: newName,
          });
        }
      } catch (e) {
        console.error("[SessionPanel] Failed to rename project:", e);
      }
    },
    [refresh, sessions]
  );

  const deleteProjectHandle = useCallback(
    async (projectId: string) => {
      try {
        await deleteProject(projectId);
        await refresh();
        const activeLabel = useSessionStore.getState().activeSessionLabel;
        const activeSession = sessions.find((s) => s.id === useSessionStore.getState().activeSessionId);
        if (activeSession?.project_id === projectId) {
          useSessionStore.getState().setActiveSessionLabel({
            sessionTitle: activeLabel.sessionTitle,
            projectName: null,
          });
        }
      } catch (e) {
        console.error("[SessionPanel] Failed to delete project:", e);
        throw e;
      }
    },
    [sessions, refresh]
  );

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

  const renameSessionHandle = useCallback(async (sessionId: number, newTitle: string) => {
    try {
      await updateSession(sessionId, { title: newTitle });
      const activeId = useSessionStore.getState().activeSessionId;
      if (activeId === sessionId) {
        const currentLabel = useSessionStore.getState().activeSessionLabel;
        useSessionStore.getState().setActiveSessionLabel({
          sessionTitle: newTitle,
          projectName: currentLabel.projectName,
        });
      }
    } catch (e) {
      console.error("[SessionPanel] Failed to rename session:", e);
    }
  }, []);

  const moveSessionToProjectHandle = useCallback(
    async (sessionId: number, projectId: string | null) => {
      try {
        await updateSession(sessionId, { projectId });
        const activeId = useSessionStore.getState().activeSessionId;
        if (activeId === sessionId) {
          const project = projectId ? sortedProjects.find((p) => p.id === projectId) : null;
          const currentLabel = useSessionStore.getState().activeSessionLabel;
          useSessionStore.getState().setActiveSessionLabel({
            sessionTitle: currentLabel.sessionTitle,
            projectName: project?.name ?? null,
          });
        }
      } catch (e) {
        console.error("[SessionPanel] Failed to move session:", e);
      }
    },
    [sortedProjects]
  );

  const deleteSessionHandle = useCallback(async (sessionId: number) => {
    try {
      const { deleteSession } = await import("@/services/historyService");
      await deleteSession(sessionId, false);
    } catch (e) {
      console.error("[SessionPanel] Failed to delete session:", e);
    }
  }, []);

  const reorderProjectsHandle = useCallback((orderedIds: string[]) => {
    setProjectOrder(orderedIds);
    try {
      localStorage.setItem("vox_projects_order", JSON.stringify(orderedIds));
    } catch (e) {
      console.warn("[SessionPanel] Failed to save project order:", e);
    }
  }, []);

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
    allProjects: sortedProjects,
    uncategorizedSessions,
    loading,
    error,
    refresh,
    createNewSession,
    createNewProject,
    renameProject: renameProjectHandle,
    deleteProject: deleteProjectHandle,
    togglePin,
    selectSession: selectSessionHandle,
    renameSession: renameSessionHandle,
    moveSessionToProject: moveSessionToProjectHandle,
    deleteSession: deleteSessionHandle,
    reorderProjects: reorderProjectsHandle,
  };
}
