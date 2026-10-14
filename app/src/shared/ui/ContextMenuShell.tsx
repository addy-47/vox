import React, { memo, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";

export interface ContextMenuShellProps {
  open: boolean;
  onClose: () => void;
  anchorRect: DOMRect | null;
  triggerRef?: React.RefObject<HTMLElement | null>;
  width?: number;
  minHeight?: number;
  className?: string;
  children: React.ReactNode;
  role?: string;
}

/**
 * Shared ContextMenuShell (Tier 1 consolidation).
 * Provides single implementation for anchor positioning, viewport clamping,
 * Escape/ArrowUp/ArrowDown/Enter keyboard navigation, and outside-pointer dismissal.
 */
export const ContextMenuShell: React.FC<ContextMenuShellProps> = memo(
  ({
    open,
    onClose,
    anchorRect,
    triggerRef,
    width = 160,
    minHeight = 100,
    className,
    children,
    role = "menu",
  }) => {
    const menuRef = useRef<HTMLDivElement>(null);

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
          return;
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
      // Autofocus first item for keyboard navigation
      menuRef.current
        ?.querySelector<HTMLElement>('[role="menuitem"]')
        ?.focus({ preventScroll: true });

      return () => {
        window.removeEventListener("pointerdown", handlePointerDown, true);
        window.removeEventListener("keydown", handleKeyDown, true);
      };
    }, [open, onClose, triggerRef]);

    if (!open || !anchorRect || typeof document === "undefined") {
      return null;
    }

    // Viewport clamping
    let left = anchorRect.left;
    let top = anchorRect.bottom + 4;

    if (left + width > window.innerWidth - 12) {
      left = window.innerWidth - width - 12;
    }
    if (left < 12) {
      left = 12;
    }
    if (top + minHeight > window.innerHeight - 12) {
      top = Math.max(12, anchorRect.top - minHeight - 4);
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
            width: `${width}px`,
          }}
          onClick={(e) => e.stopPropagation()}
          data-context-menu
          role={role}
          aria-haspopup="menu"
          aria-expanded={open}
          className={cn(
            "z-[9999] rounded-sm border border-[rgba(var(--border),0.12)] bg-[rgb(var(--card))] shadow-lg p-1 flex flex-col gap-0.5 text-[12.5px] font-sans select-none",
            className
          )}
        >
          {children}
        </motion.div>
      </AnimatePresence>,
      document.body
    );
  }
);

ContextMenuShell.displayName = "ContextMenuShell";
