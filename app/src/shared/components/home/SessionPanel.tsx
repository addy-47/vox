import React, { memo, useCallback, useMemo, useRef, useEffect, useState } from "react";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { useRenderTrace } from "@/shared/hooks/useRenderTrace";
import {
  Plus,
  Search,
  Bot,
  SlidersHorizontal,
  Loader2,
  AlertCircle,
  FolderPlus,
} from "lucide-react";
import { useSessionPanel } from "@/shared/hooks/useSessionPanel";
import {
  resolveSessionTitle,
  sessionLastActivity,
  type SessionRow,
} from "@/services/persistenceService";
import { SESSION_COPY } from "@/data/sessionCopy";
import { useSessionStore } from "@/store/sessionStore";
import {
  sessionPanelScrollTop,
} from "@/store/sessionPanelUiStore";
import { SessionRowItem } from "./session/SessionRowItem";
import { SessionProjectsSection } from "./session/SessionProjectsSection";
import { useEdgePanelActive } from "@/shared/ui/edgePanelActive";
import { Tooltip } from "@/shared/ui/Tooltip";

interface SessionPanelProps {
  onClose: () => void;
  onSearch: () => void;
}

/**
 * Conversation list rendered inside EdgePanel (side="left").
 * Reorganized structure with top action buttons (+ New, Search, Automations, Customizations)
 * and a single scrollable area with sticky non-uppercase headers (Pinned, Projects, Conversations).
 */
export const SessionPanel = memo(({ onClose, onSearch }: SessionPanelProps) => {
  useMemoryTrace("SessionPanel");
  // The panel stays mounted while closed, so its data layer must idle then.
  const panelActive = useEdgePanelActive();
  const {
    pinnedSessions,
    projects,
    allProjects,
    uncategorizedSessions,
    loading,
    error,
    refresh,
    createNewSession,
    createNewProject,
    renameProject,
    deleteProject,
    togglePin,
    selectSession,
    renameSession,
    moveSessionToProject,
    deleteSession,
    reorderProjects,
  } = useSessionPanel(panelActive);

  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const isRestoring = useSessionStore((s) => s.isRestoring);
  const restoringSessionId = useSessionStore((s) => s.restoringSessionId);
  const setActiveSessionLabel = useSessionStore((s) => s.setActiveSessionLabel);
  const [isCreatingProject, setIsCreatingProject] = useState(false);

  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of allProjects) {
      map.set(p.id, p.name);
    }
    return map;
  }, [allProjects]);

  // Combine all non-pinned sessions for the reverse-chronological Conversations section
  const allChronologicalSessions = useMemo(() => {
    const list: SessionRow[] = [
      ...uncategorizedSessions,
      ...projects.flatMap((p) => p.sessions),
    ];
    // Deduplicate by id and sort descending by updated_at / last activity
    const seen = new Set<number>();
    const deduped: SessionRow[] = [];
    for (const s of list) {
      if (!seen.has(s.id) && !s.is_pinned) {
        seen.add(s.id);
        deduped.push(s);
      }
    }
    return deduped.sort(
      (a, b) => sessionLastActivity(b) - sessionLastActivity(a)
    );
  }, [uncategorizedSessions, projects]);

  // Progressive rendering for reverse-chronological sessions to keep drawer opening buttery-smooth at 60fps
  const [chronologicalRenderLimit, setChronologicalRenderLimit] = useState(25);

  useEffect(() => {
    if (!panelActive) {
      setChronologicalRenderLimit(25);
      return;
    }
    if (typeof window === "undefined") return;
    if ("requestIdleCallback" in window) {
      const handle = window.requestIdleCallback(() => {
        setChronologicalRenderLimit((prev) => {
          if (prev < allChronologicalSessions.length) {
            return allChronologicalSessions.length;
          }
          return prev;
        });
      }, { timeout: 300 });
      return () => window.cancelIdleCallback(handle);
    } else {
      const timer = setTimeout(() => {
        setChronologicalRenderLimit((prev) => {
          if (prev < allChronologicalSessions.length) {
            return allChronologicalSessions.length;
          }
          return prev;
        });
      }, 100);
      return () => clearTimeout(timer);
    }
  }, [panelActive, allChronologicalSessions.length]);

  const displayedChronologicalSessions = useMemo(() => {
    return allChronologicalSessions.slice(0, chronologicalRenderLimit);
  }, [allChronologicalSessions, chronologicalRenderLimit]);

  useRenderTrace(
    "SessionPanel",
    () => ({
      pinned: pinnedSessions.length,
      projects: projects.length,
      uncategorized: uncategorizedSessions.length,
      chronological: allChronologicalSessions.length,
    }),
    {
      watch: {
        onClose,
        onSearch,
        activeSessionId,
        isRestoring,
        restoringSessionId,
        loading,
        error,
        pinnedSessions,
        projects,
        allProjects,
        uncategorizedSessions,
        allChronologicalSessions,
        projectNameMap,
      },
    }
  );

  const rootRef = useRef<HTMLDivElement>(null);
  const handleSelectDataRef = useRef<{
    sessions: Map<number, SessionRow>;
    projectNames: Map<string, string>;
  }>({ sessions: new Map(), projectNames: new Map() });

  useEffect(() => {
    const sessions = new Map<number, SessionRow>();
    for (const s of pinnedSessions) sessions.set(s.id, s);
    for (const s of uncategorizedSessions) sessions.set(s.id, s);
    for (const p of projects) {
      for (const s of p.sessions) sessions.set(s.id, s);
    }
    handleSelectDataRef.current = { sessions, projectNames: projectNameMap };
  }, [pinnedSessions, uncategorizedSessions, projects, projectNameMap]);

  const handleSelect = useCallback(
    (id: number) => {
      const { sessions, projectNames } = handleSelectDataRef.current;
      const session = sessions.get(id);
      if (session) {
        setActiveSessionLabel({
          sessionTitle: resolveSessionTitle(session),
          projectName: session.project_id ? projectNames.get(session.project_id) ?? null : null,
        });
      }
      onClose();
      selectSession(id).catch((e: unknown) => {
        console.debug("[SessionPanel] selectSession failed:", e);
      });
    },
    [selectSession, onClose, setActiveSessionLabel]
  );

  const handleNew = useCallback(() => {
    setActiveSessionLabel({ sessionTitle: null, projectName: null });
    onClose();
    createNewSession().catch((e: unknown) => {
      console.debug("[SessionPanel] createNewSession failed:", e);
    });
  }, [createNewSession, onClose, setActiveSessionLabel]);

  const handleSearchConversations = useCallback(() => {
    onClose();
    onSearch();
  }, [onClose, onSearch]);

  const handleCreateInProject = useCallback(
    (projectId: string) => {
      const project = handleSelectDataRef.current.projectNames.get(projectId);
      setActiveSessionLabel({ sessionTitle: null, projectName: project ?? null });
      onClose();
      createNewSession(projectId).catch((e: unknown) => {
        console.debug("[SessionPanel] createNewSession in project failed:", e);
      });
    },
    [createNewSession, onClose, setActiveSessionLabel]
  );

  const handleRetry = useCallback(() => {
    refresh().catch((e: unknown) => {
      console.debug("[SessionPanel] refresh failed:", e);
    });
  }, [refresh]);

  const scrollRef = useRef<HTMLDivElement>(null);

  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      if (loading) return;
      sessionPanelScrollTop.value = e.currentTarget.scrollTop;
    },
    [loading]
  );

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const raf = requestAnimationFrame(() => {
      el.scrollTop = sessionPanelScrollTop.value;
    });
    return () => cancelAnimationFrame(raf);
  }, [loading]);

  return (
    <div ref={rootRef} className="flex-1 min-h-0 flex flex-col overflow-hidden select-none font-sans">
      {/* ── Fixed Top Action Rows ── */}
      <div className="shrink-0 flex flex-col gap-1 px-2 pt-3 pb-2 border-b border-[rgba(var(--border),0.08)]">
        {/* + New Conversation */}
        <button
          type="button"
          onClick={handleNew}
          aria-label={SESSION_COPY.newConversationAriaLabel}
          className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left cursor-pointer transition-colors text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] bg-transparent"
        >
          <Plus size={15} className="shrink-0 text-[rgb(var(--accent))]" strokeWidth={2} />
          <span className="text-[13px] font-medium tracking-normal">
            {SESSION_COPY.newConversation}
          </span>
          {loading && (
            <Loader2 size={12} className="animate-spin text-[rgb(var(--foreground-muted))]/60 ml-auto" />
          )}
        </button>

        {/* Search Conversations -> triggers Home Search mode */}
        <button
          type="button"
          onClick={handleSearchConversations}
          aria-label={SESSION_COPY.searchConversationsAriaLabel}
          className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left cursor-pointer transition-colors text-[13px] font-medium tracking-normal text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] bg-transparent"
        >
          <Search size={15} className="shrink-0 text-[rgb(var(--accent))]" />
          <span>{SESSION_COPY.searchConversations}</span>
        </button>

        {/* Automations (Disabled with Coming Soon tooltip) */}
        <Tooltip label={SESSION_COPY.comingSoon} side="right">
          <button
            type="button"
            disabled
            className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left text-[13px] font-medium tracking-normal text-[rgb(var(--foreground-muted))]/40 opacity-60 cursor-not-allowed bg-transparent"
          >
            <Bot size={15} className="shrink-0 text-[rgb(var(--foreground-muted))]/50" />
            <span>{SESSION_COPY.automations}</span>
          </button>
        </Tooltip>

        {/* Customizations (Disabled with Coming Soon tooltip) */}
        <Tooltip label={SESSION_COPY.comingSoon} side="right">
          <button
            type="button"
            disabled
            className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left text-[13px] font-medium tracking-normal text-[rgb(var(--foreground-muted))]/40 opacity-60 cursor-not-allowed bg-transparent"
          >
            <SlidersHorizontal size={15} className="shrink-0 text-[rgb(var(--foreground-muted))]/50" />
            <span>{SESSION_COPY.customizations}</span>
          </button>
        </Tooltip>
      </div>

      {/* Transient Error Banner */}
      {error && (
        <div className="shrink-0 mx-2 mt-2 flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[11px] text-[rgb(var(--error))]">
          <div className="flex items-center gap-1.5 min-w-0">
            <AlertCircle size={13} className="shrink-0 text-[rgb(var(--error))]" />
            <span className="truncate">{error}</span>
          </div>
          <button
            type="button"
            onClick={handleRetry}
            className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:underline shrink-0 cursor-pointer"
          >
            {SESSION_COPY.retry}
          </button>
        </div>
      )}

      {/* ── Single Scrollable Container with Sticky Headers (No Uppercase, Zero-Slit Occlusion) ── */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        data-session-scroll
        className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col -mt-px"
      >
        {/* ── Section 1: Pinned Conversations ── */}
        <section className="flex flex-col pb-1">
          <div className="sticky top-0 z-30 px-3 py-1.5 text-[11.5px] font-mono font-medium tracking-normal text-[rgb(var(--foreground-muted))] bg-[rgb(var(--card))] shadow-xs">
            {SESSION_COPY.pinnedConversations}
          </div>
          <div className="flex flex-col gap-0.5 px-1.5 pt-1">
            {pinnedSessions.length > 0 ? (
              pinnedSessions.map((session) => (
                <SessionRowItem
                  key={session.id}
                  session={session}
                  active={session.id === activeSessionId}
                  restoring={isRestoring && restoringSessionId === session.id}
                  pinned={true}
                  onSelect={handleSelect}
                  onTogglePin={togglePin}
                  onRename={renameSession}
                  onMoveToProject={moveSessionToProject}
                  onDelete={deleteSession}
                  allProjects={allProjects}
                  projectTag={session.project_id ? projectNameMap.get(session.project_id) : undefined}
                />
              ))
            ) : (
              <span className="px-3 py-1.5 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noPinnedSessions}
              </span>
            )}
          </div>
        </section>

        {/* ── Section 2: Projects ── */}
        <section className="flex flex-col pb-1">
          <div className="sticky top-0 z-30 px-3 py-1.5 flex items-center justify-between text-[11.5px] font-mono font-medium tracking-normal text-[rgb(var(--foreground-muted))] bg-[rgb(var(--card))] shadow-xs">
            <span>{SESSION_COPY.projectsSection}</span>
            <Tooltip label={SESSION_COPY.createProjectAriaLabel} side="left">
              <button
                type="button"
                onClick={() => setIsCreatingProject((v) => !v)}
                className="flex items-center justify-center w-5 h-5 rounded text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer"
                aria-label={SESSION_COPY.createProjectAriaLabel}
              >
                <FolderPlus size={13} />
              </button>
            </Tooltip>
          </div>
          <div className="px-1.5 pt-1">
            <SessionProjectsSection
              projects={projects}
              allProjects={allProjects}
              activeSessionId={activeSessionId}
              isRestoring={isRestoring}
              restoringSessionId={restoringSessionId}
              createNewProject={createNewProject}
              reorderProjects={reorderProjects}
              renameProject={renameProject}
              deleteProject={deleteProject}
              refresh={refresh}
              onSelectSession={handleSelect}
              onTogglePinSession={togglePin}
              onRenameSession={renameSession}
              onMoveSessionToProject={moveSessionToProject}
              onDeleteSession={deleteSession}
              onCreateSessionInProject={handleCreateInProject}
              isCreatingProject={isCreatingProject}
              setIsCreatingProject={setIsCreatingProject}
            />
          </div>
        </section>

        {/* ── Section 3: Conversations (Reverse-Chronological History) ── */}
        <section className="flex flex-col pb-16">
          <div className="sticky top-0 z-30 px-3 py-1.5 text-[11.5px] font-mono font-medium tracking-normal text-[rgb(var(--foreground-muted))] bg-[rgb(var(--card))] shadow-xs">
            {SESSION_COPY.conversations}
          </div>
          <div className="flex flex-col gap-0.5 px-1.5">
            {displayedChronologicalSessions.length > 0 ? (
              displayedChronologicalSessions.map((session) => (
                <SessionRowItem
                  key={session.id}
                  session={session}
                  active={session.id === activeSessionId}
                  restoring={isRestoring && restoringSessionId === session.id}
                  pinned={Boolean(session.is_pinned)}
                  onSelect={handleSelect}
                  onTogglePin={togglePin}
                  onRename={renameSession}
                  onMoveToProject={moveSessionToProject}
                  onDelete={deleteSession}
                  allProjects={allProjects}
                  projectTag={session.project_id ? projectNameMap.get(session.project_id) : undefined}
                />
              ))
            ) : (
              <span className="px-3 py-1.5 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noSessionsTitle}
              </span>
            )}
          </div>
        </section>
      </div>
    </div>
  );
});

SessionPanel.displayName = "SessionPanel";
