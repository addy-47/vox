import React, { memo, useCallback, useMemo, useState, useRef } from "react";
import {
  Plus,
  Pin,
  MessageSquare,
  Folder,
  FolderOpen,
  FolderPlus,
  ChevronRight,
  Loader2,
  AlertCircle,
  Check,
  X,
  MoreHorizontal,
  Pencil,
  FolderInput,
  Trash2,
  GripVertical,
  Sparkles,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { useSessionPanel } from "@/shared/hooks/useSessionPanel";
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
import { useNotificationStore } from "@/store/notificationStore";
import { metadataResolution } from "@/services/notificationService";

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
  }: SessionRowItemProps) => {
    const [isRenaming, setIsRenaming] = useState(false);
    const [renameTitle, setRenameTitle] = useState("");
    const [menuOpen, setMenuOpen] = useState(false);
    const [moveSubmenuOpen, setMoveSubmenuOpen] = useState(false);
    const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null);

    const title = resolveSessionTitle(session);
    const recency = useMemo(
      () => formatSessionRecency(sessionLastActivity(session)),
      [session]
    );

    const isUncompacted = useNotificationStore(
      useCallback(
        (s) =>
          s.notifications.some(
            (n) =>
              n.category === "session_compaction" &&
              n.session_id === session.id &&
              n.status !== "dismissed" &&
              metadataResolution(n) !== "resolved"
          ),
        [session.id]
      )
    );

    const isCompacting = useNotificationStore(
      useCallback(
        (s) => {
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

    const handleStartRename = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        setRenameTitle(title);
        setIsRenaming(true);
        setMenuOpen(false);
      },
      [title]
    );

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
        setMoveSubmenuOpen(false);
        setMenuOpen(false);
      },
      [onMoveToProject, session.id]
    );

    const handleDelete = useCallback(() => {
      onDelete(session.id);
      setIsConfirmingDelete(false);
      setMenuOpen(false);
    }, [onDelete, session.id]);

    // Drag start for dragging session into a project
    const handleDragStart = useCallback(
      (e: React.DragEvent) => {
        e.dataTransfer.setData("text/session-id", String(session.id));
        e.dataTransfer.effectAllowed = "move";
      },
      [session.id]
    );

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
          "group relative flex items-center justify-between gap-1.5 py-1.5 rounded-lg text-left cursor-pointer transition-all select-none border border-transparent",
          active
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] font-medium shadow-sm"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]",
          restoring && "opacity-60 cursor-wait animate-pulse",
          nested ? "pl-7 pr-2" : "px-2.5"
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
            <MessageSquare
              size={13}
              className={cn(
                "shrink-0 transition-colors",
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground-muted))]"
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
                <span className="truncate text-[12.5px] tracking-wide">
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
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 flex items-center gap-1">
                  <Folder size={10} />
                  <span className="truncate">{projectTag}</span>
                </span>
              )}
            </div>
          )}
        </div>

        {/* Actions Area */}
        {!isRenaming && (
          <div className="flex items-center gap-0.5 shrink-0">
            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40 opacity-0 group-hover:opacity-100 transition-opacity mr-1">
              {recency}
            </span>

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

            {/* Three Dots Context Menu Button */}
            <div className="relative" ref={menuRef}>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setMenuOpen((prev) => !prev);
                  setMoveSubmenuOpen(false);
                  setIsConfirmingDelete(false);
                }}
                className={cn(
                  "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                  menuOpen
                    ? "opacity-100 text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.08)]"
                    : "opacity-0 group-hover:opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
                )}
                aria-label={SESSION_COPY.actions.moreOptions}
              >
                <MoreHorizontal size={13} />
              </button>

              {/* Context Dropdown Menu */}
              <AnimatePresence>
                {menuOpen && (
                  <motion.div
                    initial={{ opacity: 0, scale: 0.95, y: 4 }}
                    animate={{ opacity: 1, scale: 1, y: 0 }}
                    exit={{ opacity: 0, scale: 0.95, y: 4 }}
                    transition={{ duration: 0.12 }}
                    onClick={(e) => e.stopPropagation()}
                    className="absolute right-0 top-6 w-44 rounded-xl glass-card border border-[rgba(var(--border),0.16)] bg-[rgba(var(--card),0.96)] backdrop-blur-2xl shadow-2xl p-1 z-50 flex flex-col gap-0.5 text-[11.5px] font-sans select-none"
                  >
                    {isConfirmingDelete ? (
                      <div className="p-2 flex flex-col gap-2">
                        <span className="text-[11px] font-semibold text-red-400 leading-tight">
                          {SESSION_COPY.actions.deleteConfirm}
                        </span>
                        <div className="flex items-center gap-1.5 justify-end">
                          <button
                            type="button"
                            onClick={() => setIsConfirmingDelete(false)}
                            className="px-2 py-1 rounded text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
                          >
                            {SESSION_COPY.actions.cancel}
                          </button>
                          <button
                            type="button"
                            onClick={handleDelete}
                            className="px-2 py-1 rounded text-[10.5px] font-mono font-bold bg-red-500/20 text-red-400 hover:bg-red-500/30 cursor-pointer"
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    ) : moveSubmenuOpen ? (
                      <div className="flex flex-col gap-0.5">
                        <div className="px-2 py-1 text-[10px] font-mono uppercase text-[rgb(var(--foreground-muted))]/70 border-b border-[rgba(var(--border),0.08)] flex items-center justify-between">
                          <span>{SESSION_COPY.actions.moveToProject}</span>
                          <button
                            type="button"
                            onClick={() => setMoveSubmenuOpen(false)}
                            className="text-[9px] hover:text-[rgb(var(--foreground))] cursor-pointer"
                          >
                            Back
                          </button>
                        </div>
                        <button
                          type="button"
                          onClick={() => handleMove(null)}
                          className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.3)]" />
                          <span className="truncate">{SESSION_COPY.actions.removeFromProject}</span>
                        </button>
                        {allProjects.map((p) => (
                          <button
                            key={p.id}
                            type="button"
                            onClick={() => handleMove(p.id)}
                            className={cn(
                              "flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer",
                              session.project_id === p.id && "text-[rgb(var(--accent))] font-semibold"
                            )}
                          >
                            <Folder size={12} className="shrink-0" />
                            <span className="truncate">{p.name}</span>
                          </button>
                        ))}
                      </div>
                    ) : (
                      <>
                        {isUncompacted && (
                          <>
                            <button
                              type="button"
                              disabled={isCompacting}
                              onClick={(e) => {
                                e.stopPropagation();
                                setMenuOpen(false);
                                executeCompaction(session.id);
                              }}
                              className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] font-medium transition-colors cursor-pointer disabled:opacity-50"
                            >
                              {isCompacting ? (
                                <Loader2 size={12} className="animate-spin text-[rgb(var(--accent))]" />
                              ) : (
                                <Sparkles size={12} className="text-[rgb(var(--accent))]" />
                              )}
                              <span>
                                {isCompacting
                                  ? SESSION_COPY.actions.compactingSession
                                  : SESSION_COPY.actions.compactSession}
                              </span>
                            </button>
                            <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5" />
                          </>
                        )}

                        <button
                          type="button"
                          onClick={handleStartRename}
                          className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <Pencil size={12} className="text-[rgb(var(--foreground-muted))]" />
                          <span>{SESSION_COPY.actions.rename}</span>
                        </button>

                        <button
                          type="button"
                          onClick={() => setMoveSubmenuOpen(true)}
                          className="flex items-center justify-between px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <div className="flex items-center gap-2">
                            <FolderInput size={12} className="text-[rgb(var(--foreground-muted))]" />
                            <span>{SESSION_COPY.actions.moveToProject}</span>
                          </div>
                          <ChevronRight size={12} className="text-[rgb(var(--foreground-muted))]/60" />
                        </button>

                        <button
                          type="button"
                          onClick={handlePin}
                          className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <Pin size={12} className="text-[rgb(var(--foreground-muted))]" />
                          <span>{pinned ? SESSION_COPY.actions.unpin : SESSION_COPY.actions.pin}</span>
                        </button>

                        <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5" />

                        <button
                          type="button"
                          onClick={() => setIsConfirmingDelete(true)}
                          className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-red-500/10 text-red-400 transition-colors cursor-pointer"
                        >
                          <Trash2 size={12} />
                          <span>{SESSION_COPY.actions.delete}</span>
                        </button>
                      </>
                    )}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        )}
      </div>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";

/**
 * Conversation list rendered inside EdgePanel (side="left").
 * Antigravity IDE-grade sidebar: + New conversation, Pinned list, Projects with drag-and-drop & reorder.
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

  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [dragOverProjectId, setDragOverProjectId] = useState<string | null>(null);
  const [newSessionPulse, setNewSessionPulse] = useState(false);

  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of allProjects) {
      map.set(p.id, p.name);
    }
    return map;
  }, [allProjects]);

  const isProjectExpanded = useCallback(
    (id: string) => expandedProjects[id] !== false,
    [expandedProjects]
  );

  const toggleProject = useCallback((id: string) => {
    setExpandedProjects((prev) => ({
      ...prev,
      [id]: prev[id] === false,
    }));
  }, []);

  const handleSelect = useCallback(
    (id: number) => {
      selectSession(id)
        .then(() => onClose())
        .catch(() => {});
    },
    [selectSession, onClose]
  );

  const handleNew = useCallback(() => {
    setNewSessionPulse(true);
    setTimeout(() => setNewSessionPulse(false), 400);
    createNewSession()
      .then(() => onClose())
      .catch(() => {});
  }, [createNewSession, onClose]);

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

  // Project reorder drag handlers
  const handleProjectDragStart = useCallback((e: React.DragEvent, projectId: string) => {
    e.dataTransfer.setData("text/project-reorder-id", projectId);
    e.dataTransfer.effectAllowed = "move";
  }, []);

  const handleProjectDragOver = useCallback((e: React.DragEvent, targetProjectId: string) => {
    e.preventDefault();
    if (e.dataTransfer.types.includes("text/project-reorder-id")) {
      e.dataTransfer.dropEffect = "move";
    } else if (e.dataTransfer.types.includes("text/session-id")) {
      e.dataTransfer.dropEffect = "move";
      setDragOverProjectId(targetProjectId);
    }
  }, []);

  const handleProjectDragLeave = useCallback((targetProjectId: string) => {
    if (dragOverProjectId === targetProjectId) {
      setDragOverProjectId(null);
    }
  }, [dragOverProjectId]);

  const handleProjectDrop = useCallback(
    (e: React.DragEvent, targetProjectId: string) => {
      e.preventDefault();
      setDragOverProjectId(null);

      // Check if this was a session being dropped into a project
      const sessionIdStr = e.dataTransfer.getData("text/session-id");
      if (sessionIdStr) {
        const sId = Number(sessionIdStr);
        if (sId) {
          moveSessionToProject(sId, targetProjectId);
        }
        return;
      }

      // Check if this was a project being reordered
      const draggedProjectId = e.dataTransfer.getData("text/project-reorder-id");
      if (draggedProjectId && draggedProjectId !== targetProjectId) {
        const currentIds = projects.map((p) => p.project.id);
        const fromIdx = currentIds.indexOf(draggedProjectId);
        const toIdx = currentIds.indexOf(targetProjectId);
        if (fromIdx !== -1 && toIdx !== -1) {
          const newOrder = [...currentIds];
          newOrder.splice(fromIdx, 1);
          newOrder.splice(toIdx, 0, draggedProjectId);
          reorderProjects(newOrder);
        }
      }
    },
    [projects, moveSessionToProject, reorderProjects]
  );

  return (
    <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-3 p-3 pb-16 select-none font-sans">
      {/* ── Top Action: + New Conversation (Antigravity Style) ── */}
      <button
        type="button"
        onClick={handleNew}
        aria-label={SESSION_COPY.newConversationAriaLabel}
        className={cn(
          "w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl border transition-all cursor-pointer shrink-0 shadow-sm",
          newSessionPulse
            ? "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] scale-[0.98]"
            : "border-[rgba(var(--border),0.14)] bg-[rgba(var(--card),0.65)] hover:border-[rgba(var(--accent),0.4)] hover:bg-[rgba(var(--accent),0.08)] text-[rgb(var(--foreground))]"
        )}
      >
        <div className="flex items-center gap-2.5">
          <div className="p-1 rounded-lg bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))]">
            <Plus size={14} strokeWidth={2.2} />
          </div>
          <span className="text-[13px] font-semibold tracking-wide">
            {SESSION_COPY.newConversation}
          </span>
        </div>
        {loading ? (
          <Loader2 size={13} className="animate-spin text-[rgb(var(--foreground-muted))]/60" />
        ) : (
          <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40">
            Ctrl+N
          </span>
        )}
      </button>

      {/* ── Inline Transient Error Banner (if any) ── */}
      {error && (
        <div className="flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg bg-red-500/10 border border-red-500/20 text-[11px] text-red-400 shrink-0">
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

      {/* ── Panel Content Sections ── */}
      <div className="flex flex-col gap-4 flex-1 min-h-0">
        {/* ── Pinned Conversations ── */}
        <section className="flex flex-col gap-1">
          <div className="flex items-center gap-1.5 px-2 py-0.5">
            <Pin size={11} className="text-[rgb(var(--accent))] shrink-0" />
            <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {SESSION_COPY.pinnedSection}
            </span>
            {pinnedSessions.length > 0 && (
              <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 ml-auto">
                {pinnedSessions.length}
              </span>
            )}
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
              <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noPinnedSessions}
              </span>
            )}
          </div>
        </section>

        {/* ── Projects with FolderPlus & Reorderable Accordions ── */}
        <section className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between px-2 py-0.5">
            <div className="flex items-center gap-1.5">
              <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
                {SESSION_COPY.projectsSection}
              </span>
              <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/60">
                {projects.length}
              </span>
            </div>
            <Tooltip label={SESSION_COPY.createProjectAriaLabel} side="left">
              <button
                type="button"
                onClick={() => setIsCreatingProject((v) => !v)}
                className="flex items-center justify-center w-6 h-6 rounded-lg text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer"
                aria-label={SESSION_COPY.createProjectAriaLabel}
              >
                <FolderPlus size={14} />
              </button>
            </Tooltip>
          </div>

          {/* Inline Project Creation Input */}
          <AnimatePresence>
            {isCreatingProject && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                className="overflow-hidden px-2 pb-1"
              >
                <div className="flex items-center gap-1.5 bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] rounded-xl px-2.5 py-1.5 focus-within:border-[rgba(var(--accent),0.4)]">
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
                    className="flex-1 min-w-0 bg-transparent text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={handleCreateProject}
                    disabled={!projectName.trim()}
                    className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] disabled:opacity-30 cursor-pointer"
                  >
                    <Check size={12} />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setIsCreatingProject(false);
                      setProjectName("");
                    }}
                    className="p-1 rounded text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.1)] cursor-pointer"
                  >
                    <X size={12} />
                  </button>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Projects List Accordions */}
          {projects.length > 0 ? (
            projects.map((group) => {
              const isExpanded = isProjectExpanded(group.project.id);
              const isTarget = dragOverProjectId === group.project.id;

              return (
                <div
                  key={group.project.id}
                  onDragOver={(e) => handleProjectDragOver(e, group.project.id)}
                  onDragLeave={() => handleProjectDragLeave(group.project.id)}
                  onDrop={(e) => handleProjectDrop(e, group.project.id)}
                  className={cn(
                    "flex flex-col rounded-xl transition-all duration-150 border",
                    isTarget
                      ? "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.12)] shadow-md ring-1 ring-[rgba(var(--accent),0.4)]"
                      : "border-transparent"
                  )}
                >
                  {/* Project Header Row */}
                  <div
                    draggable
                    onDragStart={(e) => handleProjectDragStart(e, group.project.id)}
                    role="button"
                    tabIndex={0}
                    onClick={() => toggleProject(group.project.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") toggleProject(group.project.id);
                    }}
                    className="group flex items-center justify-between gap-1.5 px-2 py-1.5 rounded-lg text-left cursor-pointer hover:bg-[rgba(var(--foreground),0.04)] transition-colors select-none"
                  >
                    <div className="flex items-center gap-1.5 min-w-0 flex-1">
                      <GripVertical
                        size={12}
                        className="opacity-0 group-hover:opacity-40 hover:!opacity-80 cursor-grab text-[rgb(var(--foreground-muted))] shrink-0"
                      />
                      <ChevronRight
                        size={13}
                        className={cn(
                          "shrink-0 text-[rgb(var(--foreground-muted))]/60 transition-transform duration-200",
                          isExpanded && "rotate-90 text-[rgb(var(--foreground))]"
                        )}
                      />
                      {isExpanded ? (
                        <FolderOpen size={14} className="shrink-0 text-[rgb(var(--accent))]" />
                      ) : (
                        <Folder size={14} className="shrink-0 text-[rgb(var(--foreground-muted))]" />
                      )}
                      <span className="truncate text-[12.5px] font-medium text-[rgb(var(--foreground))]">
                        {group.project.name}
                      </span>
                    </div>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.04)] shrink-0">
                      {group.sessions.length}
                    </span>
                  </div>

                  {/* Accordion Sessions */}
                  <AnimatePresence initial={false}>
                    {isExpanded && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: "auto" }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.18, ease: "easeInOut" }}
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
                              onSelect={handleSelect}
                              onTogglePin={togglePin}
                              onRename={renameSession}
                              onMoveToProject={moveSessionToProject}
                              onDelete={deleteSession}
                              allProjects={allProjects}
                            />
                          ))
                        ) : (
                          <span className="pl-7 py-1.5 text-[11px] text-[rgb(var(--foreground-muted))]/50 italic">
                            {SESSION_COPY.noProjectSessions}
                          </span>
                        )}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              );
            })
          ) : (
            <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
              {SESSION_COPY.noProjectsYet}
            </span>
          )}
        </section>

        {/* ── Uncategorized / Recent Sessions ── */}
        <section className="flex flex-col gap-1">
          <div className="px-2 py-0.5">
            <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {pinnedSessions.length > 0 || projects.some((p) => p.sessions.length > 0)
                ? SESSION_COPY.uncategorizedSection
                : SESSION_COPY.conversationsSection}
            </span>
          </div>
          <div className="flex flex-col gap-0.5">
            {uncategorizedSessions.length > 0 ? (
              uncategorizedSessions.map((session) => (
                <SessionRowItem
                  key={session.id}
                  session={session}
                  active={session.id === activeSessionId}
                  restoring={isRestoring && restoringSessionId === session.id}
                  pinned={false}
                  onSelect={handleSelect}
                  onTogglePin={togglePin}
                  onRename={renameSession}
                  onMoveToProject={moveSessionToProject}
                  onDelete={deleteSession}
                  allProjects={allProjects}
                />
              ))
            ) : (
              <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {pinnedSessions.length > 0 || projects.some((p) => p.sessions.length > 0)
                  ? SESSION_COPY.noOtherSessions
                  : SESSION_COPY.noSessionsTitle}
              </span>
            )}
          </div>
        </section>

        {/* Bottom spacing cushion */}
        <div className="h-8 shrink-0" aria-hidden="true" />
      </div>
    </div>
  );
});
SessionPanel.displayName = "SessionPanel";
