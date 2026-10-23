import React, { memo, useState } from "react";
import { Pencil, Trash2, AlertCircle, Check, X } from "lucide-react";
import { SESSION_COPY } from "@/data/sessionCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ContextMenuShell } from "@/shared/ui/ContextMenuShell";

export interface ProjectContextMenuProps {
  open: boolean;
  onClose: () => void;
  anchorRect: DOMRect | null;
  projectName: string;
  onStartRename: () => void;
  onDelete: () => Promise<void> | void;
  triggerRef?: React.RefObject<HTMLElement | null>;
}

export const ProjectContextMenu: React.FC<ProjectContextMenuProps> = memo(
  ({
    open,
    onClose,
    anchorRect,
    onStartRename,
    onDelete,
    triggerRef,
  }) => {
    const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);

    React.useEffect(() => {
      if (!open) {
        setIsConfirmingDelete(false);
        setIsDeleting(false);
        setErrorMessage(null);
      }
    }, [open]);

    const handleConfirmDelete = async () => {
      setIsDeleting(true);
      setErrorMessage(null);
      try {
        await onDelete();
        onClose();
      } catch (err: unknown) {
        setIsDeleting(false);
        const msg = (err as { message?: string }).message || String(err);
        if (msg.includes("session(s) exist") || msg.includes("Cannot delete")) {
          const match = msg.match(/(\d+)\s+session\(s\)\s+exist/);
          if (match) {
            setErrorMessage(`Cannot delete: ${match[1]} conversation(s) inside`);
          } else {
            setErrorMessage(SESSION_COPY.projectActions.notEmptyError);
          }
        } else {
          setErrorMessage(msg);
        }
      }
    };

    const menuWidth = errorMessage ? 220 : 160;
    const menuHeight = errorMessage ? 105 : 76;

    return (
      <ContextMenuShell
        open={open}
        onClose={onClose}
        anchorRect={anchorRect}
        triggerRef={triggerRef}
        width={menuWidth}
        minHeight={menuHeight}
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
          className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground))] transition-colors cursor-pointer text-[12.5px] disabled:opacity-40 select-none"
        >
          <Pencil size={13.5} className="text-[rgb(var(--foreground-muted))]" />
          <span>{SESSION_COPY.projectActions.rename}</span>
        </button>

        <div className="h-[1px] bg-[rgba(var(--border),0.08)] my-0.5 -mx-0.5" />

        {/* Row 2: Delete project (transitions in-place) */}
        {errorMessage ? (
          <div className="flex flex-col gap-1 p-1.5 rounded-sm bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[rgb(var(--error))]">
            <div className="flex items-center gap-1.5 text-[11px] leading-tight">
              <AlertCircle size={13} className="shrink-0 text-[rgb(var(--error))]" />
              <span className="break-words min-w-0">{errorMessage}</span>
            </div>
            <button
              type="button"
              role="menuitem"
              tabIndex={-1}
              onClick={() => {
                setErrorMessage(null);
                setIsConfirmingDelete(false);
              }}
              className="self-end px-2 py-0.5 rounded-sm text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
            >
              {SESSION_COPY.projectActions.cancel}
            </button>
          </div>
        ) : isConfirmingDelete ? (
          <div className="flex items-center justify-between px-2 py-1.5 rounded-sm text-[12.5px] select-none text-[rgb(var(--danger))]">
            <div className="flex items-center gap-2">
              <Trash2 size={13.5} />
              <span className="font-medium">{SESSION_COPY.projectActions.confirm}</span>
            </div>
            <div className="flex items-center gap-1">
              <Tooltip label={SESSION_COPY.projectActions.confirm} side="top">
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  disabled={isDeleting}
                  onClick={handleConfirmDelete}
                  aria-label={SESSION_COPY.projectActions.confirm}
                  className="w-5 h-5 flex items-center justify-center rounded-sm text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.12)] transition-colors cursor-pointer disabled:opacity-40"
                >
                  <Check size={13} />
                </button>
              </Tooltip>
              <Tooltip label={SESSION_COPY.projectActions.cancel} side="top">
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
                  disabled={isDeleting}
                  onClick={() => setIsConfirmingDelete(false)}
                  aria-label={SESSION_COPY.projectActions.cancel}
                  className="w-5 h-5 flex items-center justify-center rounded-sm text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer disabled:opacity-40"
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
            onClick={() => {
              setErrorMessage(null);
              setIsConfirmingDelete(true);
            }}
            className="flex items-center gap-2 px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--danger),0.1)] text-[rgb(var(--danger))] transition-colors cursor-pointer text-[12.5px] select-none"
          >
            <Trash2 size={13.5} />
            <span>{SESSION_COPY.projectActions.delete}</span>
          </button>
        )}
      </ContextMenuShell>
    );
  }
);

ProjectContextMenu.displayName = "ProjectContextMenu";
