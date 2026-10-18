import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useSessionStore } from "@/store/sessionStore";
import {
  getSessions,
  sortSessionsNewestFirst,
  updateSession,
  type SessionRow,
} from "@/services/persistenceService";
import {
  getProjects,
  createProject,
  renameProject,
  deleteProject,
  type ProjectRow,
} from "@/services/projectsService";
import { onSessionsChanged } from "@/services/eventsService";
import { RENDER_TRACE_ENABLED } from "@/shared/hooks/useRenderTrace";
import { useVoiceSessionActions } from "@/shared/context/VoiceSessionContext";
import { SESSION_COPY } from "@/data/sessionCopy";

export interface ProjectGroup {
  project: ProjectRow;
  sessions: SessionRow[];
}

/**
 * `sessions_changed` fires often (once per conversation turn). A plain
 * `setSessions(fresh)` replaces every row object and array, which invalidates
 * the memoized session list and re-renders the whole panel subtree even when
 * nothing observable changed. Reuse the previous row objects whose fields are
 * identical, and return the previous array itself when nothing changed so React
 * bails out of the update entirely.
 */
function sameSession(a: SessionRow, b: SessionRow): boolean {
  return (
    a.project_id === b.project_id &&
    a.title === b.title &&
    a.is_pinned === b.is_pinned &&
    a.deleted_at === b.deleted_at &&
    a.created_at === b.created_at &&
    a.updated_at === b.updated_at &&
    a.turn_count === b.turn_count &&
    a.first_message === b.first_message &&
    a.uncompacted_turns === b.uncompacted_turns
  );
}

export function reconcileSessions(prev: SessionRow[], next: SessionRow[]): SessionRow[] {
  if (prev.length === 0) return next;
  const byId = new Map(prev.map((r) => [r.id, r]));
  let changed = prev.length !== next.length;
  const out = next.map((row, i) => {
    const old = byId.get(row.id);
    if (old && sameSession(old, row)) {
      if (prev[i] !== old) changed = true; // order changed
      return old;
    }
    changed = true;
    return row;
  });
  return changed ? out : prev;
}

export function reconcileProjects(prev: ProjectRow[], next: ProjectRow[]): ProjectRow[] {
  if (prev.length === 0) return next;
  const byId = new Map(prev.map((r) => [r.id, r]));
  let changed = prev.length !== next.length;
  const out = next.map((row, i) => {
    const old = byId.get(row.id);
    if (
      old &&
      old.name === row.name &&
      old.created_at === row.created_at &&
      old.updated_at === row.updated_at
    ) {
      if (prev[i] !== old) changed = true;
      return old;
    }
    changed = true;
    return row;
  });
  return changed ? out : prev;
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

/**
 * `active` gates all data work. Pass the panel's open state (see
 * `useEdgePanelActive`): while the panel is closed, this hook holds its last
 * data and stops listening for `sessions_changed`.
 */
export function useSessionPanel(active: boolean = true): UseSessionPanelReturn {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [projectOrder, setProjectOrder] = useState<string[]>(() => {
    try {
      const saved = localStorage.getItem("vox_projects_order");
      return saved ? (JSON.parse(saved) as string[]) : [];
    } catch {
      return [];
    }
  });

  const { selectSession, startNewConversation } = useVoiceSessionActions();

  // Latest values for callbacks that must stay referentially stable. Refresh
  // runs on every `sessions_changed`, so a callback closed over `sessions`
  // changes identity constantly and defeats `memo` on every session/project row.
  const sessionsRef = useRef<SessionRow[]>(sessions);
  const projectsRef = useRef<ProjectRow[]>(projects);
  const sortedProjectsRef = useRef<ProjectRow[]>([]);
  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);
  useEffect(() => {
    projectsRef.current = projects;
  }, [projects]);

  const refresh = useCallback(async () => {
    setError(null);
    const t0 = performance.now();
    performance.mark("session-panel-refresh-start");
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
      // Reconcile against the ref so we can report whether this refresh changed
      // anything. Functional `setState` alone cannot: its updater runs later.
      const prevSessions = sessionsRef.current;
      const prevProjects = projectsRef.current;
      const nextSessions = reconcileSessions(prevSessions, sortSessionsNewestFirst(sessionData ?? []));
      const nextProjects = reconcileProjects(prevProjects, Array.isArray(projectData) ? projectData : []);
      sessionsRef.current = nextSessions;
      projectsRef.current = nextProjects;
      setSessions(nextSessions);
      setProjects(nextProjects);
      if (RENDER_TRACE_ENABLED) {
        console.info(
          `[trace] refresh ${(performance.now() - t0).toFixed(1)}ms ` +
            `sessions=${nextSessions.length}(changed=${nextSessions !== prevSessions}) ` +
            `projects=${nextProjects.length}(changed=${nextProjects !== prevProjects})`
        );
      }
    } catch (e: unknown) {
      console.warn("[SessionPanel] refresh error:", e);
      sessionsRef.current = [];
      projectsRef.current = [];
      setSessions([]);
      setProjects([]);
      if (e instanceof Error) setError(e.message);
      else setError(SESSION_COPY.sessionsFailed);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // The panel stays mounted while closed (EdgePanel keeps `children` alive), so
    // an ungated subscription would fetch and re-render the whole hidden
    // subtree on every `sessions_changed`. Gate both on `active`, and refresh
    // once on activation so data created while closed still shows up.
    if (!active) return;
    let isMounted = true;
    // Defer the initial fetch out of the effect body: `refresh` closes over
    // setState, so calling it synchronously here reads as setState-in-effect.
    queueMicrotask(() => {
      if (!isMounted) return;
      void refresh();
    });

    const unlisten = onSessionsChanged(() => {
      if (!isMounted) return;
      void refresh();
    });

    return () => { isMounted = false; unlisten(); };
  }, [active, refresh]);

  // Deferred prefetch: while the panel is closed, fetch once after startup so
  // the first open renders cached data immediately. Live `sessions_changed`
  // updates stay gated on `active` to avoid hidden-subtree re-renders.
  const prefetchedRef = useRef(false);
  useEffect(() => {
    if (active || prefetchedRef.current) return;
    const timer = window.setTimeout(() => {
      if (prefetchedRef.current) return;
      prefetchedRef.current = true;
      void refresh();
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [active, refresh]);

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

  useEffect(() => {
    sortedProjectsRef.current = sortedProjects;
  }, [sortedProjects]);

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
        const activeSession = sessionsRef.current.find(
          (s) => s.id === useSessionStore.getState().activeSessionId
        );
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
    [refresh]
  );

  const deleteProjectHandle = useCallback(
    async (projectId: string) => {
      try {
        await deleteProject(projectId);
        await refresh();
        const activeLabel = useSessionStore.getState().activeSessionLabel;
        const activeSession = sessionsRef.current.find(
          (s) => s.id === useSessionStore.getState().activeSessionId
        );
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
    [refresh]
  );

  const togglePin = useCallback(async (sessionId: number) => {
    try {
      const session = sessionsRef.current.find((s) => s.id === sessionId);
      if (session) {
        await updateSession(sessionId, {
          isPinned: !session.is_pinned,
        });
      }
    } catch (e) {
      console.error("[SessionPanel] Failed to toggle pin:", e);
    }
  }, []);

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
          const project = projectId ? sortedProjectsRef.current.find((p) => p.id === projectId) : null;
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
    []
  );

  const deleteSessionHandle = useCallback(async (sessionId: number) => {
    try {
      const { deleteSession } = await import("@/services/persistenceService");
      await deleteSession(sessionId, false);
      const activeId = useSessionStore.getState().activeSessionId;
      if (activeId === sessionId) {
        useSessionStore.getState().setActiveSessionId(null);
        useSessionStore.getState().setActiveSessionLabel({ sessionTitle: null, projectName: null });
      }
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
