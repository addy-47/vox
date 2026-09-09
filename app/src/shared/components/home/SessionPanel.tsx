import { memo, useCallback, useMemo, useState } from "react";
import {
  Plus,
  Pin,
  MessageSquare,
  Folder,
  FolderOpen,
  FolderPlus,
  ChevronRight,
  Loader2,
  AlertCircle,
  Check,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
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
import { Tooltip } from "@/shared/ui/Tooltip";

interface SessionPanelProps {
  onClose: () => void;
}

interface SessionRowItemProps {
  session: SessionRow;
  active: boolean;
  restoring: boolean;
  pinned: boolean;
  onSelect: (id: number) => void;
  onTogglePin: (id: number) => void;
  nested?: boolean;
}

const SessionRowItem = memo(
  ({
    session,
    active,
    restoring,
    pinned,
    onSelect,
    onTogglePin,
    nested = false,
  }: SessionRowItemProps) => {
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
    const recency = useMemo(
      () => formatSessionRecency(sessionLastActivity(session)),
      [session]
    );

    return (
      <div
        role="button"
        tabIndex={0}
        onClick={handleSelect}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") handleSelect();
        }}
        aria-current={active ? "true" : undefined}
        className={cn(
          "group flex items-center justify-between gap-2 py-1.5 rounded-lg text-left cursor-pointer transition-colors select-none",
          active
            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)]",
          restoring && "opacity-60 cursor-wait",
          nested ? "pl-7 pr-2.5" : "px-2.5"
        )}
      >
        <div className="flex items-center gap-2 min-w-0 flex-1">
          {restoring ? (
            <Loader2
              size={13}
              className="shrink-0 animate-spin text-[rgb(var(--accent))]"
              aria-label={SESSION_COPY.restoringAriaLabel}
            />
          ) : (
            <MessageSquare
              size={13}
              className={cn(
                "shrink-0 transition-colors",
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground-muted))]"
              )}
            />
          )}
          <span className="truncate text-[12.5px] tracking-wide flex-1">
            {title}
          </span>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40 opacity-0 group-hover:opacity-100 transition-opacity">
            {recency}
          </span>
          <button
            type="button"
            onClick={handlePin}
            aria-pressed={pinned}
            aria-label={pinned ? SESSION_COPY.unpinAriaLabel : SESSION_COPY.pinAriaLabel}
            className={cn(
              "flex items-center justify-center w-5 h-5 rounded hover:bg-[rgba(var(--foreground),0.08)] transition-all cursor-pointer",
              pinned
                ? "text-[rgb(var(--accent))] opacity-100"
                : "opacity-0 group-hover:opacity-60 hover:!opacity-100 text-[rgb(var(--foreground-muted))]"
            )}
          >
            <Pin size={12} fill={pinned ? "currentColor" : "none"} />
          </button>
        </div>
      </div>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";

/**
 * Conversation list rendered inside EdgePanel (side="left").
 * Clean IDE-like design: + New Session bar, Pinned list, Projects with FolderPlus and Accordions.
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

  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});

  const isProjectExpanded = useCallback(
    (id: string) => expandedProjects[id] !== false,
    [expandedProjects]
  );

  const toggleProject = useCallback((id: string) => {
    setExpandedProjects((prev) => ({
      ...prev,
      [id]: prev[id] === false,
    }));
  }, []);

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
        setIsCreatingProject(false);
        return refresh();
      })
      .catch(() => {});
  }, [projectName, createNewProject, refresh]);

  return (
    <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-3 p-3 pb-16 select-none">
      {/* ── Top Action: + New Session ── */}
      <button
        type="button"
        onClick={handleNew}
        aria-label={SESSION_COPY.newConversationAriaLabel}
        className="w-full flex items-center justify-between px-3 py-2 rounded-lg bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--accent),0.1)] text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))] text-[12.5px] font-medium tracking-wide transition-all cursor-pointer shrink-0"
      >
        <div className="flex items-center gap-2">
          <Plus size={14} strokeWidth={2.2} className="text-[rgb(var(--accent))]" />
          <span>{SESSION_COPY.newConversation}</span>
        </div>
        {loading && (
          <Loader2 size={13} className="animate-spin text-[rgb(var(--foreground-muted))]/60" />
        )}
      </button>

      {/* ── Inline Transient Error Banner (if any) ── */}
      {error && (
        <div className="flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg bg-red-500/10 border border-red-500/20 text-[11px] text-red-400 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <AlertCircle size={13} className="shrink-0 text-red-400" />
            <span className="truncate">{error}</span>
          </div>
          <button
            type="button"
            onClick={handleRetry}
            className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:underline shrink-0 cursor-pointer"
          >
            {SESSION_COPY.retry}
          </button>
        </div>
      )}

      {/* ── Panel Content Sections (Always Visible Structure) ── */}
      <div className="flex flex-col gap-3.5 flex-1 min-h-0">
        {/* ── Pinned Sessions ── */}
        <section className="flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 px-2 py-0.5">
            <Pin size={11} className="text-[rgb(var(--accent))] shrink-0" />
            <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {SESSION_COPY.pinnedSection}
            </span>
          </div>
          <div className="flex flex-col">
            {pinnedSessions.length > 0 ? (
              pinnedSessions.map((session) => (
                <SessionRowItem
                  key={session.id}
                  session={session}
                  active={session.id === activeSessionId}
                  restoring={isRestoring && restoringSessionId === session.id}
                  pinned={true}
                  onSelect={handleSelect}
                  onTogglePin={togglePin}
                />
              ))
            ) : (
              <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {SESSION_COPY.noPinnedSessions}
              </span>
            )}
          </div>
        </section>

        {/* ── Projects with FolderPlus & Accordion ── */}
        <section className="flex flex-col gap-1">
          <div className="flex items-center justify-between px-2 py-0.5">
            <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {SESSION_COPY.projectsSection}
            </span>
            <Tooltip label={SESSION_COPY.createProjectAriaLabel} side="left">
              <button
                type="button"
                onClick={() => setIsCreatingProject((v) => !v)}
                className="flex items-center justify-center w-6 h-6 rounded text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer"
                aria-label={SESSION_COPY.createProjectAriaLabel}
              >
                <FolderPlus size={14} />
              </button>
            </Tooltip>
          </div>

          {/* Inline Project Creation Input */}
          <AnimatePresence>
            {isCreatingProject && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                className="overflow-hidden px-2 pb-1"
              >
                <div className="flex items-center gap-1.5 bg-[rgba(var(--foreground),0.05)] rounded-lg px-2.5 py-1 focus-within:ring-1 focus-within:ring-[rgba(var(--accent),0.4)]">
                  <input
                    autoFocus
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleCreateProject();
                      if (e.key === "Escape") {
                        setIsCreatingProject(false);
                        setProjectName("");
                      }
                    }}
                    placeholder={SESSION_COPY.newProjectPlaceholder}
                    aria-label={SESSION_COPY.createProjectAriaLabel}
                    className="flex-1 min-w-0 bg-transparent text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={handleCreateProject}
                    disabled={!projectName.trim()}
                    className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] disabled:opacity-30 cursor-pointer"
                  >
                    <Check size={12} />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setIsCreatingProject(false);
                      setProjectName("");
                    }}
                    className="p-1 rounded text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.1)] cursor-pointer"
                  >
                    <X size={12} />
                  </button>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* Projects List Accordions */}
          {projects.length > 0 ? (
            projects.map((group) => {
              const isExpanded = isProjectExpanded(group.project.id);
              return (
                <div key={group.project.id} className="flex flex-col">
                  {/* Project Header Row */}
                  <div
                    role="button"
                    tabIndex={0}
                    onClick={() => toggleProject(group.project.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") toggleProject(group.project.id);
                    }}
                    className="group flex items-center justify-between gap-1.5 px-2 py-1.5 rounded-lg text-left cursor-pointer hover:bg-[rgba(var(--foreground),0.04)] transition-colors select-none"
                  >
                    <div className="flex items-center gap-1.5 min-w-0 flex-1">
                      <ChevronRight
                        size={13}
                        className={cn(
                          "shrink-0 text-[rgb(var(--foreground-muted))]/60 transition-transform duration-200",
                          isExpanded && "rotate-90 text-[rgb(var(--foreground))]"
                        )}
                      />
                      {isExpanded ? (
                        <FolderOpen size={14} className="shrink-0 text-[rgb(var(--accent))]" />
                      ) : (
                        <Folder size={14} className="shrink-0 text-[rgb(var(--foreground-muted))]" />
                      )}
                      <span className="truncate text-[12.5px] font-medium text-[rgb(var(--foreground))]">
                        {group.project.name}
                      </span>
                    </div>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/50 px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.04)] shrink-0">
                      {group.sessions.length}
                    </span>
                  </div>

                  {/* Accordion Sessions */}
                  <AnimatePresence initial={false}>
                    {isExpanded && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: "auto" }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.18, ease: "easeInOut" }}
                        className="flex flex-col overflow-hidden"
                      >
                        {group.sessions.length > 0 ? (
                          group.sessions.map((session) => (
                            <SessionRowItem
                              key={session.id}
                              session={session}
                              active={session.id === activeSessionId}
                              restoring={isRestoring && restoringSessionId === session.id}
                              pinned={false}
                              nested={true}
                              onSelect={handleSelect}
                              onTogglePin={togglePin}
                            />
                          ))
                        ) : (
                          <span className="pl-7 py-1 text-[11px] text-[rgb(var(--foreground-muted))]/50 italic">
                            {SESSION_COPY.noProjectSessions}
                          </span>
                        )}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              );
            })
          ) : (
            <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
              {SESSION_COPY.noProjectsYet}
            </span>
          )}
        </section>

        {/* ── Uncategorized / Recent Sessions ── */}
        <section className="flex flex-col gap-0.5">
          <div className="px-2 py-0.5">
            <span className="text-[10.5px] font-mono font-bold tracking-[0.16em] uppercase text-[rgb(var(--foreground-muted))]/70">
              {pinnedSessions.length > 0 || projects.some((p) => p.sessions.length > 0)
                ? SESSION_COPY.uncategorizedSection
                : SESSION_COPY.conversationsSection}
            </span>
          </div>
          <div className="flex flex-col">
            {uncategorizedSessions.length > 0 ? (
              uncategorizedSessions.map((session) => (
                <SessionRowItem
                  key={session.id}
                  session={session}
                  active={session.id === activeSessionId}
                  restoring={isRestoring && restoringSessionId === session.id}
                  pinned={false}
                  onSelect={handleSelect}
                  onTogglePin={togglePin}
                />
              ))
            ) : (
              <span className="px-2.5 py-1 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic">
                {pinnedSessions.length > 0 || projects.some((p) => p.sessions.length > 0)
                  ? SESSION_COPY.noOtherSessions
                  : SESSION_COPY.noSessionsTitle}
              </span>
            )}
          </div>
        </section>

        {/* Bottom spacing cushion */}
        <div className="h-8 shrink-0" aria-hidden="true" />
      </div>
    </div>
  );
});
SessionPanel.displayName = "SessionPanel";
