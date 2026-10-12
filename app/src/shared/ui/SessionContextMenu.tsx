import React, { memo, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  Pencil,
  FolderInput,
  Folder,
  Trash2,
  Shrink,
  Loader2,
  ChevronRight,
  Check,
  X,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SESSION_COPY } from "@/data/sessionCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
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
  triggerRef?: React.RefObject<HTMLElement | null>;
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
    triggerRef,
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
        const target = e.target as Node;
        if (triggerRef?.current && triggerRef.current.contains(target)) {
          return;
        }
        if (menuRef.current && !menuRef.current.contains(target)) {
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
      // Keyboard users land on the first item; mouse users keep :focus-visible clean.
      menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus({ preventScroll: true });
      return () => {
        window.removeEventListener("pointerdown", handlePointerDown, true);
        window.removeEventListener("keydown", handleKeyDown, true);
      };
    }, [open, onClose]);

    if (!open || !anchorRect || typeof document === "undefined") {
      return null;
    }

    // Position menu to the RIGHT of the anchor button (opening inward toward the page)
    const menuWidth = moveSubmenuOpen ? 180 : 160;
    const menuHeight = moveSubmenuOpen ? 180 : 130;

    // Position menu below the anchor button opening toward bottom-right
    let left = anchorRect.left;
    let top = anchorRect.bottom + 4;

    if (left + menuWidth > window.innerWidth - 12) {
      left = window.innerWidth - menuWidth - 12;
    }
    if (left < 12) {
      left = 12;
    }
    if (top + menuHeight > window.innerHeight - 12) {
      top = Math.max(12, anchorRect.top - menuHeight - 4);
    }

    return createPortal(
      <AnimatePresence>
        <motion.div
          ref={menuRef}
          initial={{ opacity: 0, scale: 0.96, y: -2 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.96, y: -2 }}
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
          className="z-[9999] rounded-sm border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))] shadow-lg backdrop-blur-md p-1 flex flex-col gap-0.5 text-[12.5px] font-sans select-none"
        >
          {moveSubmenuOpen ? (
            <div className="flex flex-col gap-0.5 max-h-[200px] overflow-y-auto custom-scrollbar">
              <div className="px-2 py-1 text-[10px] font-mono uppercase text-[rgb(var(--foreground-muted))]/70 border-b border-[rgba(var(--border),0.08)] flex items-center justify-between">
                <span>{SESSION_COPY.actions.moveToProject}</span>
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  data-action="back"
                  onClick={() => setMoveSubmenuOpen(false)}
                  className="px-1.5 py-0.5 rounded-sm flex items-center text-[10.5px] hover:text-[rgb(var(--foreground))] cursor-pointer font-bold text-[rgb(var(--accent))]"
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
                className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer text-[12.5px]"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.3)] shrink-0" />
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
                    "flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer text-[12.5px]",
                    currentProjectId === p.id && "text-[rgb(var(--accent))] font-semibold"
                  )}
                >
                  <Folder size={13} className="shrink-0" />
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
                    className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] font-medium transition-colors cursor-pointer text-[12.5px] disabled:opacity-50"
                  >
                    {isCompacting ? (
                      <Loader2 size={13} className="animate-spin text-[rgb(var(--accent))]" />
                    ) : (
                      <Shrink size={13} className="text-[rgb(var(--accent))]" />
                    )}
                    <span>
                      {isCompacting
                        ? SESSION_COPY.actions.compactingSession
                        : SESSION_COPY.actions.compactSession}
                    </span>
                  </button>
                  <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5 -mx-0.5" />
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
                className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer text-[12.5px]"
              >
                <Pencil size={13.5} className="text-[rgb(var(--foreground-muted))]" />
                <span>{SESSION_COPY.actions.rename}</span>
              </button>

              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                data-action="move-to-project"
                onClick={() => setMoveSubmenuOpen(true)}
                className="flex items-center justify-between px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer text-[12.5px]"
              >
                <div className="flex items-center gap-2">
                  <FolderInput size={13.5} className="text-[rgb(var(--foreground-muted))]" />
                  <span>{SESSION_COPY.actions.moveToProject}</span>
                </div>
                <ChevronRight size={13} className="text-[rgb(var(--foreground-muted))]/60" />
              </button>

              <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5 -mx-0.5" />

              {isConfirmingDelete ? (
                <div className="flex items-center justify-between px-2 py-1.5 rounded-sm text-[12.5px] select-none text-rose-400">
                  <div className="flex items-center gap-2">
                    <Trash2 size={13.5} />
                    <span className="font-medium">Delete</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <Tooltip label={SESSION_COPY.projectActions.confirm} side="top">
                      <button
                        type="button"
                        role="menuitem"
                        tabIndex={-1}
                        onClick={() => {
                          onDelete();
                          onClose();
                        }}
                        aria-label={SESSION_COPY.projectActions.confirm}
                        className="w-5 h-5 flex items-center justify-center rounded-sm text-[rgb(var(--foreground-muted))] hover:text-red-500 hover:bg-red-500/10 transition-colors cursor-pointer"
                      >
                        <Check size={13} />
                      </button>
                    </Tooltip>
                    <Tooltip label={SESSION_COPY.actions.cancel} side="top">
                      <button
                        type="button"
                        role="menuitem"
                        tabIndex={-1}
                        onClick={() => setIsConfirmingDelete(false)}
                        aria-label={SESSION_COPY.actions.cancel}
                        className="w-5 h-5 flex items-center justify-center rounded-sm text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                      >
                        <X size={13} />
                      </button>
                    </Tooltip>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  data-action="delete"
                  onClick={() => setIsConfirmingDelete(true)}
                  className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-rose-500/10 text-rose-400 transition-colors cursor-pointer text-[12.5px]"
                >
                  <Trash2 size={13.5} />
                  <span>{SESSION_COPY.actions.delete}</span>
                </button>
              )}
            </>
          )}
        </motion.div>
      </AnimatePresence>,
      document.body
    );
  }
);
SessionContextMenu.displayName = "SessionContextMenu";
