import React, { useState, useEffect, useRef, memo } from "react";
import { MessageSquarePlus } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface SelectionRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

export interface SelectionAnchor {
  line: number;
  quotedText: string;
  top: number;
  left: number;
  right: number;
  bottom: number;
  rects: SelectionRect[];
}

export interface PersonalMemoryCommentPopoverProps {
  anchor: SelectionAnchor | null;
  onAddComment: (comment: { line: number; quotedText: string; text: string; top: number }) => void;
  onCancel: () => void;
  onOpenChange?: (isOpen: boolean) => void;
}

export const PersonalMemoryCommentPopover: React.FC<PersonalMemoryCommentPopoverProps> = memo(
  ({ anchor, onAddComment, onCancel, onOpenChange }) => {
    const [isOpen, setIsOpen] = useState(false);
    const [commentText, setCommentText] = useState("");
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const popoverRef = useRef<HTMLDivElement>(null);

    // Reset state when anchor changes
    useEffect(() => {
      if (anchor) {
        setIsOpen(false);
        setCommentText("");
        onOpenChange?.(false);
      }
    }, [anchor, onOpenChange]);

    // Focus textarea when opened
    useEffect(() => {
      if (!isOpen) return undefined;
      onOpenChange?.(true);
      const timer = setTimeout(() => {
        textareaRef.current?.focus();
      }, 30);
      return () => clearTimeout(timer);
    }, [isOpen, onOpenChange]);

    // Close on click outside when open
    useEffect(() => {
      if (!isOpen) return undefined;
      const handleClickOutside = (e: MouseEvent) => {
        if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
          setIsOpen(false);
          onOpenChange?.(false);
          onCancel();
        }
      };
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [isOpen, onCancel, onOpenChange]);

    if (!anchor) return null;

    const handleSubmit = (e?: React.FormEvent) => {
      e?.preventDefault();
      if (!commentText.trim()) return;
      onAddComment({
        line: anchor.line,
        quotedText: anchor.quotedText,
        text: commentText.trim(),
        top: anchor.top,
      });
      setCommentText("");
      setIsOpen(false);
      onOpenChange?.(false);
    };

    const handleDiscard = () => {
      setIsOpen(false);
      onOpenChange?.(false);
      onCancel();
    };

    return (
      <div
        ref={popoverRef}
        className="absolute z-50 pointer-events-auto"
        style={{
          top: `${anchor.bottom + 6}px`,
          left: `${Math.max(12, Math.min(isOpen ? anchor.left : anchor.right - 28, window.innerWidth - 340))}px`,
        }}
      >
        {!isOpen ? (
          /* Sleek opaque button at bottom-right corner of highlighted text */
          <button
            type="button"
            onMouseDown={(e) => {
              // Prevent document mousedown from collapsing selection before opening
              e.preventDefault();
              e.stopPropagation();
              setIsOpen(true);
              onOpenChange?.(true);
            }}
            className="w-7 h-7 rounded-lg bg-[rgb(var(--card))] border border-[rgb(var(--accent))] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.15)] shadow-xl flex items-center justify-center cursor-pointer transition-transform duration-150 hover:scale-110 active:scale-95"
            style={{ backgroundColor: "rgb(var(--card))" }}
            title={MEMORY_COPY.addComment}
          >
            <MessageSquarePlus size={15} strokeWidth={2.2} />
          </button>
        ) : (
          /* Minimal inline comment card: auto-focused textarea + Discard & Save buttons */
          <div
            className="w-[280px] sm:w-[310px] rounded-xl border border-[rgba(var(--accent),0.4)] shadow-2xl p-3 flex flex-col gap-2.5 animate-in fade-in zoom-in-95 duration-150 text-[rgb(var(--foreground))]"
            style={{ backgroundColor: "rgb(var(--card))" }}
          >
            <textarea
              ref={textareaRef}
              autoFocus
              value={commentText}
              onChange={(e) => setCommentText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSubmit();
                } else if (e.key === "Escape") {
                  handleDiscard();
                }
                // Shift+Enter falls through to textarea default → newline
              }}
              rows={3}
              placeholder={MEMORY_COPY.leaveCommentHint}
              className="w-full bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.2)] focus:border-[rgb(var(--accent))] rounded-lg p-2.5 text-[12.5px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 resize-none focus:outline-none transition-colors leading-relaxed font-sans"
            />

            <div className="flex items-center justify-end gap-2 pt-0.5">
              <button
                type="button"
                onClick={handleDiscard}
                className="px-2.5 py-1 text-[11.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] rounded-lg transition-colors cursor-pointer"
              >
                Discard
              </button>

              <button
                type="button"
                onClick={() => handleSubmit()}
                disabled={!commentText.trim()}
                className={cn(
                  "px-3.5 py-1.5 rounded-lg text-[11.5px] font-mono font-semibold shadow-sm transition-all cursor-pointer flex items-center justify-center min-w-[56px]",
                  commentText.trim()
                    ? "bg-[rgba(var(--accent),0.22)] border border-[rgba(var(--accent),0.45)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.32)] active:scale-95 shadow-md"
                    : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))]/40 border border-[rgba(var(--border),0.12)] cursor-not-allowed"
                )}
              >
                <span>Save</span>
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }
);

PersonalMemoryCommentPopover.displayName = "PersonalMemoryCommentPopover";
