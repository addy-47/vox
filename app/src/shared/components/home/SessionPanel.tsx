import { memo, useCallback, useMemo, useState } from "react";
import { Plus, Pin, MessageSquare, FolderClosed, Loader2, AlertCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSessionPanel } from "@/shared/hooks/useSessionPanel";
import { useVoiceSession } from "@/shared/context/VoiceSessionContext";
import {
  resolveSessionTitle,
  sessionLastActivity,
  formatSessionRecency,
  type SessionRow,
} from "@/services/historyService";
import { SESSION_COPY } from "@/data/sessionCopy";

interface SessionPanelProps {
  onClose: () => void;
}

const SessionRowItem = memo(
  ({
    session,
    active,
    restoring,
    pinned,
    onSelect,
    onTogglePin,
  }: {
    session: SessionRow;
    active: boolean;
    restoring: boolean;
    pinned: boolean;
    onSelect: (id: number) => void;
    onTogglePin: (id: number) => void;
  }) => {
    const handleSelect = useCallback(() => {
      onSelect(session.id);
    }, [onSelect, session.id]);

    const handlePin = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onTogglePin(session.id);
      },
      [onTogglePin, session.id]
    );

    const title = resolveSessionTitle(session);
    const recency = useMemo(() => formatSessionRecency(sessionLastActivity(session)), [session]);
    const turnLabel = useMemo(() => {
      const unit = session.turn_count === 1 ? SESSION_COPY.turnSingular : SESSION_COPY.turnPlural;
      return `${session.turn_count} ${unit}`;
    }, [session.turn_count]);

    return (
      <div
        className={cn(
          "w-full text-left px-3 py-2.5 rounded-xl border transition-all duration-300",
          "hover:bg-[rgb(var(--accent))]/10",
          active ? "bg-[rgb(var(--accent))]/15 border-[rgb(var(--accent))]/40" : "bg-transparent border-[rgba(var(--border),0.12)]",
          restoring && "opacity-60"
        )}
      >
        <div className="flex items-center gap-2 min-w-0">
          <button
            onClick={handleSelect}
            disabled={restoring}
            aria-current={active ? "true" : undefined}
            className="flex-1 min-w-0 flex items-center gap-2 text-left cursor-pointer disabled:cursor-wait"
          >
            {restoring ? (
              <Loader2 size={14} className="shrink-0 animate-spin text-[rgb(var(--accent))]" aria-label={SESSION_COPY.restoringAriaLabel} />
            ) : (
              <MessageSquare size={14} className="shrink-0 text-[rgb(var(--foreground-muted))]" />
            )}
            <span className="flex-1 min-w-0 truncate text-[13px] font-medium text-[rgb(var(--foreground))]">{title}</span>
          </button>
          <button
            onClick={handlePin}
            aria-pressed={pinned}
            aria-label={pinned ? SESSION_COPY.unpinAriaLabel : SESSION_COPY.pinAriaLabel}
            className={cn(
              "shrink-0 flex items-center justify-center w-7 h-7 rounded-lg transition-colors cursor-pointer",
              pinned
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)]"
            )}
          >
            <Pin size={13} fill={pinned ? "currentColor" : "none"} />
          </button>
        </div>
        <span className="mt-1 block pl-6 text-[11px] tracking-wide text-[rgb(var(--foreground-muted))]">
          {recency} · {turnLabel}
        </span>
      </div>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";

/**
 * Conversation list rendered inside EdgePanel (side="left", home only).
 * Data comes from useSessionPanel; selection/creation close the panel.
 */
export const SessionPanel = memo(({ onClose }: SessionPanelProps) => {
  const {
    pinnedSessions,
    projects,
    uncategorizedSessions,
    loading,
    error,
    refresh,
    createNewSession,
    createNewProject,
    togglePin,
    selectSession,
  } = useSessionPanel();
  const { activeSessionId, isRestoring, restoringSessionId } = useVoiceSession();

  const [projectName, setProjectName] = useState("");

  const handleSelect = useCallback(
    (id: number) => {
      selectSession(id)
        .then(() => onClose())
        .catch(() => {});
    },
    [selectSession, onClose]
  );

  const handleNew = useCallback(() => {
    createNewSession()
      .then(() => onClose())
      .catch(() => {});
  }, [createNewSession, onClose]);

  const handleRetry = useCallback(() => {
    refresh().catch(() => {});
  }, [refresh]);

  const handleCreateProject = useCallback(() => {
    const name = projectName.trim();
    if (!name) return;
    createNewProject(name)
      .then(() => {
        setProjectName("");
        return refresh();
      })
      .catch(() => {});
  }, [projectName, createNewProject, refresh]);

  const renderRow = useCallback(
    (session: SessionRow, pinned: boolean) => (
      <SessionRowItem
        key={session.id}
        session={session}
        active={session.id === activeSessionId}
        restoring={isRestoring && restoringSessionId === session.id}
        pinned={pinned}
        onSelect={handleSelect}
        onTogglePin={togglePin}
      />
    ),
    [activeSessionId, isRestoring, restoringSessionId, handleSelect, togglePin]
  );

  const isEmpty = pinnedSessions.length === 0 && projects.length === 0 && uncategorizedSessions.length === 0;

  return (
    <div className="flex flex-col gap-3 p-3">
      <button
        onClick={handleNew}
        aria-label={SESSION_COPY.newConversationAriaLabel}
        className="w-full flex items-center justify-center gap-2 px-3 h-10 rounded-xl border border-[rgb(var(--accent))]/40 bg-[rgb(var(--accent))]/10 hover:bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] text-[12px] font-bold tracking-wider uppercase transition-all cursor-pointer"
      >
        <Plus size={15} />
        {SESSION_COPY.newConversation}
      </button>

      <div className="flex items-center gap-2">
        <input
          value={projectName}
          onChange={(e) => setProjectName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleCreateProject();
          }}
          placeholder={SESSION_COPY.newProjectPlaceholder}
          aria-label={SESSION_COPY.createProjectAriaLabel}
          className="flex-1 min-w-0 h-9 px-3 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none focus:border-[rgba(var(--accent),0.4)]"
        />
        <button
          onClick={handleCreateProject}
          disabled={!projectName.trim()}
          aria-label={SESSION_COPY.createProjectAriaLabel}
          className="shrink-0 flex items-center justify-center w-9 h-9 rounded-xl border border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <Plus size={15} />
        </button>
      </div>

      {loading ? (
        <p className="px-3 py-6 text-center text-[12px] text-[rgb(var(--foreground-muted))]">{SESSION_COPY.loadingSessions}</p>
      ) : error ? (
        <div className="flex flex-col items-center gap-2 px-3 py-6 text-center">
          <AlertCircle size={18} className="text-red-400" />
          <p className="text-[12px] text-[rgb(var(--foreground))]/80">{error}</p>
          <button
            onClick={handleRetry}
            className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:underline cursor-pointer"
          >
            {SESSION_COPY.retry}
          </button>
        </div>
      ) : isEmpty ? (
        <div className="flex flex-col items-center gap-1.5 px-3 py-6 text-center">
          <p className="text-[13px] font-medium text-[rgb(var(--foreground))]">{SESSION_COPY.noSessionsTitle}</p>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]">{SESSION_COPY.noSessionsDesc}</p>
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {pinnedSessions.length > 0 && (
            <section className="flex flex-col gap-1.5">
              <h3 className="px-3 text-[10.5px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--accent))]/80">
                {SESSION_COPY.pinnedSection}
              </h3>
              {pinnedSessions.map((s) => renderRow(s, true))}
            </section>
          )}
          {projects.length > 0 && (
            <section className="flex flex-col gap-2.5">
              <h3 className="px-3 text-[10.5px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--accent))]/80">
                {SESSION_COPY.projectsSection}
              </h3>
              {projects.map((group) => (
                <div key={group.project.id} className="flex flex-col gap-1.5">
                  <div className="flex items-center gap-1.5 px-3 text-[rgb(var(--foreground-muted))]">
                    <FolderClosed size={13} className="shrink-0" />
                    <span className="flex-1 min-w-0 truncate text-[11px] font-bold tracking-[0.14em] uppercase">
                      {group.project.name}
                    </span>
                  </div>
                  {group.sessions.map((s) => renderRow(s, false))}
                </div>
              ))}
            </section>
          )}
          {uncategorizedSessions.length > 0 && (
            <section className="flex flex-col gap-1.5">
              <h3 className="px-3 text-[10.5px] font-mono font-bold uppercase tracking-[0.18em] text-[rgb(var(--accent))]/80">
                {SESSION_COPY.uncategorizedSection}
              </h3>
              {uncategorizedSessions.map((s) => renderRow(s, false))}
            </section>
          )}
        </div>
      )}
    </div>
  );
});
SessionPanel.displayName = "SessionPanel";
