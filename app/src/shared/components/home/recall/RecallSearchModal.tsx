import { memo, useState, useRef, useMemo } from "react";
import { Search, Filter, Binoculars, Check, X, Ghost } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Modal } from "@/shared/ui/Modal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { EmptyState } from "@/shared/components/common";
import {
  resolveSessionTitle,
  sessionLastActivity,
  formatSessionRecency,
  type SessionRow,
} from "@/services/persistenceService";
import { type SortOrder } from "@/store/recallSearchStore";
import { SESSION_COPY } from "@/data/sessionCopy";
import { HISTORY_COPY } from "@/data/historyCopy";
import { CompactSessionButton } from "./CompactSessionButton";
import { useOverlay } from "@/shared/hooks/useOverlay";

export interface RecallSearchModalProps {
  open: boolean;
  onClose: () => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  sortOrder: SortOrder;
  setSortOrder: (order: SortOrder) => void;
  sessions: SessionRow[];
  totalCount: number;
  onSelectSession: (session: SessionRow) => void;
  projectNameMap?: Map<string, string>;
}

const SORT_OPTIONS: Array<{ key: SortOrder; label: string }> = [
  { key: "newest", label: SESSION_COPY.sortNewest },
  { key: "oldest", label: SESSION_COPY.sortOldest },
  { key: "turns", label: SESSION_COPY.sortTurns },
  { key: "uncompacted", label: SESSION_COPY.sortUncompacted },
];

/**
 * Mobile and compact-layout search modal for conversation recall.
 * Provides a clean modal with an embedded filter icon directly in the search bar,
 * real-time query filtering, and smooth session transcript drawer invocation.
 */
export const RecallSearchModal = memo(({
  open,
  onClose,
  searchQuery,
  setSearchQuery,
  sortOrder,
  setSortOrder,
  sessions,
  totalCount,
  onSelectSession,
  projectNameMap,
}: RecallSearchModalProps) => {
  const [filterMenuOpen, setFilterMenuOpen] = useState(false);
  const filterMenuRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useOverlay({
    active: filterMenuOpen,
    onClose: () => setFilterMenuOpen(false),
    ref: filterMenuRef,
    dismissOnOutside: true,
  });

  const activeSortLabel = useMemo(() => {
    return SORT_OPTIONS.find((o) => o.key === sortOrder)?.label ?? SESSION_COPY.sortNewest;
  }, [sortOrder]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      position="global"
      title={
        <div className="flex items-center gap-2">
          <Binoculars size={16} className="text-[rgb(var(--accent))] shrink-0" />
          <span className="font-display text-[14px] font-bold tracking-tight text-[rgb(var(--foreground))]">
            {SESSION_COPY.searchConversations}
          </span>
        </div>
      }
      ariaLabel={SESSION_COPY.searchConversations}
      className="w-full sm:w-[min(540px,92vw)] h-[min(560px,calc(100dvh-96px))] rounded-2xl bg-[rgba(var(--card),0.96)] border border-[rgba(var(--border),0.15)] shadow-2xl backdrop-blur-xl"
      bodyClassName="h-full flex flex-col min-h-0 overflow-hidden px-4 sm:px-6 py-3"
      footer={
        <div className="px-4 sm:px-6 py-2.5 border-t border-[rgba(var(--border),0.08)] bg-[rgba(var(--foreground),0.02)] flex items-center justify-between shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="flex items-center gap-1.5 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer select-none group"
          >
            <span>← {SESSION_COPY.backToVoiceMode}</span>
            <kbd className="px-1.5 py-0.5 rounded border border-[rgba(var(--foreground),0.12)] bg-[rgba(var(--foreground),0.04)] text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 group-hover:text-[rgb(var(--foreground))] transition-colors">
              Esc
            </kbd>
          </button>
        </div>
      }
    >
      <div className="flex flex-col h-full min-h-0 gap-2.5">
        {/* ── Underline Search Input with Embedded Filter Icon ── */}
        <div className="relative w-full shrink-0">
          <div className="flex items-center gap-2 py-2 border-b border-[rgba(var(--foreground),0.15)] focus-within:border-[rgb(var(--accent))] transition-colors duration-150 w-full shrink-0">
            <Search size={14} className="text-[rgb(var(--accent))] shrink-0 opacity-80" />
            <input
              ref={inputRef}
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={SESSION_COPY.searchConversationsPlaceholder}
              autoFocus
              className="flex-1 min-w-0 bg-transparent text-[12.5px] font-sans text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
            />

            {/* Filter icon embedded inside the search bar */}
            <div className="relative" ref={filterMenuRef}>
              <Tooltip label="Filter & sort conversations" side="bottom">
                <button
                  type="button"
                  onClick={() => setFilterMenuOpen((prev) => !prev)}
                  className={cn(
                    "relative p-1 rounded-lg transition-colors cursor-pointer shrink-0",
                    sortOrder !== "newest" || filterMenuOpen
                      ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.12)] hover:bg-[rgba(var(--accent),0.2)]"
                      : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)]"
                  )}
                  aria-label="Filter & sort conversations"
                  aria-expanded={filterMenuOpen}
                >
                  <Filter size={14} />
                  {sortOrder !== "newest" && (
                    <span className="absolute top-0.5 right-0.5 w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                  )}
                </button>
              </Tooltip>

              {/* Filter / Sort popover menu */}
              {filterMenuOpen && (
                <div
                  role="menu"
                  className="absolute right-0 top-[calc(100%+8px)] z-50 w-44 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgb(var(--card))] shadow-xl p-1 flex flex-col gap-0.5 backdrop-blur-xl animate-in fade-in zoom-in-95 duration-100"
                >
                  <div className="px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 border-b border-[rgba(var(--border),0.08)]">
                    Sort by
                  </div>
                  {SORT_OPTIONS.map(({ key, label }) => {
                    const isSelected = sortOrder === key;
                    return (
                      <button
                        key={key}
                        type="button"
                        role="menuitem"
                        onClick={() => {
                          setSortOrder(key);
                          setFilterMenuOpen(false);
                        }}
                        className={cn(
                          "flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left text-[12px] font-sans transition-colors cursor-pointer",
                          isSelected
                            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium"
                            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)]"
                        )}
                      >
                        <span>{label}</span>
                        {isSelected && <Check size={12} className="shrink-0" />}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Clear search button or kbd shortcut */}
            {searchQuery ? (
              <button
                type="button"
                onClick={() => setSearchQuery("")}
                className="p-1 rounded-full text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer shrink-0 transition-colors"
                aria-label="Clear search"
              >
                <X size={12} />
              </button>
            ) : (
              <kbd className="shrink-0 px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] border border-[rgba(var(--border),0.10)] text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 leading-none select-none">
                /
              </kbd>
            )}
          </div>
        </div>

        {/* ── Metadata & Active Filter Indicator ── */}
        <div className="flex items-center justify-between px-1 text-[11px] font-mono text-[rgb(var(--foreground-muted))] shrink-0">
          <span>
            {sessions.length} {sessions.length === 1 ? "conversation" : "conversations"}
            {totalCount > sessions.length && ` (of ${totalCount})`}
          </span>
          {sortOrder !== "newest" && (
            <button
              type="button"
              onClick={() => setSortOrder("newest")}
              className="flex items-center gap-1 text-[10.5px] text-[rgb(var(--accent))] hover:underline cursor-pointer"
            >
              <span>{activeSortLabel}</span>
              <X size={10} />
            </button>
          )}
        </div>

        {/* ── Session List Container ── */}
        <div className="flex-1 overflow-y-auto min-h-0 custom-scrollbar flex flex-col gap-1.5 pr-0.5">
          {sessions.length === 0 ? (
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center my-auto min-h-0">
              <EmptyState
                icon={Ghost}
                title={SESSION_COPY.noMatchingConversations}
                description={HISTORY_COPY.noMatchingSessions}
                className="max-w-sm border-0 bg-transparent p-0 min-h-0"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => setSearchQuery("")}
                  className="mt-3 px-3.5 py-1 rounded-full text-[11px] font-mono bg-[rgba(var(--accent),0.08)] hover:bg-[rgba(var(--accent),0.16)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] transition-colors cursor-pointer"
                >
                  {HISTORY_COPY.clearSearch}
                </button>
              )}
            </div>
          ) : (
            sessions.map((session) => {
              const title = resolveSessionTitle(session);
              const projectName = session.project_id && projectNameMap ? projectNameMap.get(session.project_id) : undefined;
              const recency = formatSessionRecency(sessionLastActivity(session));

              return (
                <div
                  key={session.id}
                  onClick={() => onSelectSession(session)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelectSession(session);
                    }
                  }}
                  className="group flex flex-col gap-1.5 p-3 rounded-xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--foreground),0.025)] hover:bg-[rgba(var(--foreground),0.055)] hover:border-[rgba(var(--accent),0.35)] transition-all duration-150 cursor-pointer select-none text-left"
                >
                  <div className="flex items-center justify-between gap-2 min-w-0">
                    <h4 className="text-[13px] font-sans font-medium text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] transition-colors truncate">
                      {title}
                    </h4>
                    <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 shrink-0">
                      {recency}
                    </span>
                  </div>

                  <div className="flex items-center gap-2 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70">
                    <span>
                      {session.turn_count} {session.turn_count === 1 ? SESSION_COPY.turnSingular : SESSION_COPY.turnPlural}
                    </span>
                    {projectName && (
                      <>
                        <span className="opacity-40">•</span>
                        <span className="px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)] text-[10px] truncate max-w-[140px]">
                          {projectName}
                        </span>
                      </>
                    )}
                    {session.uncompacted_turns > 0 && (
                      <div className="ml-auto" onClick={(e) => e.stopPropagation()}>
                        <CompactSessionButton
                          sessionId={session.id}
                          uncompactedTurns={session.uncompacted_turns}
                          variant="chip"
                        />
                      </div>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </Modal>
  );
});

RecallSearchModal.displayName = "RecallSearchModal";
