import React, { useMemo, memo } from "react";
import { motion } from "framer-motion";
import { Trash2, Check, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { type SessionRow } from "@/services/historyService";

function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export interface VoiceRippleNodeProps {
  session: SessionRow;
  isSelected: boolean;
  isConfirmingDelete: boolean;
  onSelect: (session: SessionRow) => void;
  onDelete: (e: React.MouseEvent, id: number) => void;
  onCancelDelete: (e: React.MouseEvent) => void;
  x: number;
  y: number;
}

export const VoiceRippleNode = memo(
  ({
    session,
    isSelected,
    isConfirmingDelete,
    onSelect,
    onDelete,
    onCancelDelete,
    x,
    y,
  }: VoiceRippleNodeProps) => {
    const previewText = useMemo(() => {
      const msg = session.first_message || "No transcript recorded";
      const words = msg.trim().split(/\s+/);
      if (words.length <= 6) return msg;
      return words.slice(0, 6).join(" ") + "...";
    }, [session.first_message]);

    return (
      <motion.div
        initial={{ opacity: 0, scale: 0.8 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.8 }}
        transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
        style={{
          position: "absolute",
          left: x - 112,
          top: y - 60,
        }}
        className={cn(
          "w-56 rounded-2xl p-4 flex flex-col text-left transition-colors duration-300 select-none group cursor-pointer z-10 glass-card",
          isSelected
            ? "border-[rgba(var(--accent),0.6)] bg-[rgba(var(--accent),0.12)] shadow-lg"
            : "hover:bg-[rgba(var(--accent),0.04)]"
        )}
        onClick={(e) => {
          e.stopPropagation();
          onSelect(session);
        }}
      >
        {/* Dynamic pulsing background ring — active only when selected */}
        <div className="absolute top-4 left-4 w-2.5 h-2.5 rounded-full bg-[rgb(var(--accent))]">
          {isSelected && (
            <div className="absolute inset-[-4px] rounded-full border border-[rgb(var(--accent))]/40 animate-ping" />
          )}
        </div>

        {/* Top Info */}
        <div className="flex items-center justify-between mb-2 pl-4">
          <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">
            {formatTime(session.started_at)}
          </span>
          <span className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))]">
            {session.turn_count} {session.turn_count === 1 ? "turn" : "turns"}
          </span>
        </div>

        {/* Snippet */}
        <p className="text-[13px] font-normal leading-relaxed italic text-[rgb(var(--foreground))] pl-4">
          "{previewText}"
        </p>

        {/* Delete button (minimum 28px hit target) */}
        <div className={cn("absolute top-2.5 right-2.5 transition-opacity duration-200 z-20", isConfirmingDelete ? "opacity-100" : "opacity-0 group-hover:opacity-100")}>
          {isConfirmingDelete ? (
            <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
              <button
                onClick={(e) => onDelete(e, session.id)}
                className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 cursor-pointer shadow-md"
                aria-label="Confirm delete"
              >
                <Check size={14} strokeWidth={2.5} />
              </button>
              <button
                onClick={onCancelDelete}
                className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shadow-md"
                aria-label="Cancel delete"
              >
                <X size={14} strokeWidth={2.5} />
              </button>
            </div>
          ) : (
            <button
              onClick={(e) => onDelete(e, session.id)}
              className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer"
              aria-label="Delete session"
            >
              <Trash2 size={14} />
            </button>
          )}
        </div>
      </motion.div>
    );
  }
);

VoiceRippleNode.displayName = "VoiceRippleNode";
