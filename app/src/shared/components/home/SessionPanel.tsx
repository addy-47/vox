import React, { memo, useCallback, useMemo, useState, useRef, useEffect } from "react";
import {
  Plus,
  Pin,
  Folder,
  FolderPlus,
  Loader2,
  AlertCircle,
  Check,
  X,
  MoreVertical,
  History,
  FolderGit2,
} from "lucide-react";
import { AnimatePresence, motion, Reorder, useDragControls } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { useSessionPanel, type ProjectGroup } from "@/shared/hooks/useSessionPanel";
import { useVoiceSession } from "@/shared/context/VoiceSessionContext";
import {
  resolveSessionTitle,
  sessionLastActivity,
  formatSessionRecency,
  type SessionRow,
} from "@/services/historyService";
import { type ProjectRow } from "@/services/projectService";
import { SESSION_COPY } from "@/data/sessionCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SessionContextMenu } from "@/shared/ui/SessionContextMenu";
import { useNotificationStore, selectUncompactedSessionIds } from "@/store/notificationStore";
import { metadataResolution } from "@/services/notificationService";
import { useSessionStore } from "@/store/sessionStore";

interface SessionPanelProps {
  onClose: () => void;
}

interface SessionRowItemProps {
  session: SessionRow;
  active: boolean;
  restoring: boolean;
  pinned: boolean;
  onSelect: (id: number) => void;
  onTogglePin: (id: number) => void;
  onRename: (id: number, newTitle: string) => void;
  onMoveToProject: (id: number, projectId: string | null) => void;
  onDelete: (id: number) => void;
  allProjects: ProjectRow[];
  nested?: boolean;
  projectTag?: string;
  showRecency?: boolean;
}

const SessionRowItem = memo(
  ({
    session,
    active,
    restoring,
    pinned,
    onSelect,
    onTogglePin,
    onRename,
    onMoveToProject,
    onDelete,
    allProjects,
    nested = false,
    projectTag,
    showRecency = true,
  }: SessionRowItemProps) => {
    const [isRenaming, setIsRenaming] = useState(false);
    const [renameTitle, setRenameTitle] = useState("");
    const [menuOpen, setMenuOpen] = useState(false);
    const [menuAnchorRect, setMenuAnchorRect] = useState<DOMRect | null>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);

    const title = resolveSessionTitle(session);
    const recency = useMemo(
      () => formatSessionRecency(sessionLastActivity(session)),
      [session]
    );

    const isUncompacted = useNotificationStore(
      useCallback(
        (s) => selectUncompactedSessionIds(s).has(session.id),
        [session.id]
      )
    );

    const isCompacting = useNotificationStore(
      useCallback(
        (s) => {
          if (s.activeActionIds.length === 0) return false;
          const notif = s.notifications.find(
            (n) =>
              n.category === "session_compaction" &&
              n.session_id === session.id &&
              n.status !== "dismissed" &&
              metadataResolution(n) !== "resolved"
          );
          return notif ? s.activeActionIds.includes(notif.id) : false;
        },
        [session.id]
      )
    );

    const executeCompaction = useNotificationStore(
      (s) => s.executeCompactionForSession
    );

    const handleSelect = useCallback(() => {
      if (isRenaming || menuOpen) return;
      onSelect(session.id);
    }, [onSelect, session.id, isRenaming, menuOpen]);

    const handlePin = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onTogglePin(session.id);
      },
      [onTogglePin, session.id]
    );

    const handleStartRename = useCallback(() => {
      setRenameTitle(title);
      setIsRenaming(true);
    }, [title]);

    const handleCommitRename = useCallback(() => {
      const trimmed = renameTitle.trim();
      if (trimmed && trimmed !== title) {
        onRename(session.id, trimmed);
      }
      setIsRenaming(false);
    }, [renameTitle, title, onRename, session.id]);

    const handleCancelRename = useCallback(() => {
      setIsRenaming(false);
    }, []);

    const handleMove = useCallback(
      (pId: string | null) => {
        onMoveToProject(session.id, pId);
      },
      [onMoveToProject, session.id]
    );

    const handleDelete = useCallback(() => {
      onDelete(session.id);
    }, [onDelete, session.id]);

    const handleCompact = useCallback(() => {
      executeCompaction(session.id);
    }, [executeCompaction, session.id]);

    // Drag start for dragging session into a project
    const handleDragStart = useCallback(
      (e: React.DragEvent) => {
        e.dataTransfer.setData("text/session-id", String(session.id));
        e.dataTransfer.effectAllowed = "move";
      },
      [session.id]
    );

    const handleMenuClick = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      if (triggerRef.current) {
        setMenuAnchorRect(triggerRef.current.getBoundingClientRect());
      }
      setMenuOpen((prev) => !prev);
    }, []);

    return (
      <div
        draggable={!isRenaming}
        onDragStart={handleDragStart}
        role="button"
        tabIndex={0}
        onClick={handleSelect}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") handleSelect();
        }}
        aria-current={active ? "true" : undefined}
        className={cn(
          "group relative flex items-center justify-between gap-1.5 py-2 rounded-lg text-left cursor-pointer transition-all select-none border border-transparent",
          active
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] font-medium shadow-sm"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]",
          restoring && "opacity-60 cursor-wait animate-pulse",
          nested ? "pl-7 pr-2" : "px-3"
        )}
      >
        <div className="flex items-center gap-2 min-w-0 flex-1">
          {restoring ? (
            <Loader2
              size={13}
              className="shrink-0 animate-spin text-[rgb(var(--accent))]"
              aria-label={SESSION_COPY.restoringAriaLabel}
            />
          ) : (
            <div
              className={cn(
                "w-1.5 h-1.5 rounded-full shrink-0 transition-colors",
                active
                  ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                  : "bg-[rgba(var(--foreground),0.2)] group-hover:bg-[rgba(var(--foreground),0.4)]"
              )}
            />
          )}

          {isRenaming ? (
            <div
              className="flex items-center gap-1 flex-1 min-w-0"
              onClick={(e) => e.stopPropagation()}
            >
              <input
                autoFocus
                type="text"
                value={renameTitle}
                onChange={(e) => setRenameTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleCommitRename();
                  if (e.key === "Escape") handleCancelRename();
                }}
                onBlur={handleCommitRename}
                className="w-full bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--accent),0.5)] rounded px-1.5 py-0.5 text-[12px] font-sans text-[rgb(var(--foreground))] focus:outline-none"
              />
              <button
                type="button"
                onClick={handleCommitRename}
                className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] cursor-pointer"
              >
                <Check size={12} />
              </button>
            </div>
          ) : (
            <div className="flex flex-col min-w-0 flex-1">
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="truncate text-[13.5px] tracking-normal leading-tight font-sans">
                  {title}
                </span>
                {isUncompacted && (
                  <span
                    className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.6)] shrink-0 animate-pulse"
                    title={SESSION_COPY.actions.uncompactedTurnsTooltip}
                  />
                )}
              </div>
              {projectTag && (
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 flex items-center gap-1 mt-0.5">
                  <Folder size={10} className="shrink-0" />
                  <span className="truncate">{projectTag}</span>
                </span>
              )}
            </div>
          )}
        </div>

        {/* Actions Area */}
        {!isRenaming && (
          <div className="flex items-center gap-0.5 shrink-0">
            {showRecency && (
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/40 opacity-0 group-hover:opacity-100 transition-opacity mr-1">
                {recency}
              </span>
            )}

            {/* Pin Toggle */}
            <button
              type="button"
              onClick={handlePin}
              aria-pressed={pinned}
              aria-label={pinned ? SESSION_COPY.unpinAriaLabel : SESSION_COPY.pinAriaLabel}
              className={cn(
                "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                pinned
                  ? "text-[rgb(var(--accent))] opacity-100"
                  : "opacity-0 group-hover:opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
              )}
            >
              <Pin size={12} fill={pinned ? "currentColor" : "none"} />
            </button>

            {/* Vertical Three Dots Context Menu Trigger */}
            <button
              ref={triggerRef}
              type="button"
              onClick={handleMenuClick}
              className={cn(
                "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                menuOpen
                  ? "opacity-100 text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.08)]"
                  : "opacity-0 group-hover:opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
              )}
              aria-label={SESSION_COPY.actions.moreOptions}
            >
              <MoreVertical size={13} />
            </button>
          </div>
        )}

        {/* Floating Context Menu Portal (Opens inward/rightwards) */}
        <SessionContextMenu
          open={menuOpen}
          onClose={() => setMenuOpen(false)}
          anchorRect={menuAnchorRect}
          sessionId={session.id}
          currentProjectId={session.project_id ?? null}
          allProjects={allProjects}
          isUncompacted={isUncompacted}
          isCompacting={isCompacting}
          onCompact={handleCompact}
          onStartRename={handleStartRename}
          onMoveToProject={handleMove}
          onDelete={handleDelete}
        />
      </div>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";

/**
 * Clean Project Item (No chevron accordion indicator, hover + to add conversation in project)
 */
interface ProjectRowItemProps {
  group: ProjectGroup;
  allProjects: ProjectRow[];
  activeSessionId: number | null;
  isRestoring: boolean;
  restoringSessionId: number | null;
  isDragTarget: boolean;
  onSelectSession: (id: number) => void;
  onTogglePinSession: (id: number) => void;
  onRenameSession: (id: number, newTitle: string) => void;
  onMoveSessionToProject: (id: number, projectId: string | null) => void;
  onDeleteSession: (id: number) => void;
  onCreateSessionInProject: (projectId: string) => void;
  onDragOver: (e: React.DragEvent, projectId: string) => void;
  onDragLeave: (projectId: string) => void;
  onDrop: (e: React.DragEvent, projectId: string) => void;
  onDragEndCommit?: () => void;
}

const ProjectRowItem = memo(
  ({
    group,
    allProjects,
    activeSessionId,
    isRestoring,
    restoringSessionId,
    isDragTarget,
    onSelectSession,
    onTogglePinSession,
    onRenameSession,
    onMoveSessionToProject,
    onDeleteSession,
    onCreateSessionInProject,
    onDragOver,
    onDragLeave,
    onDrop,
    onDragEndCommit,
  }: ProjectRowItemProps) => {
    const [expanded, setExpanded] = useState(false);
    const [isDragging, setIsDragging] = useState(false);
    const dragControls = useDragControls();
    const isDraggingRef = useRef(false);

    const handleToggleExpand = useCallback(() => {
      if (isDraggingRef.current) return;
      setExpanded((prev) => !prev);
    }, []);

    const handleCreateInProject = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onCreateSessionInProject(group.project.id);
      },
      [group.project.id, onCreateSessionInProject]
    );

    return (
      <Reorder.Item
        as="div"
        value={group.project.id}
        id={group.project.id}
        dragListener={false}
        dragControls={dragControls}
        onDragStart={() => {
          isDraggingRef.current = true;
          setIsDragging(true);
        }}
        onDragEnd={() => {
          setIsDragging(false);
          setTimeout(() => {
            isDraggingRef.current = false;
          }, 80);
          onDragEndCommit?.();
        }}
        onDragOver={(e) => onDragOver(e, group.project.id)}
        onDragLeave={() => onDragLeave(group.project.id)}
        onDrop={(e) => onDrop(e, group.project.id)}
        className={cn(
          "flex flex-col rounded-lg transition-colors border border-transparent select-none relative",
          isDragging &&
            "z-50 bg-[rgb(var(--card))] shadow-[0_16px_36px_-4px_rgba(0,0,0,0.45),0_0_0_1px_rgba(var(--accent),0.4),0_0_24px_-2px_rgba(var(--accent),0.2)]",
          isDragTarget &&
            "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.12)] shadow-md"
        )}
        whileDrag={{
          scale: 1.025,
          cursor: "grabbing",
        }}
        transition={{ type: "spring", stiffness: 450, damping: 32 }}
      >
        {/* Project Header Row: Flat folder, title, hover + to add session */}
        <div
          role="button"
          tabIndex={0}
          onPointerDown={(e) => {
            if (e.button === 0 && !(e.target as HTMLElement).closest("button")) {
              dragControls.start(e);
            }
          }}
          onClick={handleToggleExpand}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") handleToggleExpand();
          }}
          className="group flex items-center justify-between gap-1.5 px-3 py-2 rounded-lg text-left cursor-grab active:cursor-grabbing hover:bg-[rgba(var(--foreground),0.04)] transition-colors select-none touch-none"
        >
          <div className="flex items-center gap-2 min-w-0 flex-1">
            <Folder
              size={13}
              className={cn(
                "shrink-0 transition-colors",
                expanded
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground-muted))]"
              )}
            />
            <span className="truncate text-[13.5px] font-medium text-[rgb(var(--foreground))]">
              {group.project.name}
            </span>
          </div>

          <div className="flex items-center gap-1 shrink-0">
            {/* Hover + button to start conversation in this project */}
            <Tooltip label={SESSION_COPY.newInProjectAriaLabel} side="left">
              <button
                type="button"
                onClick={handleCreateInProject}
                className="opacity-0 group-hover:opacity-100 flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--accent),0.1)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all cursor-pointer"
                aria-label={SESSION_COPY.newInProjectAriaLabel}
              >
                <Plus size={12} />
              </button>
            </Tooltip>

            {group.sessions.length > 0 && (
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/50 px-1 py-0.2 rounded bg-[rgba(var(--foreground),0.04)] shrink-0">
                {group.sessions.length}
              </span>
            )}
          </div>
        </div>

        {/* Clean Nested Sessions (Indented under project) */}
        <AnimatePresence initial={false}>
          {expanded && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.16, ease: "easeInOut" }}
              className="flex flex-col overflow-hidden gap-0.5"
            >
              {group.sessions.length > 0 ? (
                group.sessions.map((session) => (
                  <SessionRowItem
                    key={session.id}
                    session={session}
                    active={session.id === activeSessionId}
                    restoring={isRestoring && restoringSessionId === session.id}
                    pinned={false}
                    nested={true}
                    onSelect={onSelectSession}
                    onTogglePin={onTogglePinSession}
                    onRename={onRenameSession}
                    onMoveToProject={onMoveSessionToProject}
                    onDelete={onDeleteSession}
                    allProjects={allProjects}
                  />
                ))
              ) : (
                <span className="pl-6 py-1.5 text-[11px] text-[rgb(var(--foreground-muted))]/40 italic">
                  {SESSION_COPY.noProjectSessions}
                </span>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </Reorder.Item>
    );
  }
);
ProjectRowItem.displayName = "ProjectRowItem";

/**
 * Conversation list rendered inside EdgePanel (side="left").
 * Antigravity IDE-style sidebar with + New conversation, History toggle, Pinned, and Projects.
 */
export const SessionPanel = memo(({ onClose }: SessionPanelProps) => {
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
    togglePin,
    selectSession,
    renameSession,
    moveSessionToProject,
    deleteSession,
    reorderProjects,
  } = useSessionPanel();
  const { activeSessionId, isRestoring, restoringSessionId } = useVoiceSession();
  const setActiveSessionLabel = useSessionStore((s) => s.setActiveSessionLabel);

  const [viewMode, setViewMode] = useState<"projects" | "history">("projects");
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [dragOverProjectId, setDragOverProjectId] = useState<string | null>(null);

  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of allProjects) {
      map.set(p.id, p.name);
    }
    return map;
  }, [allProjects]);

  // Combine all sessions for full reverse-chronological history view
  const allChronologicalSessions = useMemo(() => {
    const list: SessionRow[] = [
      ...pinnedSessions,
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
  }, [pinnedSessions, uncategorizedSessions, projects]);

  const handleSelect = useCallback(
    (id: number) => {
      // Find session across all lists to resolve its title and project name for the breadcrumb
      const allSessions = [
        ...pinnedSessions,
        ...uncategorizedSessions,
        ...projects.flatMap((p) => p.sessions),
      ];
      const session = allSessions.find((s) => s.id === id);
      if (session) {
        setActiveSessionLabel({
          sessionTitle: resolveSessionTitle(session),
          projectName: session.project_id ? projectNameMap.get(session.project_id) ?? null : null,
        });
      }
      selectSession(id)
        .then(() => onClose())
        .catch(() => {});
    },
    [selectSession, onClose, pinnedSessions, uncategorizedSessions, projects, projectNameMap, setActiveSessionLabel]
  );

  const handleNew = useCallback(() => {
    setActiveSessionLabel({ sessionTitle: null, projectName: null });
    createNewSession()
      .then(() => onClose())
      .catch(() => {});
  }, [createNewSession, onClose, setActiveSessionLabel]);

  const handleCreateInProject = useCallback(
    (projectId: string) => {
      const project = allProjects.find((p) => p.id === projectId);
      setActiveSessionLabel({ sessionTitle: null, projectName: project?.name ?? null });
      createNewSession(projectId)
        .then(() => onClose())
        .catch(() => {});
    },
    [createNewSession, onClose, allProjects, setActiveSessionLabel]
  );

  const handleRetry = useCallback(() => {
    refresh().catch(() => {});
  }, [refresh]);

  const handleCreateProject = useCallback(() => {
    const name = projectName.trim();
    if (!name) return;
    createNewProject(name)
      .then(() => {
        setProjectName("");
        setIsCreatingProject(false);
        return refresh();
      })
      .catch(() => {});
  }, [projectName, createNewProject, refresh]);

  // Real-time project reorder with local optimistic IDs
  const [projectIds, setProjectIds] = useState<string[]>(() =>
    projects.map((g) => g.project.id)
  );
  const isDraggingProjectRef = useRef(false);

  // Sync with incoming projects prop ONLY if not actively dragging
  useEffect(() => {
    if (!isDraggingProjectRef.current) {
      setProjectIds(projects.map((g) => g.project.id));
    }
  }, [projects]);

  const handleReorderProjectIds = useCallback((newIds: string[]) => {
    isDraggingProjectRef.current = true;
    setProjectIds(newIds);
  }, []);

  const handleDragEndCommit = useCallback(() => {
    isDraggingProjectRef.current = false;
    reorderProjects(projectIds);
  }, [projectIds, reorderProjects]);

  // Fast O(1) project group lookup by ID
  const projectGroupMap = useMemo(() => {
    const map = new Map<string, ProjectGroup>();
    for (const g of projects) {
      map.set(g.project.id, g);
    }
    return map;
  }, [projects]);

  // Session drop into project handlers
  const handleSessionDragOver = useCallback((e: React.DragEvent, targetProjectId: string) => {
    if (e.dataTransfer.types.includes("text/session-id")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setDragOverProjectId(targetProjectId);
    }
  }, []);

  const handleSessionDragLeave = useCallback(
    (targetProjectId: string) => {
      if (dragOverProjectId === targetProjectId) {
        setDragOverProjectId(null);
      }
    },
    [dragOverProjectId]
  );

  const handleSessionDrop = useCallback(
    (e: React.DragEvent, targetProjectId: string) => {
      e.preventDefault();
      setDragOverProjectId(null);
      const sessionIdStr = e.dataTransfer.getData("text/session-id");
      if (sessionIdStr) {
        const sId = Number(sessionIdStr);
        if (sId) {
          moveSessionToProject(sId, targetProjectId);
        }
      }
    },
    [moveSessionToProject]
  );

  return (
    <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-3 px-3 pt-1 pb-16 select-none font-sans">
      {/* ── Top Actions: + New Conversation & Conversation History ── */}
      <div className="flex flex-col gap-0.5 shrink-0">
        <button
          type="button"
          onClick={handleNew}
          aria-label={SESSION_COPY.newConversationAriaLabel}
          className="w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg text-left cursor-pointer transition-colors text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] bg-[rgba(var(--foreground),0.03)]"
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
          onClick={() => setViewMode((m) => (m === "history" ? "projects" : "history"))}
          aria-pressed={viewMode === "history"}
          className={cn(
            "w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg text-left cursor-pointer transition-colors text-[13.5px] font-medium tracking-normal",
            viewMode === "history"
              ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)]"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
          )}
        >
          {viewMode === "history" ? (
            <FolderGit2 size={14} className="shrink-0 text-[rgb(var(--accent))]" />
          ) : (
            <History size={14} className="shrink-0 text-[rgb(var(--foreground-muted))]" />
          )}
          <span>
            {viewMode === "history" ? SESSION_COPY.projectsSection : SESSION_COPY.conversationHistory}
          </span>
        </button>
      </div>

      {/* ── Transient Error Banner ── */}
      {error && (
        <div className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-lg bg-red-500/10 border border-red-500/20 text-[11px] text-red-400 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <AlertCircle size={13} className="shrink-0 text-red-400" />
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

      {/* ── Pinned Section — always visible in both modes ── */}
      <section className="flex flex-col gap-1 shrink-0">
        <div className="flex items-center gap-1.5 px-2 py-0.5">
          <span className="text-[11.5px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
            {SESSION_COPY.pinnedSection}
          </span>
          <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/50 ml-auto">
            {pinnedSessions.length}
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

      {/* ── Main Content Area ── */}
      {viewMode === "history" ? (
        /* Full Flat Reverse-Chronological Session History */
        <div className="flex flex-col gap-1 flex-1 min-h-0">
          <div className="px-3 py-1 text-[11.5px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
            {SESSION_COPY.conversationHistory}
          </div>
          <div className="flex flex-col gap-0.5">
            {allChronologicalSessions.length > 0 ? (
              allChronologicalSessions.map((session) => (
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
              <span className="px-2 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noSessionsTitle}
              </span>
            )}
          </div>
        </div>
      ) : (
        /* Default Mode: Projects only (pinned is above, always visible) */
        <div className="flex flex-col gap-3.5 flex-1 min-h-0">

          {/* ── Projects Section ── */}

          <section className="flex flex-col gap-1">
            <div className="flex items-center justify-between px-2 py-0.5">
              <div className="flex items-center gap-1.5">
                <span className="text-[11.5px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
                  {SESSION_COPY.projectsSection}
                </span>
                {projects.length > 0 && (
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50">
                    {projects.length}
                  </span>
                )}
              </div>
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

            {/* Inline Project Creation — underline style, no pill */}
            <AnimatePresence>
              {isCreatingProject && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  className="overflow-hidden px-2 pb-1"
                >
                  <div className="flex items-center gap-1.5 border-b border-[rgba(var(--accent),0.5)] focus-within:border-[rgba(var(--accent),0.9)] transition-colors py-1">
                    <input
                      autoFocus
                      value={projectName}
                      onChange={(e) => setProjectName(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleCreateProject();
                        if (e.key === "Escape") {
                          setIsCreatingProject(false);
                          setProjectName("");
                        }
                      }}
                      placeholder={SESSION_COPY.newProjectPlaceholder}
                      aria-label={SESSION_COPY.createProjectAriaLabel}
                      className="flex-1 min-w-0 bg-transparent text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none"
                    />
                    <button
                      type="button"
                      onClick={handleCreateProject}
                      disabled={!projectName.trim()}
                      className="flex items-center justify-center w-6 h-6 rounded text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] disabled:opacity-30 cursor-pointer transition-colors"
                    >
                      <Check size={13} />
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setIsCreatingProject(false);
                        setProjectName("");
                      }}
                      className="flex items-center justify-center w-6 h-6 rounded text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer transition-colors"
                    >
                      <X size={13} />
                    </button>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Projects List with Framer Motion Reorder.Group */}
            <div className="flex flex-col gap-0.5">
              {projectIds.length > 0 ? (
                <Reorder.Group
                  as="div"
                  axis="y"
                  values={projectIds}
                  onReorder={handleReorderProjectIds}
                  className="flex flex-col gap-0.5"
                >
                  {projectIds.map((id) => {
                    const group = projectGroupMap.get(id);
                    if (!group) return null;
                    return (
                      <ProjectRowItem
                        key={id}
                        group={group}
                        allProjects={allProjects}
                        activeSessionId={activeSessionId}
                        isRestoring={isRestoring}
                        restoringSessionId={restoringSessionId}
                        isDragTarget={dragOverProjectId === id}
                        onSelectSession={handleSelect}
                        onTogglePinSession={togglePin}
                        onRenameSession={renameSession}
                        onMoveSessionToProject={moveSessionToProject}
                        onDeleteSession={deleteSession}
                        onCreateSessionInProject={handleCreateInProject}
                        onDragOver={handleSessionDragOver}
                        onDragLeave={handleSessionDragLeave}
                        onDrop={handleSessionDrop}
                        onDragEndCommit={handleDragEndCommit}
                      />
                    );
                  })}
                </Reorder.Group>
              ) : (
                <span className="px-2 py-1 text-[11px] text-[rgb(var(--foreground-muted))]/35 italic">
                  {SESSION_COPY.noProjectsYet}
                </span>
              )}
            </div>
          </section>

          {/* Bottom spacing cushion */}
          <div className="h-6 shrink-0" aria-hidden="true" />
        </div>
      )}
    </div>
  );
});
SessionPanel.displayName = "SessionPanel";
