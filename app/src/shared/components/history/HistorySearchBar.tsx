import { useState, useMemo, useEffect, useCallback, memo } from "react";
import { MessageSquare } from "lucide-react";
import {
  resolveSessionTitle,
  getTurns,
  getCachedTurns,
  type SessionRow,
} from "@/services/persistenceService";
import { cn } from "@/shared/lib/utils";
import { SearchBarShell, HighlightMatch } from "@/shared/ui";
import { HISTORY_COPY } from "@/data/historyCopy";

export interface HistorySearchBarProps {
  sessions: SessionRow[];
  onSelectSession: (session: SessionRow) => void;
  className?: string;
  dropdownPlacement?: "bottom" | "top";
}

export interface HistorySearchHit {
  session: SessionRow;
  matchType: "title" | "turn" | "first_message";
  matchedSnippet?: string;
  matchedRole?: "user" | "assistant";
  turnIndex?: number;
}

function formatDateSub(timestamp: number): string {
  try {
    return new Date(timestamp).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

/** Extracts a readable snippet with context around the query match. */
function extractSnippet(text: string, query: string, maxLen = 90): string {
  const lowerText = text.toLowerCase();
  const lowerQ = query.toLowerCase();
  const matchIdx = lowerText.indexOf(lowerQ);
  if (matchIdx === -1) {
    return text.length > maxLen ? text.slice(0, maxLen).trim() + "…" : text;
  }

  const start = Math.max(0, matchIdx - 28);
  const end = Math.min(text.length, matchIdx + query.length + 54);
  let snippet = text.slice(start, end).replace(/[\r\n]+/g, " ").trim();

  if (start > 0) snippet = "…" + snippet;
  if (end < text.length) snippet = snippet + "…";
  return snippet;
}

export const HistorySearchBar = memo<HistorySearchBarProps>(({
  sessions,
  onSelectSession,
  className,
  dropdownPlacement = "bottom",
}) => {
  const [value, setValue] = useState("");
  const [activeIndex, setActiveIndex] = useState(-1);

  // Background turn indexing queue for scalable search across all session messages
  useEffect(() => {
    let canceled = false;
    const loadQueue = async () => {
      const unindexed = sessions.filter((s) => !getCachedTurns(s.id));
      const BATCH_SIZE = 4;
      for (let i = 0; i < unindexed.length; i += BATCH_SIZE) {
        if (canceled) break;
        const batch = unindexed.slice(i, i + BATCH_SIZE);
        await Promise.all(batch.map((s) => getTurns(s.id).catch(() => [])));
      }
    };

    const idleId = window.requestIdleCallback
      ? window.requestIdleCallback(() => void loadQueue())
      : setTimeout(() => void loadQueue(), 150);

    return () => {
      canceled = true;
      if (typeof idleId === "number") {
        if (window.cancelIdleCallback) {
          window.cancelIdleCallback(idleId);
        } else {
          clearTimeout(idleId);
        }
      }
    };
  }, [sessions]);

  // Scalable search across titles, first message, and all turns
  const results = useMemo<HistorySearchHit[]>(() => {
    const q = value.trim().toLowerCase();
    if (!q) return [];

    const hits: HistorySearchHit[] = [];
    const MAX_RESULTS = 7;

    for (const session of sessions) {
      if (hits.length >= MAX_RESULTS) break;

      const title = resolveSessionTitle(session);
      const lowerTitle = title.toLowerCase();

      // 1. Direct title match
      if (lowerTitle.includes(q)) {
        hits.push({
          session,
          matchType: "title",
        });
        continue;
      }

      // 2. Check full turns in memory cache
      const turns = getCachedTurns(session.id);
      let foundTurn = false;

      if (turns && turns.length > 0) {
        for (const turn of turns) {
          if (turn.user_text && turn.user_text.toLowerCase().includes(q)) {
            hits.push({
              session,
              matchType: "turn",
              matchedSnippet: extractSnippet(turn.user_text, q),
              matchedRole: "user",
              turnIndex: turn.turn_id,
            });
            foundTurn = true;
            break;
          }
          if (turn.assistant_text && turn.assistant_text.toLowerCase().includes(q)) {
            hits.push({
              session,
              matchType: "turn",
              matchedSnippet: extractSnippet(turn.assistant_text, q),
              matchedRole: "assistant",
              turnIndex: turn.turn_id,
            });
            foundTurn = true;
            break;
          }
        }
      }

      // 3. Fallback to session.first_message if turns not yet cached
      if (!foundTurn && session.first_message && session.first_message.toLowerCase().includes(q)) {
        hits.push({
          session,
          matchType: "first_message",
          matchedSnippet: extractSnippet(session.first_message, q),
          matchedRole: "user",
        });
      }
    }

    return hits;
  }, [sessions, value]);

  const handleClear = useCallback(() => {
    setValue("");
    setActiveIndex(-1);
  }, []);

  const handleSelect = useCallback(
    (hit: HistorySearchHit) => {
      onSelectSession(hit.session);
      setValue("");
      setActiveIndex(-1);
    },
    [onSelectSession]
  );

  return (
    <SearchBarShell
      id="history-search-input"
      value={value}
      onChange={(val) => {
        setValue(val);
        setActiveIndex(-1);
      }}
      onClear={handleClear}
      placeholder={HISTORY_COPY.searchPlaceholder}
      ariaLabel={HISTORY_COPY.searchPlaceholder}
      clearAriaLabel={HISTORY_COPY.clearSearch}
      className={className}
      dropdownPlacement={dropdownPlacement}
      itemCount={results.length}
      activeIndex={activeIndex}
      onActiveIndexChange={setActiveIndex}
      onSelectActive={(idx) => {
        if (results[idx]) {
          handleSelect(results[idx]);
        }
      }}
      dropdownHeader={
        <div className="px-2.5 py-1.5 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] border-b border-[rgba(var(--border),0.08)] flex items-center justify-between">
          <span>{HISTORY_COPY.allSessionsLabel}</span>
          <span className="opacity-70 font-bold">{results.length}</span>
        </div>
      }
      dropdownContent={
        results.length === 0 ? (
          <div className="px-4 py-4 text-center text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]">
            {HISTORY_COPY.noMatchingSessions}
          </div>
        ) : (
          <div className="py-1 max-h-[320px] overflow-y-auto custom-scrollbar flex flex-col gap-0.5">
            {results.map((hit, idx) => {
              const isActive = idx === activeIndex;
              const title = resolveSessionTitle(hit.session);
              const dateLabel = formatDateSub(hit.session.created_at);

              return (
                <button
                  key={`${hit.session.id}-${idx}`}
                  type="button"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    handleSelect(hit);
                  }}
                  className={cn(
                    "w-full text-left p-2.5 rounded-xl transition-colors cursor-pointer flex flex-col",
                    isActive
                      ? "bg-[rgba(var(--accent),0.10)]"
                      : "hover:bg-[rgba(var(--foreground),0.04)]"
                  )}
                >
                  <div className="flex items-center justify-between gap-2 min-w-0">
                    <div className="flex items-center gap-2 min-w-0">
                      <MessageSquare
                        size={13}
                        className="text-[rgb(var(--accent))] shrink-0 opacity-70"
                      />
                      <span className="text-[13px] font-medium text-[rgb(var(--foreground))] truncate">
                        <HighlightMatch text={title} query={value} />
                      </span>
                    </div>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 shrink-0">
                      {dateLabel}
                    </span>
                  </div>

                  {hit.matchedSnippet && (
                    <p className="text-[11.5px] font-sans text-[rgb(var(--foreground-muted))] line-clamp-2 leading-relaxed mt-1 pl-5 m-0">
                      <HighlightMatch text={hit.matchedSnippet} query={value} />
                    </p>
                  )}
                </button>
              );
            })}
          </div>
        )
      }
    />
  );
});

HistorySearchBar.displayName = "HistorySearchBar";
