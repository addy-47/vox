import React, { memo, useState, useMemo } from "react";
import { Check, X, Trash2, ChevronLeft, ChevronRight, Search, MessageSquare, Filter } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { formatDateTime, resolveSessionTitle, type SessionRow } from "@/services/historyService";
import { HISTORY_COPY } from "@/data/historyCopy";
import { useHistoryFilterStore } from "@/store/historyFilterStore";
import { CalendarPicker, toDateKey } from "./CalendarPicker";

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

// ─── Fuzzy Match ────────────────────────────────────────────────────────────

/**
 * Returns true if `needle` fuzzy-matches `haystack`.
 * Every character of needle must appear in order inside haystack.
 */
function fuzzyMatch(needle: string, haystack: string): boolean {
  if (!needle) return true;
  let ni = 0;
  for (let hi = 0; hi < haystack.length && ni < needle.length; hi++) {
    if (haystack[hi] === needle[ni]) ni++;
  }
  return ni === needle.length;
}

/**
 * Returns the haystack string split into segments: [text, isHighlighted].
 * Greedily marks characters matched by needle.
 */
function fuzzyHighlight(needle: string, haystack: string): { text: string; highlight: boolean }[] {
  if (!needle) return [{ text: haystack, highlight: false }];

  const result: { text: string; highlight: boolean }[] = [];
  let ni = 0;

  for (let hi = 0; hi < haystack.length; hi++) {
    const char = haystack[hi];
    if (ni < needle.length && char.toLowerCase() === needle[ni].toLowerCase()) {
      result.push({ text: char, highlight: true });
      ni++;
    } else {
      result.push({ text: char, highlight: false });
    }
  }

  // Collapse consecutive highlights / non-highlights
  const collapsed: { text: string; highlight: boolean }[] = [];
  for (const seg of result) {
    const last = collapsed[collapsed.length - 1];
    if (last && last.highlight === seg.highlight) {
      last.text += seg.text;
    } else {
      collapsed.push({ ...seg });
    }
  }

  return collapsed;
}

interface HighlightedTextProps {
  text: string;
  query: string;
  className?: string;
}

const HighlightedText: React.FC<HighlightedTextProps> = memo(({ text, query, className }) => {
  const segments = useMemo(() => fuzzyHighlight(query, text), [query, text]);

  if (!query) {
    return <span className={className}>{text}</span>;
  }

  return (
    <span className={className}>
      {segments.map((seg, i) =>
        seg.highlight ? (
          <span
            key={i}
            className="text-[rgb(var(--accent))] font-bold underline decoration-[rgb(var(--accent))]/40"
          >
            {seg.text}
          </span>
        ) : (
          <span key={i}>{seg.text}</span>
        )
      )}
    </span>
  );
});
HighlightedText.displayName = "HighlightedText";

// ─── HistoryListView ────────────────────────────────────────────────────────

export const HistoryListView: React.FC<HistoryListViewProps> = memo(
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
  }) => {
    const [searchQuery, setSearchQuery] = useState("");
    const [showDateRangePicker, setShowDateRangePicker] = useState(false);
    const [customStartDate, setCustomStartDate] = useState("");
    const [customEndDate, setCustomEndDate] = useState("");
    const dateFilter = useHistoryFilterStore((s) => s.dateFilter);
    const setDateFilter = useHistoryFilterStore((s) => s.setDateFilter);

    const q = searchQuery.trim().toLowerCase();

    const sessionDates = useMemo(() => {
      return new Set(sessions.map((s) => toDateKey(new Date(s.created_at))));
    }, [sessions]);

    const initialCalendarDate = useMemo(() => {
      if (sessions.length > 0) {
        return new Date(sessions[0].created_at);
      }
      return new Date();
    }, [sessions]);

    const filteredSessions = useMemo(() => {
      const now = Date.now();

      return sessions.filter((s) => {
        // 1. Date Filter
        if (dateFilter === "today") {
          const sessionDate = new Date(s.created_at);
          const today = new Date();
          if (sessionDate.toDateString() !== today.toDateString()) return false;
        } else if (dateFilter === "week") {
          const sevenDaysMs = 7 * 24 * 60 * 60 * 1000;
          if (now - s.created_at > sevenDaysMs) return false;
        } else if (dateFilter === "month") {
          const thirtyDaysMs = 30 * 24 * 60 * 60 * 1000;
          if (now - s.created_at > thirtyDaysMs) return false;
        } else if (dateFilter === "custom") {
          const sessionDateKey = toDateKey(new Date(s.created_at));
          if (customStartDate && !customEndDate) {
            if (sessionDateKey !== customStartDate) return false;
          } else {
            if (customStartDate && sessionDateKey < customStartDate) return false;
            if (customEndDate && sessionDateKey > customEndDate) return false;
          }
        }

        // 2. Fuzzy search on session title
        if (q) {
          const title = resolveSessionTitle(s).toLowerCase();
          return fuzzyMatch(q, title);
        }

        return true;
      });
    }, [sessions, q, dateFilter, customStartDate, customEndDate]);

    return (
      <div className="w-full flex-1 overflow-y-auto px-4 pt-4 pb-28 space-y-3 custom-scrollbar z-20">
        {/* Standardized Header with Title, Day Context, and Carousel Navigation Chevrons */}
        <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--accent),0.12)] mb-3 px-1 shrink-0">
          <div className="flex flex-col">
            <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {HISTORY_COPY.historyAndSessions}
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
                className="w-8 h-8 rounded-xl glass-card border border-[rgba(var(--accent),0.15)] flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 disabled:opacity-20 disabled:pointer-events-none transition-all cursor-pointer shadow-sm"
                aria-label={HISTORY_COPY.prevDay}
              >
                <ChevronLeft size={16} />
              </button>
            )}
            {onNextDate && (
              <button
                onClick={onNextDate}
                disabled={!canNextDate}
                className="w-8 h-8 rounded-xl glass-card border border-[rgba(var(--accent),0.15)] flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 disabled:opacity-20 disabled:pointer-events-none transition-all cursor-pointer shadow-sm"
                aria-label={HISTORY_COPY.nextDay}
              >
                <ChevronRight size={16} />
              </button>
            )}
          </div>
        </div>

        {/* Quick Search Input — Underline Style matching Design System */}
        <div className="px-1 pb-1">
          <div className="flex items-center gap-2 py-1.5 border-b border-[rgba(var(--foreground),0.15)] focus-within:border-[rgba(var(--accent),0.7)] transition-colors">
            <Search size={13} className="text-[rgb(var(--accent))] opacity-70 shrink-0" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={HISTORY_COPY.searchPlaceholder}
              className="w-full bg-transparent text-[11.5px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery("")}
                className="p-0.5 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer transition-colors"
                aria-label={HISTORY_COPY.clearSearch}
              >
                <X size={12} />
              </button>
            )}
          </div>
        </div>

        {/* Date Filter Pills */}
        <div className="flex items-center gap-1.5 px-1 py-1 overflow-x-auto no-scrollbar">
          {(
            [
              { id: "all", label: HISTORY_COPY.filterAll },
              { id: "today", label: HISTORY_COPY.filterToday },
              { id: "week", label: HISTORY_COPY.filterPastWeek },
              { id: "month", label: HISTORY_COPY.filterPastMonth },
            ] as const
          ).map((pill) => {
            const active = dateFilter === pill.id;
            return (
              <button
                key={pill.id}
                type="button"
                onClick={() => {
                  setDateFilter(pill.id);
                  setCustomStartDate("");
                  setCustomEndDate("");
                }}
                className={cn(
                  "px-2.5 py-1 rounded-lg text-[10.5px] font-mono whitespace-nowrap transition-all cursor-pointer border",
                  active
                    ? "bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.45)] font-bold shadow-[0_0_8px_rgba(var(--accent),0.2)]"
                    : "bg-[rgba(var(--card),0.4)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.12)] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.2)]"
                )}
              >
                {pill.label}
              </button>
            );
          })}

          {/* Filter icon button directly right of the last 30 days pill */}
          <button
            type="button"
            onClick={() => setShowDateRangePicker((prev) => !prev)}
            title={HISTORY_COPY.filterLabel}
            aria-label={HISTORY_COPY.filterLabel}
            className={cn(
              "p-1.5 rounded-lg border text-[10.5px] font-mono whitespace-nowrap transition-all cursor-pointer flex items-center justify-center shrink-0",
              showDateRangePicker || dateFilter === "custom"
                ? "bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.45)] font-bold shadow-[0_0_8px_rgba(var(--accent),0.2)]"
                : "bg-[rgba(var(--card),0.4)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.12)] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.2)]"
            )}
          >
            <Filter size={11} strokeWidth={1.8} />
          </button>
        </div>

        {/* Date Selection Filter Range Option Panel */}
        {showDateRangePicker && (
          <div className="mx-1 animate-in fade-in zoom-in-95 duration-150">
            <CalendarPicker
              startDate={customStartDate || null}
              endDate={customEndDate || null}
              sessionDates={sessionDates}
              initialDate={initialCalendarDate}
              onChange={(start, end) => {
                setCustomStartDate(start ?? "");
                setCustomEndDate(end ?? "");
                if (start || end) {
                  setDateFilter("custom");
                } else {
                  setDateFilter("all");
                }
              }}
            />
          </div>
        )}

        {filteredSessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-8 text-center">
            <MessageSquare size={24} className="text-[rgb(var(--foreground-muted))] opacity-40 mb-2" />
            <p className="text-[12px] font-mono text-[rgb(var(--foreground))] font-semibold">
              {HISTORY_COPY.noMatchingSessions}
            </p>
          </div>
        ) : (
          filteredSessions.map((session) => {
            const isSelected = selectedSession?.id === session.id;
            const isConfirmingDelete = confirmDeleteId === session.id;
            const title = resolveSessionTitle(session);

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
                      {formatDateTime(session.created_at)}
                    </span>
                  </div>
                  <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] font-medium">
                    {session.turn_count}{" "}
                    {session.turn_count === 1
                      ? HISTORY_COPY.turnSingular
                      : HISTORY_COPY.turnPlural}
                  </span>
                </div>

                {/* Session title with fuzzy highlight */}
                <p className="text-[13px] font-medium leading-relaxed pr-8 line-clamp-2">
                  <HighlightedText
                    text={title}
                    query={q}
                    className="text-[rgb(var(--foreground))]"
                  />
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
          })
        )}
      </div>
    );
  }
);

HistoryListView.displayName = "HistoryListView";
