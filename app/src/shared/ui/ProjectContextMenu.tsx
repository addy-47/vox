import React, { memo, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { Pencil, Trash2, AlertCircle } from "lucide-react";
import { SESSION_COPY } from "@/data/sessionCopy";

export interface ProjectContextMenuProps {
  open: boolean;
  onClose: () => void;
  anchorRect: DOMRect | null;
  projectName: string;
  onStartRename: () => void;
  onDelete: () => Promise<void> | void;
}

export const ProjectContextMenu: React.FC<ProjectContextMenuProps> = memo(
  ({
    open,
    onClose,
    anchorRect,
    onStartRename,
    onDelete,
  }) => {
    const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      if (!open) {
        setIsConfirmingDelete(false);
        setIsDeleting(false);
        setErrorMessage(null);
      }
    }, [open]);

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
          menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []
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

    const handleConfirmDelete = async () => {
      setIsDeleting(true);
      setErrorMessage(null);
      try {
        await onDelete();
        onClose();
      } catch (err: unknown) {
        setIsDeleting(false);
        const msg = (err as { message?: string })?.message || String(err);
        if (msg.includes("session(s) exist") || msg.includes("Cannot delete")) {
          const match = msg.match(/(\d+)\s+session\(s\)\s+exist/);
          if (match) {
            setErrorMessage(`Cannot delete: ${match[1]} conversation(s) inside`);
          } else {
            setErrorMessage(SESSION_COPY.projectActions.notEmptyError);
          }
        } else {
          setErrorMessage(msg.slice(0, 50));
        }
      }
    };

    if (!open || !anchorRect || typeof document === "undefined") {
      return null;
    }

    const menuWidth = errorMessage ? 210 : (isConfirmingDelete ? 175 : 160);
    const menuHeight = errorMessage ? 96 : 76;

    let left = anchorRect.right + 6;
    let top = anchorRect.top - 4;

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
          className="z-[9999] rounded-xl border border-[rgba(var(--border),0.16)] bg-[rgb(var(--card))]/85 backdrop-blur-xl shadow-lg p-1 flex flex-col gap-0.5 text-[11.5px] font-sans select-none"
        >
          {/* Row 1: Rename project */}
          <button
            type="button"
            role="menuitem"
            tabIndex={-1}
            data-action="rename"
            disabled={isDeleting}
            onClick={() => {
              onStartRename();
              onClose();
            }}
            className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer disabled:opacity-40"
          >
            <Pencil size={12} className="text-[rgb(var(--foreground-muted))]" />
            <span>{SESSION_COPY.projectActions.rename}</span>
          </button>

          <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5" />

          {/* Row 2: Delete project (transitions in-place) */}
          {errorMessage ? (
            <div className="flex flex-col gap-1 p-1.5 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400">
              <div className="flex items-center gap-1.5 text-[10.5px] leading-tight">
                <AlertCircle size={12} className="shrink-0 text-red-400" />
                <span>{errorMessage}</span>
              </div>
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                onClick={() => {
                  setErrorMessage(null);
                  setIsConfirmingDelete(false);
                }}
                className="self-end px-1.5 py-0.5 rounded text-[10px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
              >
                {SESSION_COPY.projectActions.cancel}
              </button>
            </div>
          ) : isConfirmingDelete ? (
            <div className="flex items-center justify-between gap-1.5 px-2 py-1 rounded-lg bg-red-500/10 text-red-400">
              <span className="text-[11px] font-semibold">{SESSION_COPY.projectActions.deleteConfirm}</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  disabled={isDeleting}
                  onClick={() => setIsConfirmingDelete(false)}
                  className="px-1.5 py-0.5 rounded text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer disabled:opacity-40"
                >
                  {SESSION_COPY.projectActions.cancel}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  disabled={isDeleting}
                  onClick={handleConfirmDelete}
                  className="px-2 py-0.5 rounded text-[10.5px] font-mono font-bold bg-red-500/20 text-red-400 hover:bg-red-500/30 cursor-pointer disabled:opacity-40"
                >
                  {isDeleting ? "..." : SESSION_COPY.projectActions.confirm}
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              role="menuitem"
              tabIndex={-1}
              data-action="delete"
              onClick={() => {
                setErrorMessage(null);
                setIsConfirmingDelete(true);
              }}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-red-500/10 text-red-400 transition-colors cursor-pointer"
            >
              <Trash2 size={12} />
              <span>{SESSION_COPY.projectActions.delete}</span>
            </button>
          )}
        </motion.div>
      </AnimatePresence>,
      document.body
    );
  }
);

ProjectContextMenu.displayName = "ProjectContextMenu";
