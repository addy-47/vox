import React, { memo, useCallback, useMemo, useRef, useEffect } from "react";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { useRenderTrace, RENDER_TRACE_ENABLED } from "@/shared/hooks/useRenderTrace";
import {
  Plus,
  Loader2,
  AlertCircle,
  MessagesSquare,
  FolderGit2,
} from "lucide-react";
import { useSessionPanel } from "@/shared/hooks/useSessionPanel";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  resolveSessionTitle,
  sessionLastActivity,
  type SessionRow,
} from "@/services/persistenceService";
import { SESSION_COPY } from "@/data/sessionCopy";
import { useSessionStore } from "@/store/sessionStore";
import {
  useSessionPanelUiStore,
  sessionPanelScrollTop,
} from "@/store/sessionPanelUiStore";
import { SessionRowItem } from "./session/SessionRowItem";
import { SessionProjectsSection } from "./session/SessionProjectsSection";
import { useEdgePanelActive } from "@/shared/ui/edgePanelActive";

interface SessionPanelProps {
  onClose: () => void;
}

/**
 * Fixed height for one row in the virtualized history list. Rows are spaced by
 * this constant, so no per-row measurement is needed.
 */
const SESSION_ROW_HEIGHT = 52;

/**
 * Conversation list rendered inside EdgePanel (side="left").
 * Antigravity IDE-style sidebar with + New conversation, History toggle, Pinned, and Projects.
 */
export const SessionPanel = memo(({ onClose }: SessionPanelProps) => {
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

  const viewMode = useSessionPanelUiStore((s) => s.viewMode);
  const setViewMode = useSessionPanelUiStore((s) => s.setViewMode);

  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of allProjects) {
      map.set(p.id, p.name);
    }
    return map;
  }, [allProjects]);

  // Combine all non-pinned sessions for full reverse-chronological history view (pinned shown separately above, never duplicated)
  const allChronologicalSessions = useMemo(() => {
    const list: SessionRow[] = [
      ...uncategorizedSessions,
      ...projects.flatMap((p) => p.sessions),
    ];
    // Deduplicate by id and sort descending by updated_at / last activity
    const seen = new Set<number>();
    const deduped: SessionRow[] = [];
    for (const s of list) {
      if (!seen.has(s.id)) {
        seen.add(s.id);
        deduped.push(s);
      }
    }
    return deduped.sort(
      (a, b) => sessionLastActivity(b) - sessionLastActivity(a)
    );
  }, [uncategorizedSessions, projects]);

  // [trace] RCA: `watch` holds the identities React itself compares. Every key
  // in the logged `changed=[...]` is a cause of this component's re-render.
  useRenderTrace(
    "SessionPanel",
    () => ({
      viewMode,
      pinned: pinnedSessions.length,
      projects: projects.length,
      uncategorized: uncategorizedSessions.length,
      chronological: allChronologicalSessions.length,
    }),
    {
      watch: {
        onClose,
        viewMode,
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

  // [trace] Open latency: EdgePanel marks `edge-panel-open` on every open. The
  // mark is cleared here, otherwise a later open measures against a stale mark
  // and reports seconds that never elapsed.
  useEffect(() => {
    if (!RENDER_TRACE_ENABLED) return;
    try {
      performance.measure("session-panel-mount", "edge-panel-open");
      const entry = performance.getEntriesByName("session-panel-mount").pop();
      if (entry) {
        console.info(
          `[trace] SessionPanel mounted ${entry.duration.toFixed(1)}ms after open`
        );
      }
    } catch {
      /* no open mark present */
    }
    performance.clearMarks("edge-panel-open");
  }, []);

  // [trace] Data -> paint cost and DOM weight. This is the number the lag is
  // made of: how long from the fetch that populated the panel to the frame that
  // shows it, and how many elements that frame had to build.
  const rootRef = useRef<HTMLDivElement>(null);
  const firstDataPaintRef = useRef(false);
  useEffect(() => {
    if (loading || firstDataPaintRef.current) return;
    firstDataPaintRef.current = true;
    if (!RENDER_TRACE_ENABLED) return;
    const nodes = rootRef.current?.querySelectorAll("*").length ?? 0;
    requestAnimationFrame(() => {
      let ms = Number.NaN;
      try {
        performance.measure(
          "session-panel-first-paint",
          "session-panel-refresh-start"
        );
        ms = (
          performance.getEntriesByName("session-panel-first-paint").pop()
            ?.duration ?? Number.NaN
        );
      } catch {
        /* no refresh mark present */
      }
      performance.clearMarks("session-panel-refresh-start");
      console.info(
        `[trace] SessionPanel data->paint ${ms.toFixed(1)}ms domNodes=${nodes} ` +
          `projects=${projects.length} rows=${
            pinnedSessions.length +
            uncategorizedSessions.length +
            projects.reduce((n, p) => n + p.sessions.length, 0)
          }`
      );
    });
  }, [loading, pinnedSessions, uncategorizedSessions, projects]);

  // `handleSelect` is passed to every SessionRowItem. If it closed over the
  // session lists it would get a new identity on every data change, defeating
  // `memo` on all ~40 rows. Read the data from a ref instead so the callback is
  // stable.
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
      // Resolve title and project name for the breadcrumb from the latest data.
      const { sessions, projectNames } = handleSelectDataRef.current;
      const session = sessions.get(id);
      if (session) {
        setActiveSessionLabel({
          sessionTitle: resolveSessionTitle(session),
          projectName: session.project_id ? projectNames.get(session.project_id) ?? null : null,
        });
      }
      onClose();
      selectSession(id).catch(() => {});
    },
    [selectSession, onClose, setActiveSessionLabel]
  );

  const handleNew = useCallback(() => {
    setActiveSessionLabel({ sessionTitle: null, projectName: null });
    onClose();
    createNewSession().catch(() => {});
  }, [createNewSession, onClose, setActiveSessionLabel]);

  const handleCreateInProject = useCallback(
    (projectId: string) => {
      // Read through the ref so this stays stable: it reaches every project
      // row, and it showed up in the trace as changing on every data load.
      const project = handleSelectDataRef.current.projectNames.get(projectId);
      setActiveSessionLabel({ sessionTitle: null, projectName: project ?? null });
      onClose();
      createNewSession(projectId).catch(() => {});
    },
    [createNewSession, onClose, setActiveSessionLabel]
  );

  const handleRetry = useCallback(() => {
    refresh().catch(() => {});
  }, [refresh]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: allChronologicalSessions.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => SESSION_ROW_HEIGHT,
    // Constant row height on purpose. With dynamic measurement, every row that
    // mounts while scrolling re-reports its height, so `getTotalSize()` changes
    // on each pass and the native scrollbar thumb visibly resizes as you scroll.
    // A fixed height makes the total stable for the whole session.
    measureElement: () => SESSION_ROW_HEIGHT,
    overscan: 6,
  });

  // Persist scroll offset across panel close/reopen (panel stays mounted).
  const scrollTraceAtRef = useRef(0);
  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      if (loading) return;
      const el = e.currentTarget;
      sessionPanelScrollTop.value = el.scrollTop;
      // [trace] RCA: a native scrollbar thumb is proportional to
      // scrollHeight/clientHeight, so a thumb that resizes while scrolling means
      // one of those moved. Throttled to one line per 500ms.
      if (!RENDER_TRACE_ENABLED) return;
      const now = performance.now();
      if (now - scrollTraceAtRef.current < 500) return;
      scrollTraceAtRef.current = now;
      console.info(
        `[trace] history-scroll clientH=${el.clientHeight} scrollH=${el.scrollHeight} ` +
          `virtualTotal=${rowVirtualizer.getTotalSize()} count=${rowVirtualizer.getVirtualItems().length} ` +
          `scrollTop=${el.scrollTop.toFixed(0)}`
      );
    },
    [loading, rowVirtualizer]
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
      {/* ── Fixed Header: Top Actions + Error Banner + Pinned Section ── */}
      <div className="shrink-0 flex flex-col gap-5 px-1.5 pt-3 pb-2">
        {/* Top Actions: + New Conversation & view mode toggle */}
        <div className="flex flex-col gap-1">
          <button
            type="button"
            onClick={handleNew}
            aria-label={SESSION_COPY.newConversationAriaLabel}
            className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-sm text-left cursor-pointer transition-colors text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] bg-transparent"
          >
            <Plus size={14} className="shrink-0 text-[rgb(var(--accent))]" strokeWidth={2} />
            <span className="text-[13.5px] font-medium tracking-normal">
              {SESSION_COPY.newConversation}
            </span>
            {loading && (
              <Loader2 size={12} className="animate-spin text-[rgb(var(--foreground-muted))]/60 ml-auto" />
            )}
          </button>

          <button
            type="button"
            onClick={() => setViewMode(viewMode === "history" ? "projects" : "history")}
            aria-pressed={viewMode === "history"}
            className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-sm text-left cursor-pointer transition-colors text-[13.5px] font-medium tracking-normal text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] bg-transparent"
          >
            {viewMode === "history" ? (
              <FolderGit2 size={14} className="shrink-0 text-[rgb(var(--accent))]" />
            ) : (
              <MessagesSquare size={14} className="shrink-0 text-[rgb(var(--accent))]" />
            )}
            <span>
              {viewMode === "history" ? SESSION_COPY.projectsSection : SESSION_COPY.conversationHistory}
            </span>
          </button>
        </div>

        {/* Transient Error Banner */}
        {error && (
          <div className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-sm bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[11px] text-[rgb(var(--error))]">
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

        {/* Pinned Section — always visible in both modes */}
        <section className="flex flex-col gap-1">
          <div className="flex items-center gap-1.5 px-2 py-0.5">
            <span className="text-[11.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/60">
              {SESSION_COPY.pinnedSection}
            </span>
          </div>
          <div className="flex flex-col gap-0.5">
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
              <span className="px-2 py-1.5 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noPinnedSessions}
              </span>
            )}
          </div>
        </section>
      </div>

      {/* ── Main Content Area: Fixed Header + Scrollable Content per mode ── */}
      <div className="flex-1 min-h-0 flex flex-col px-1.5 pt-2">
        {viewMode === "history" ? (
          /* Full Flat Reverse-Chronological Session History */
          <div className="flex flex-col flex-1 min-h-0">
            {/* Fixed Header */}
            <div className="shrink-0 px-2 py-1 text-[11.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/60">
              {SESSION_COPY.conversationHistory}
            </div>

            {/* Scrollable Virtualized History List */}
            <div
              ref={scrollRef}
              onScroll={handleScroll}
              data-session-scroll
              className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-0.5 pt-1 pb-16"
            >
              {allChronologicalSessions.length > 0 ? (
                <div
                  style={{
                    height: `${rowVirtualizer.getTotalSize()}px`,
                    width: "100%",
                    position: "relative",
                  }}
                >
                  {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                    const session = allChronologicalSessions[virtualRow.index];
                    return (
                      <div
                        key={session.id}
                        data-index={virtualRow.index}
                        style={{
                          position: "absolute",
                          top: 0,
                          left: 0,
                          width: "100%",
                          transform: `translateY(${virtualRow.start}px)`,
                        }}
                      >
                        <SessionRowItem
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
                      </div>
                    );
                  })}
                </div>
              ) : (
                <span className="px-2 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                  {SESSION_COPY.noSessionsTitle}
                </span>
              )}
            </div>
          </div>
        ) : (
          /* Default Mode: Projects only */
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
          />
        )}
      </div>
    </div>
  );
});
SessionPanel.displayName = "SessionPanel";
