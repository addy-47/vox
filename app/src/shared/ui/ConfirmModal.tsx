import { memo, useRef } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { OverlayBackdrop } from "@/shared/ui/OverlayBackdrop";

export interface ConfirmModalProps {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  isDestructive?: boolean;
  isLoading?: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
}

/**
 * Single-surface confirmation dialog strictly matching Antigravity design spec (Image 2).
 * Standalone overlay avoiding multi-layer borders, nested clipping, or scrollbar artifacts.
 */
export const ConfirmModal = memo(
  ({
    open,
    title,
    description,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    isDestructive = true,
    isLoading = false,
    onConfirm,
    onCancel,
  }: ConfirmModalProps) => {
    const dialogRef = useRef<HTMLDivElement>(null);

    useOverlay({
      onClose: isLoading ? () => {} : onCancel,
      active: open,
      ref: dialogRef,
    });

    if (typeof document === "undefined") return null;

    return createPortal(
      <AnimatePresence>
        {open && (
          <div
            className="fixed inset-0 z-60 pointer-events-none flex items-center justify-center p-4 sm:p-6"
            role="presentation"
          >
            <OverlayBackdrop onClose={isLoading ? undefined : onCancel} className="absolute cursor-default" />

            <motion.div
              ref={dialogRef}
              role="alertdialog"
              aria-modal="true"
              aria-labelledby="confirm-modal-title"
              aria-describedby="confirm-modal-desc"
              tabIndex={-1}
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.96 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              className="relative w-[min(440px,92vw)] p-6 rounded-2xl bg-[rgb(var(--card))] border border-[rgba(var(--foreground),0.08)] shadow-[0_20px_50px_rgba(0,0,0,0.6)] pointer-events-auto select-none"
              onClick={(e) => e.stopPropagation()}
            >
              {/* Header: Title & Close Button */}
              <div className="flex items-center justify-between gap-3">
                <h3
                  id="confirm-modal-title"
                  className="text-[15px] font-semibold text-[rgb(var(--foreground))] tracking-tight"
                >
                  {title}
                </h3>
                <button
                  type="button"
                  aria-label="Close"
                  disabled={isLoading}
                  onClick={onCancel}
                  className="p-1.5 -mr-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer disabled:opacity-50"
                >
                  <X size={16} />
                </button>
              </div>

              {/* Description Body */}
              <p
                id="confirm-modal-desc"
                className="mt-2.5 text-[13px] leading-relaxed text-[rgb(var(--foreground-muted))] font-normal select-text"
              >
                {description}
              </p>

              {/* Actions Footer */}
              <div className="mt-6 flex items-center justify-end gap-2.5">
                <button
                  type="button"
                  onClick={onCancel}
                  disabled={isLoading}
                  className="px-4 py-2 rounded-xl text-[13px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.06)] hover:bg-[rgba(var(--foreground),0.12)] border border-[rgba(var(--border),0.1)] active:scale-98 transition-all cursor-pointer disabled:opacity-50"
                >
                  {cancelLabel}
                </button>
                <button
                  type="button"
                  onClick={() => void onConfirm()}
                  disabled={isLoading}
                  className={cn(
                    "px-4 py-2 rounded-xl text-[13px] font-semibold transition-all cursor-pointer disabled:opacity-50 active:scale-98 shadow-sm",
                    isDestructive
                      ? "bg-[rgb(var(--danger))] text-white hover:brightness-110 shadow-[0_0_16px_rgba(var(--danger),0.25)]"
                      : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-105"
                  )}
                >
                  {confirmLabel}
                </button>
              </div>
            </motion.div>
          </div>
        )}
      </AnimatePresence>,
      document.body
    );
  }
);

ConfirmModal.displayName = "ConfirmModal";
