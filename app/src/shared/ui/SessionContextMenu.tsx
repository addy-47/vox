import React, { memo, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  Pencil,
  FolderInput,
  Folder,
  Trash2,
  Sparkles,
  Loader2,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SESSION_COPY } from "@/data/sessionCopy";
import type { ProjectRow } from "@/services/projectService";

export interface SessionContextMenuProps {
  open: boolean;
  onClose: () => void;
  anchorRect: DOMRect | null;
  sessionId: number;
  currentProjectId: string | null;
  allProjects: ProjectRow[];
  isUncompacted: boolean;
  isCompacting: boolean;
  onCompact: () => void;
  onStartRename: () => void;
  onMoveToProject: (projectId: string | null) => void;
  onDelete: () => void;
}

export const SessionContextMenu: React.FC<SessionContextMenuProps> = memo(
  ({
    open,
    onClose,
    anchorRect,
    sessionId,
    currentProjectId,
    allProjects,
    isUncompacted,
    isCompacting,
    onCompact,
    onStartRename,
    onMoveToProject,
    onDelete,
  }) => {
    const [moveSubmenuOpen, setMoveSubmenuOpen] = React.useState(false);
    const [isConfirmingDelete, setIsConfirmingDelete] = React.useState(false);
    const menuRef = useRef<HTMLDivElement>(null);

    // Reset internal state when menu closes or target changes
    useEffect(() => {
      if (!open) {
        setMoveSubmenuOpen(false);
        setIsConfirmingDelete(false);
      }
    }, [open, sessionId]);

    // Handle outside click & escape to close
    useEffect(() => {
      if (!open) return;

      const handlePointerDown = (e: PointerEvent) => {
        if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
          onClose();
        }
      };

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }

        const menuItems = Array.from(
          menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
        );
        if (menuItems.length === 0) return;

        const currentFocus = document.activeElement as HTMLElement;
        const currentIndex = menuItems.indexOf(currentFocus);

        if (e.key === "ArrowDown") {
          e.preventDefault();
          const next = currentIndex < menuItems.length - 1 ? currentIndex + 1 : 0;
          menuItems[next]?.focus();
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          const prev = currentIndex > 0 ? currentIndex - 1 : menuItems.length - 1;
          menuItems[prev]?.focus();
        } else if (e.key === "Enter") {
          e.preventDefault();
          if (currentFocus && currentFocus.getAttribute("data-action")) {
            currentFocus.click();
          }
        }
      };

      window.addEventListener("pointerdown", handlePointerDown, true);
      window.addEventListener("keydown", handleKeyDown, true);
      return () => {
        window.removeEventListener("pointerdown", handlePointerDown, true);
        window.removeEventListener("keydown", handleKeyDown, true);
      };
    }, [open, onClose]);

    if (!open || !anchorRect || typeof document === "undefined") {
      return null;
    }

    // Position menu to the RIGHT of the anchor button (opening inward toward the page)
    const menuWidth = 176;
    const menuHeight = isConfirmingDelete ? 90 : moveSubmenuOpen ? 220 : 160;

    let left = anchorRect.right + 6;
    let top = anchorRect.top - 4;

    // Viewport bounds check
    if (left + menuWidth > window.innerWidth - 12) {
      left = anchorRect.left - menuWidth - 6;
    }
    if (top + menuHeight > window.innerHeight - 12) {
      top = Math.max(12, window.innerHeight - menuHeight - 12);
    }

    return createPortal(
      <AnimatePresence>
        <motion.div
          ref={menuRef}
          initial={{ opacity: 0, scale: 0.96, x: -4 }}
          animate={{ opacity: 1, scale: 1, x: 0 }}
          exit={{ opacity: 0, scale: 0.96, x: -4 }}
          transition={{ duration: 0.12 }}
          style={{
            position: "fixed",
            left: `${Math.round(left)}px`,
            top: `${Math.round(top)}px`,
            width: `${menuWidth}px`,
          }}
          onClick={(e) => e.stopPropagation()}
          data-context-menu
          role="menu"
          aria-haspopup="menu"
          aria-expanded={open}
          className="z-[9999] rounded-xl border border-[rgba(var(--border),0.16)] bg-[rgb(var(--card))]/75 backdrop-blur-xl shadow-sm p-1 flex flex-col gap-0.5 text-[11.5px] font-sans select-none"
        >
          {isConfirmingDelete ? (
            <div className="p-2 flex flex-col gap-2">
              <span className="text-[11px] font-semibold text-red-400 leading-tight">
                {SESSION_COPY.actions.deleteConfirm}
              </span>
              <div className="flex items-center gap-1.5 justify-end">
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  data-action="cancel"
                  onClick={() => setIsConfirmingDelete(false)}
                  className="px-2 py-1 rounded text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
                >
                  {SESSION_COPY.actions.cancel}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  data-action="delete-confirm"
                  onClick={() => {
                    onDelete();
                    onClose();
                  }}
                  className="px-2 py-1 rounded text-[10.5px] font-mono font-bold bg-red-500/20 text-red-400 hover:bg-red-500/30 cursor-pointer"
                >
                  Delete
                </button>
              </div>
            </div>
          ) : moveSubmenuOpen ? (
            <div className="flex flex-col gap-0.5 max-h-[220px] overflow-y-auto custom-scrollbar">
              <div className="px-2 py-1 text-[10px] font-mono uppercase text-[rgb(var(--foreground-muted))]/70 border-b border-[rgba(var(--border),0.08)] flex items-center justify-between">
                <span>{SESSION_COPY.actions.moveToProject}</span>
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  data-action="back"
                  onClick={() => setMoveSubmenuOpen(false)}
                  className="text-[9px] hover:text-[rgb(var(--foreground))] cursor-pointer font-bold text-[rgb(var(--accent))]"
                >
                  Back
                </button>
              </div>
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                data-action="remove-from-project"
                onClick={() => {
                  onMoveToProject(null);
                  onClose();
                }}
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.3)]" />
                <span className="truncate">{SESSION_COPY.actions.removeFromProject}</span>
              </button>
                {allProjects.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    role="menuitem"
                    tabIndex={-1}
                    data-action={`move-to-project-${p.id}`}
                    onClick={() => {
                      onMoveToProject(p.id);
                      onClose();
                    }}
                    className={cn(
                      "flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer",
                      currentProjectId === p.id && "text-[rgb(var(--accent))] font-semibold"
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
                    role="menuitem"
                    tabIndex={-1}
                    data-action="compact-session"
                    disabled={isCompacting}
                    onClick={() => {
                      onCompact();
                      onClose();
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
                role="menuitem"
                tabIndex={-1}
                data-action="rename"
                onClick={() => {
                  onStartRename();
                  onClose();
                }}
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <Pencil size={12} className="text-[rgb(var(--foreground-muted))]" />
                <span>{SESSION_COPY.actions.rename}</span>
              </button>

              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                data-action="move-to-project"
                onClick={() => setMoveSubmenuOpen(true)}
                className="flex items-center justify-between px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-2">
                  <FolderInput size={12} className="text-[rgb(var(--foreground-muted))]" />
                  <span>{SESSION_COPY.actions.moveToProject}</span>
                </div>
                <ChevronRight size={12} className="text-[rgb(var(--foreground-muted))]/60" />
              </button>

              <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5" />

              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                data-action="delete"
                onClick={() => setIsConfirmingDelete(true)}
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-red-500/10 text-red-400 transition-colors cursor-pointer"
              >
                <Trash2 size={12} />
                <span>{SESSION_COPY.actions.delete}</span>
              </button>
            </>
          )}
        </motion.div>
      </AnimatePresence>,
      document.body
    );
  }
);
SessionContextMenu.displayName = "SessionContextMenu";
