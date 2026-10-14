import React, { memo, useCallback, useMemo, useState, useRef, useEffect } from "react";
import { FolderPlus, Check, X } from "lucide-react";
import { AnimatePresence, motion, Reorder } from "framer-motion";
import { type ProjectGroup } from "@/shared/hooks/useSessionPanel";
import { type ProjectRow } from "@/services/projectService";
import { SESSION_COPY } from "@/data/sessionCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ProjectRowItem } from "./ProjectRowItem";

export interface SessionProjectsSectionProps {
  projects: ProjectGroup[];
  allProjects: ProjectRow[];
  activeSessionId: number | null;
  isRestoring: boolean;
  restoringSessionId: number | null;
  createNewProject: (name: string) => Promise<unknown>;
  reorderProjects: (order: string[]) => void;
  renameProject: (id: string, name: string) => void;
  deleteProject: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
  onSelectSession: (id: number) => void;
  onTogglePinSession: (id: number) => void;
  onRenameSession: (id: number, newTitle: string) => void;
  onMoveSessionToProject: (id: number, projectId: string | null) => void;
  onDeleteSession: (id: number) => void;
  onCreateSessionInProject: (projectId: string) => void;
}

export const SessionProjectsSection = memo(
  ({
    projects,
    allProjects,
    activeSessionId,
    isRestoring,
    restoringSessionId,
    createNewProject,
    reorderProjects,
    renameProject,
    deleteProject,
    refresh,
    onSelectSession,
    onTogglePinSession,
    onRenameSession,
    onMoveSessionToProject,
    onDeleteSession,
    onCreateSessionInProject,
  }: SessionProjectsSectionProps) => {
    const [isCreatingProject, setIsCreatingProject] = useState(false);
    const [projectName, setProjectName] = useState("");
    const [dragOverProjectId, setDragOverProjectId] = useState<string | null>(null);

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

    // Real-time project reorder with local optimistic IDs
    const [projectIds, setProjectIds] = useState<string[]>(() =>
      projects.map((g) => g.project.id)
    );
    const isDraggingProjectRef = useRef(false);

    // Sync with incoming projects prop ONLY if not actively dragging
    useEffect(() => {
      if (!isDraggingProjectRef.current) {
        setProjectIds(projects.map((g) => g.project.id));
      }
    }, [projects]);

    const projectIdsRef = useRef(projectIds);
    useEffect(() => {
      projectIdsRef.current = projectIds;
    }, [projectIds]);

    const handleReorderProjectIds = useCallback((newIds: string[]) => {
      isDraggingProjectRef.current = true;
      projectIdsRef.current = newIds;
      setProjectIds(newIds);
    }, []);

    const handleDragEndCommit = useCallback(() => {
      isDraggingProjectRef.current = false;
      reorderProjects(projectIdsRef.current);
    }, [reorderProjects]);

    // Fast O(1) project group lookup by ID
    const projectGroupMap = useMemo(() => {
      const map = new Map<string, ProjectGroup>();
      for (const g of projects) {
        map.set(g.project.id, g);
      }
      return map;
    }, [projects]);

    // Session drop into project handlers
    const handleSessionDragOver = useCallback(
      (e: React.DragEvent, targetProjectId: string) => {
        if (e.dataTransfer.types.includes("text/session-id")) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          setDragOverProjectId((prev) => (prev === targetProjectId ? prev : targetProjectId));
        }
      },
      []
    );

    const handleSessionDragLeave = useCallback((targetProjectId: string) => {
      setDragOverProjectId((prev) => (prev === targetProjectId ? null : prev));
    }, []);

    const handleSessionDrop = useCallback(
      (e: React.DragEvent, targetProjectId: string) => {
        e.preventDefault();
        setDragOverProjectId(null);
        const sessionIdStr = e.dataTransfer.getData("text/session-id");
        if (sessionIdStr) {
          const sId = Number(sessionIdStr);
          if (sId) {
            onMoveSessionToProject(sId, targetProjectId);
          }
        }
      },
      [onMoveSessionToProject]
    );

    return (
      <div className="flex flex-col flex-1 min-h-0">
        {/* Fixed Projects Section Header */}
        <section className="shrink-0 flex flex-col gap-1">
          <div className="flex items-center justify-between px-2 py-0.5">
            <div className="flex items-center gap-1.5">
              <span className="text-[11.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/60">
                {SESSION_COPY.projectsSection}
              </span>
            </div>
            <Tooltip label={SESSION_COPY.createProjectAriaLabel} side="left">
              <button
                type="button"
                onClick={() => setIsCreatingProject((v) => !v)}
                className="flex items-center justify-center w-5 h-5 rounded text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer"
                aria-label={SESSION_COPY.createProjectAriaLabel}
              >
                <FolderPlus size={13} />
              </button>
            </Tooltip>
          </div>

          {/* Inline Project Creation */}
          <AnimatePresence>
            {isCreatingProject && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                className="overflow-hidden px-2 pb-1"
              >
                <div className="flex items-center gap-1.5 border-b border-[rgba(var(--accent),0.5)] focus-within:border-[rgba(var(--accent),0.9)] transition-colors py-1">
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
                    className="flex-1 min-w-0 bg-transparent text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none"
                  />
                  <button
                    type="button"
                    onClick={handleCreateProject}
                    disabled={!projectName.trim()}
                    className="flex items-center justify-center w-6 h-6 rounded text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] disabled:opacity-30 cursor-pointer transition-colors"
                  >
                    <Check size={13} />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setIsCreatingProject(false);
                      setProjectName("");
                    }}
                    className="flex items-center justify-center w-6 h-6 rounded text-[rgb(var(--foreground-muted))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer transition-colors"
                  >
                    <X size={13} />
                  </button>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </section>

        {/* Scrollable Projects List */}
        <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-0.5 pt-1 pb-16">
          {projectIds.length > 0 ? (
            <Reorder.Group
              as="div"
              axis="y"
              values={projectIds}
              onReorder={handleReorderProjectIds}
              className="flex flex-col gap-0.5"
            >
              {projectIds.map((id) => {
                const group = projectGroupMap.get(id);
                if (!group) return null;
                return (
                  <ProjectRowItem
                    key={id}
                    group={group}
                    allProjects={allProjects}
                    activeSessionId={activeSessionId}
                    isRestoring={isRestoring}
                    restoringSessionId={restoringSessionId}
                    isDragTarget={dragOverProjectId === id}
                    onSelectSession={onSelectSession}
                    onTogglePinSession={onTogglePinSession}
                    onRenameSession={onRenameSession}
                    onMoveSessionToProject={onMoveSessionToProject}
                    onDeleteSession={onDeleteSession}
                    onCreateSessionInProject={onCreateSessionInProject}
                    onRenameProject={renameProject}
                    onDeleteProject={deleteProject}
                    onDragOver={handleSessionDragOver}
                    onDragLeave={handleSessionDragLeave}
                    onDrop={handleSessionDrop}
                    onDragEndCommit={handleDragEndCommit}
                  />
                );
              })}
            </Reorder.Group>
          ) : (
            <span className="px-2 py-1 text-[11px] text-[rgb(var(--foreground-muted))]/35 italic">
              {SESSION_COPY.noProjectsYet}
            </span>
          )}
        </div>
      </div>
    );
  }
);
SessionProjectsSection.displayName = "SessionProjectsSection";
