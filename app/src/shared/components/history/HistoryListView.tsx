import React, { memo } from "react";
import { Check, X, Trash2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { formatDateTime, type SessionRow } from "@/services/historyService";
import { HISTORY_COPY } from "@/data/historyCopy";

export interface HistoryListViewProps {
  dayLabel: string;
  sessions: SessionRow[];
  selectedSession: SessionRow | null;
  confirmDeleteId: number | null;
  canPrevDate?: boolean;
  canNextDate?: boolean;
  onPrevDate?: () => void;
  onNextDate?: () => void;
  onSelect: (session: SessionRow) => void;
  onDelete: (e: React.MouseEvent, id: number) => void;
  onCancelDelete: (e: React.MouseEvent) => void;
}

export const HistoryListView = memo(
  ({
    dayLabel,
    sessions,
    selectedSession,
    confirmDeleteId,
    canPrevDate = false,
    canNextDate = false,
    onPrevDate,
    onNextDate,
    onSelect,
    onDelete,
    onCancelDelete,
  }: HistoryListViewProps) => {
    return (
      <div className="w-full flex-1 overflow-y-auto px-4 pt-4 pb-28 space-y-3 custom-scrollbar z-20">
        {/* Header with Title and Carousel Chevrons in Same Row */}
        <div className="flex items-center justify-between py-2 px-1 mb-2 shrink-0">
          <div className="flex flex-col">
            <h1 className="text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {HISTORY_COPY.headerTitle}
            </h1>
            <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
              {dayLabel}
            </span>
          </div>

          {/* Inline Navigation Buttons */}
          <div className="flex items-center gap-1.5">
            {onPrevDate && (
              <button
                onClick={onPrevDate}
                disabled={!canPrevDate}
                className="w-8 h-8 rounded-full bg-black/40 border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 disabled:opacity-20 disabled:pointer-events-none transition-all cursor-pointer shadow-sm"
                aria-label="Previous Day"
              >
                ‹
              </button>
            )}
            {onNextDate && (
              <button
                onClick={onNextDate}
                disabled={!canNextDate}
                className="w-8 h-8 rounded-full bg-black/40 border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 disabled:opacity-20 disabled:pointer-events-none transition-all cursor-pointer shadow-sm"
                aria-label="Next Day"
              >
                ›
              </button>
            )}
          </div>
        </div>

        {sessions.map((session) => {
          const isSelected = selectedSession?.id === session.id;
          const isConfirmingDelete = confirmDeleteId === session.id;
          const previewText = session.first_message || HISTORY_COPY.noTranscript;

          return (
            <div
              key={session.id}
              onClick={(e) => {
                e.stopPropagation();
                onSelect(session);
              }}
              className={cn(
                "w-full rounded-2xl p-4 flex flex-col text-left transition-all duration-200 select-none cursor-pointer relative group glass-card",
                "border-[rgba(var(--border),0.15)] bg-[rgb(var(--card))]/80 hover:border-[rgba(var(--accent),0.55)] hover:bg-[rgb(var(--card))]/95 hover:shadow-[0_0_20px_rgba(var(--accent),0.15)]",
                isSelected && "border-[rgb(var(--accent))] bg-[rgb(var(--card))] shadow-[0_0_25px_rgba(var(--accent),0.35)]"
              )}
            >
              {/* Header row: glowing dot + time + turns */}
              <div className="flex items-center justify-between mb-2 pr-8">
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_6px_rgb(var(--accent))]" />
                  <span className="text-[12px] font-mono text-[rgb(var(--foreground))] font-bold">
                    {formatDateTime(session.started_at)}
                  </span>
                </div>
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] font-medium">
                  {session.turn_count}{" "}
                  {session.turn_count === 1
                    ? HISTORY_COPY.turnSingular
                    : HISTORY_COPY.turnPlural}
                </span>
              </div>

              {/* Message quote */}
              <p className="text-[13px] font-medium leading-relaxed text-[rgb(var(--foreground))] pr-8 line-clamp-2">
                "{previewText}"
              </p>

              {/* Action */}
              <div className="absolute top-3.5 right-3.5 z-20">
                {isConfirmingDelete ? (
                  <div
                    className="flex items-center gap-1"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      onClick={(e) => onDelete(e, session.id)}
                      className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 cursor-pointer shadow-md"
                      aria-label={HISTORY_COPY.deleteConfirm}
                    >
                      <Check size={14} strokeWidth={2.5} />
                    </button>
                    <button
                      onClick={onCancelDelete}
                      className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shadow-md"
                      aria-label={HISTORY_COPY.cancelDelete}
                    >
                      <X size={14} strokeWidth={2.5} />
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={(e) => onDelete(e, session.id)}
                    className="w-7 h-7 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-colors cursor-pointer"
                    aria-label={HISTORY_COPY.deleteSession}
                  >
                    <Trash2 size={13} />
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    );
  }
);

HistoryListView.displayName = "HistoryListView";
