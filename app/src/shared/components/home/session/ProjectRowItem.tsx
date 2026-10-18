import React, { memo, useCallback, useState, useRef } from "react";
import {
  Folder,
  FolderOpen,
  Plus,
  MoreVertical,
  Check,
} from "lucide-react";
import { Reorder, useDragControls } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { type ProjectGroup } from "@/shared/hooks/useSessionPanel";
import { type ProjectRow } from "@/services/projectsService";
import { SESSION_COPY } from "@/data/sessionCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ProjectContextMenu } from "@/shared/ui/ProjectContextMenu";
import { useSessionPanelUiStore } from "@/store/sessionPanelUiStore";
import { SessionRowItem } from "./SessionRowItem";
import { useRenderTrace } from "@/shared/hooks/useRenderTrace";

/**
 * How many of a project's conversations are mounted when it is expanded.
 *
 * Each session row mounts 2-3 floating-ui tooltips plus a context menu, so a
 * project holding 42 conversations mounted ~100 floating-ui setups. The panel
 * now stays mounted while closed, so that cost is paid on every open. Cap the
 * list, then reveal everything remaining in one click.
 */
const PROJECT_SESSION_LIMIT = 7;

export interface ProjectRowItemProps {
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
  onRenameProject: (projectId: string, newName: string) => void;
  onDeleteProject: (projectId: string) => Promise<void>;
  onDragOver: (e: React.DragEvent, projectId: string) => void;
  onDragLeave: (projectId: string) => void;
  onDrop: (e: React.DragEvent, projectId: string) => void;
  onDragEndCommit?: () => void;
}

export const ProjectRowItem = memo(
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
    onRenameProject,
    onDeleteProject,
    onDragOver,
    onDragLeave,
    onDrop,
    onDragEndCommit,
  }: ProjectRowItemProps) => {
    const projectId = group.project.id;
    const expanded = useSessionPanelUiStore((s) =>
      s.expandedProjectIds.includes(projectId)
    );
    const toggleProjectExpanded = useSessionPanelUiStore(
      (s) => s.toggleProjectExpanded
    );
    const [isDragging, setIsDragging] = useState(false);
    const [isRenaming, setIsRenaming] = useState(false);
    const [renameName, setRenameName] = useState("");
    const [menuOpen, setMenuOpen] = useState(false);
    const [menuAnchorRect, setMenuAnchorRect] = useState<DOMRect | null>(null);
    const [showAllSessions, setShowAllSessions] = useState(false);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const dragControls = useDragControls();
    const isDraggingRef = useRef(false);

    // [trace] RCA: which prop/state identity caused this project row to render.
    useRenderTrace(
      "ProjectRowItem",
      () => ({ projectId, sessions: group.sessions.length, expanded }),
      {
        watch: {
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
          onRenameProject,
          onDeleteProject,
          onDragOver,
          onDragLeave,
          onDrop,
          onDragEndCommit,
          expanded,
          isDragging,
          isRenaming,
          renameName,
          menuOpen,
          showAllSessions,
        },
        maxChangeLogs: 12,
      }
    );

    const handleToggleExpand = useCallback(() => {
      if (isDraggingRef.current || isRenaming || menuOpen) return;
      if (import.meta.env.DEV) {
        console.info(`[trace-first-open] Project expand intent project=${projectId} sessions=${group.sessions.length} expanding=${!expanded} @${performance.now().toFixed(1)}ms`);
      }
      toggleProjectExpanded(projectId);
      // Re-apply the row cap on every collapse, so reopening a project never
      // mounts its full session list again.
      if (expanded) setShowAllSessions(false);
    }, [isDraggingRef, isRenaming, menuOpen, toggleProjectExpanded, projectId, group.sessions.length, expanded]);

    const handleCreateInProject = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onCreateSessionInProject(group.project.id);
      },
      [group.project.id, onCreateSessionInProject]
    );

    const handleStartRename = useCallback(() => {
      setRenameName(group.project.name);
      setIsRenaming(true);
    }, [group.project.name]);

    const handleCommitRename = useCallback(() => {
      const trimmed = renameName.trim();
      if (trimmed && trimmed !== group.project.name) {
        onRenameProject(group.project.id, trimmed);
      }
      setIsRenaming(false);
    }, [renameName, group.project.name, onRenameProject, group.project.id]);

    const handleCancelRename = useCallback(() => {
      setIsRenaming(false);
    }, []);

    const handleDelete = useCallback(() => {
      return onDeleteProject(group.project.id);
    }, [onDeleteProject, group.project.id]);

    const handleMenuClick = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      if (triggerRef.current) {
        setMenuAnchorRect(triggerRef.current.getBoundingClientRect());
      }
      setMenuOpen((prev) => !prev);
    }, []);

    const hiddenSessionCount = expanded
      ? Math.max(0, group.sessions.length - PROJECT_SESSION_LIMIT)
      : 0;
    const visibleSessions =
      expanded && hiddenSessionCount > 0 && !showAllSessions
        ? group.sessions.slice(0, PROJECT_SESSION_LIMIT)
        : group.sessions;

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
          isDraggingRef.current = false;
          onDragEndCommit?.();
        }}
        onPointerUp={() => {
          if (isDraggingRef.current) {
            setIsDragging(false);
            isDraggingRef.current = false;
            onDragEndCommit?.();
          }
        }}
        onDragOver={(e) => onDragOver(e, group.project.id)}
        onDragLeave={() => onDragLeave(group.project.id)}
        onDrop={(e) => onDrop(e, group.project.id)}
        className={cn(
          "flex flex-col rounded-sm border border-transparent select-none relative",
          isDragging &&
            "z-50 bg-[rgb(var(--card))] shadow-[0_16px_36px_-4px_rgba(0,0,0,0.45),0_0_0_1px_rgba(var(--accent),0.4),0_0_24px_-2px_rgba(var(--accent),0.2)]",
          isDragTarget &&
            "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.12)] shadow-md"
        )}
        layout={false}
        whileDrag={{
          scale: 1,
          cursor: "grabbing",
        }}
        transition={{ duration: 0 }}
      >
        {/* Project Header Row: Flat folder, title / rename input, hover + to add session, hover 3-dots */}
        <div
          role="button"
          tabIndex={0}
          onPointerDown={(e) => {
            if (e.button === 0 && !(e.target as HTMLElement).closest("button") && !isRenaming) {
              dragControls.start(e);
            }
          }}
          onClick={handleToggleExpand}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") handleToggleExpand();
          }}
          className="group flex items-center justify-between gap-1.5 px-2 py-2 rounded-sm text-left cursor-grab active:cursor-grabbing hover:bg-[rgba(var(--foreground),0.04)] transition-colors select-none touch-none"
        >
          <div className="flex items-center gap-2 min-w-0 flex-1">
            {expanded ? (
              <FolderOpen
                size={13}
                className="shrink-0 text-[rgb(var(--accent))] transition-colors"
              />
            ) : (
              <Folder
                size={13}
                className="shrink-0 text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground-muted))] transition-colors"
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
                  value={renameName}
                  onChange={(e) => setRenameName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleCommitRename();
                    if (e.key === "Escape") handleCancelRename();
                  }}
                  onBlur={handleCommitRename}
                  className="w-full bg-transparent border-0 border-b border-[rgb(var(--accent))] px-0 py-0.5 text-[12px] font-sans text-[rgb(var(--foreground))] focus:outline-none"
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
              <span className="truncate text-[13.5px] font-medium text-[rgb(var(--foreground))]">
                {group.project.name}
              </span>
            )}
          </div>

          {!isRenaming && (
            <div className="flex items-center gap-1 shrink-0 relative justify-end">

              {/* + button to start conversation in this project */}
              <Tooltip label={SESSION_COPY.newInProjectAriaLabel} side="left" asChild>
                <button
                  type="button"
                  onClick={handleCreateInProject}
                  className={cn(
                    "items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--accent),0.1)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-all cursor-pointer",
                    expanded || menuOpen
                      ? "flex opacity-100"
                      : "hidden group-hover:flex text-[rgb(var(--foreground-muted))]"
                  )}
                  aria-label={SESSION_COPY.newInProjectAriaLabel}
                >
                  <Plus size={12} />
                </button>
              </Tooltip>

              {/* 3-dot action button */}
              <Tooltip label={SESSION_COPY.actions.moreOptions} side="top">
                <button
                  ref={triggerRef}
                  type="button"
                  onClick={handleMenuClick}
                  className={cn(
                    "items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                    expanded || menuOpen
                      ? "flex opacity-100 text-[rgb(var(--foreground))]"
                      : "hidden group-hover:flex opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
                  )}
                  aria-label={SESSION_COPY.actions.moreOptions}
                >
                  <MoreVertical size={13} />
                </button>
              </Tooltip>
            </div>
          )}
        </div>

        {/* Floating Project Context Menu Portal */}
        <ProjectContextMenu
          open={menuOpen}
          onClose={() => setMenuOpen(false)}
          anchorRect={menuAnchorRect}
          projectName={group.project.name}
          onStartRename={handleStartRename}
          onDelete={handleDelete}
          triggerRef={triggerRef}
        />

        {/* Clean Nested Sessions (Indented under project)

            Deliberately no height animation here. The previous
            `AnimatePresence` + `motion.div animate={{ height: "auto" }}` made
            framer-motion measure the whole subtree on every expand and
            collapse. Its transition was `duration: 0`, so the animation was
            already instant — the measurement bought nothing visible. */}
        {expanded && (
          <div className="flex flex-col gap-0.5">
            {visibleSessions.length > 0 ? (
              visibleSessions.map((session) => (
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
            {hiddenSessionCount > 0 && !showAllSessions && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setShowAllSessions(true);
                }}
                className="pl-6 py-1.5 text-left text-[11px] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
              >
                {SESSION_COPY.showMoreSessions(hiddenSessionCount)}
              </button>
            )}
          </div>
        )}
      </Reorder.Item>
    );
  }
);
ProjectRowItem.displayName = "ProjectRowItem";
