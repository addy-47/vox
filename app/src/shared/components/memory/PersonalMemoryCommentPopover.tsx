import React, { useState, useEffect, useRef, memo } from "react";
import { MessageSquarePlus } from "lucide-react";
import { cn } from "@/shared/lib/utils";

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
          /* Sleek borderless trigger button at bottom-right of highlighted text */
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setIsOpen(true);
              onOpenChange?.(true);
            }}
            className="text-[rgb(var(--accent))] hover:scale-115 flex items-center justify-center cursor-pointer transition-transform p-1 opacity-90 hover:opacity-100 drop-shadow-sm"
            title="Add Comment"
          >
            <MessageSquarePlus size={16} />
          </button>
        ) : (
          /* Clean minimal Antigravity-style comment box */
          <div
            className="w-[280px] sm:w-[320px] rounded-lg border border-[rgba(var(--border),0.2)] shadow-2xl p-2.5 flex flex-col gap-2 animate-in fade-in zoom-in-95 duration-100 text-[rgb(var(--foreground))]"
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
              }}
              rows={2}
              placeholder="Leave a comment"
              className="w-full bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.14)] focus:border-[rgba(var(--accent),0.5)] rounded-md p-2 text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 resize-none focus:outline-none transition-colors leading-relaxed font-sans"
            />

            <div className="flex items-center justify-between pt-0.5">
              <button
                type="button"
                onClick={handleDiscard}
                className="px-1.5 py-0.5 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                Cancel
              </button>

              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => handleSubmit()}
                  disabled={!commentText.trim()}
                  className={cn(
                    "px-3 py-1 rounded-md text-[11.5px] font-medium font-sans transition-all flex items-center justify-center cursor-pointer",
                    commentText.trim()
                      ? "bg-[rgb(var(--accent))] text-black hover:opacity-90 active:scale-95 shadow-xs"
                      : "bg-[rgba(var(--accent),0.12)] text-[rgba(var(--accent),0.3)] cursor-not-allowed"
                  )}
                >
                  <span>Add Comment</span>
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    );
  }
);

PersonalMemoryCommentPopover.displayName = "PersonalMemoryCommentPopover";
