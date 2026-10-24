import React, { memo, useState, useRef, useCallback, useMemo } from "react";
import {
  Pin,
  Folder,
  Loader2,
  Check,
  MoreVertical,
  Shrink,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SessionContextMenu } from "@/shared/ui/SessionContextMenu";
import {
  resolveSessionTitle,
  sessionLastActivity,
  formatSessionRecency,
  type SessionRow,
} from "@/services/persistenceService";
import { type ProjectRow } from "@/services/projectsService";
import { SESSION_COPY } from "@/data/sessionCopy";
import {
  useNotificationStore,
  selectUncompactedSessionIds,
  selectActiveCompactionSessionIds,
} from "@/store/notificationStore";
import { useRenderTrace } from "@/shared/hooks/useRenderTrace";

export interface SessionRowItemProps {
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

export const SessionRowItem = memo(
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
        (s) => selectActiveCompactionSessionIds(s).has(session.id),
        [session.id]
      )
    );

    const executeCompaction = useNotificationStore(
      (s) => s.executeCompactionForSession
    );

    // [trace] RCA: which prop/store/local-state identity caused this row to
    // render. Capped: there can be ~40 rows, so a storm would flood the console.
    useRenderTrace(
      "SessionRowItem",
      () => ({ id: session.id, active, pinned, nested }),
      {
        watch: {
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
          nested,
          projectTag,
          showRecency,
          isUncompacted,
          isCompacting,
          isRenaming,
          renameTitle,
          menuOpen,
        },
        maxChangeLogs: 3,
        heartbeat: 50,
      }
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
      void executeCompaction(session.id);
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
          "group relative flex items-center justify-between gap-1.5 py-2 rounded-sm text-left cursor-pointer transition-colors select-none border border-transparent overflow-hidden",
          active
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] font-medium shadow-sm"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]",
          restoring && "opacity-60 cursor-wait animate-pulse",
          nested ? "pl-6 pr-2" : "px-2"
        )}
      >
        <div className="flex items-center gap-2 min-w-0 flex-1">
          {restoring && (
            <Loader2
              size={13}
              className="shrink-0 animate-spin text-[rgb(var(--accent))]"
              aria-label={SESSION_COPY.restoringAriaLabel}
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
            <div className="flex flex-col min-w-0 flex-1">
              <div className="flex items-center gap-1.5 min-w-0">
                <span
                  className={cn(
                    "truncate text-[13.5px] font-medium tracking-normal leading-tight font-sans",
                    active
                      ? "text-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground))]"
                  )}
                >
                  {title}
                </span>
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

        {/* Actions / Recency Area */}
        {!isRenaming && (
          <div className="flex items-center gap-0.5 shrink-0 relative min-w-[36px] justify-end">
            {/* Static Date/Time: visible by default, replaced on hover or when menu is open */}
            {showRecency && (
              <span
                className={cn(
                  "text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 transition-opacity mr-1 whitespace-nowrap",
                  menuOpen ? "hidden" : "group-hover:hidden"
                )}
              >
                {recency}
              </span>
            )}

            {/* Interactive Action Buttons: hidden by default, visible on group hover or when menu is open */}
            <div
              className={cn(
                "items-center gap-0.5",
                menuOpen ? "flex" : "hidden group-hover:flex"
              )}
            >
              {/* Compact Trigger (uncompacted sessions only) */}
              {isUncompacted && (
                <Tooltip label={SESSION_COPY.actions.compactSession} side="top">
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      e.preventDefault();
                      void executeCompaction(session.id);
                    }}
                    aria-label={SESSION_COPY.actions.uncompactedTurnsTooltip}
                    className="flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--accent),0.12)] transition-all cursor-pointer text-[rgb(var(--accent))] opacity-60 hover:!opacity-100"
                  >
                    {isCompacting ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <Shrink size={12} />
                    )}
                  </button>
                </Tooltip>
              )}

              {/* Pin Toggle */}
              <Tooltip label={pinned ? SESSION_COPY.unpinAriaLabel : SESSION_COPY.pinAriaLabel} side="top">
                <button
                  type="button"
                  onClick={handlePin}
                  aria-pressed={pinned}
                  aria-label={pinned ? SESSION_COPY.unpinAriaLabel : SESSION_COPY.pinAriaLabel}
                  className={cn(
                    "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                    pinned
                      ? "text-[rgb(var(--accent))] opacity-100"
                      : "opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
                  )}
                >
                  <Pin size={12} fill={pinned ? "currentColor" : "none"} />
                </button>
              </Tooltip>

              {/* Vertical Three Dots Context Menu Trigger */}
              <Tooltip label={SESSION_COPY.actions.moreOptions} side="top">
                <button
                  ref={triggerRef}
                  type="button"
                  onClick={handleMenuClick}
                  className={cn(
                    "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
                    menuOpen
                      ? "opacity-100 text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.08)]"
                      : "opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
                  )}
                  aria-label={SESSION_COPY.actions.moreOptions}
                >
                  <MoreVertical size={13} />
                </button>
              </Tooltip>
            </div>
          </div>
        )}

        {/* Floating Context Menu Portal (Opens inward/rightwards) */}
        {menuOpen && (
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
            triggerRef={triggerRef}
          />
        )}
      </div>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";
