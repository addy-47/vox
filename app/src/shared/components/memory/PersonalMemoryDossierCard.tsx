import React, { memo, useEffect } from "react";
import { FileText, Copy, Check, RotateCw, MessageSquare } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import Lenis from "lenis";
import { PixelSynthesisCanvas } from "./PixelSynthesisCanvas";
import { PersonalMemoryVersionNav } from "./PersonalMemoryVersionNav";
import { PersonalMemoryCommentPopover, type SelectionAnchor } from "./PersonalMemoryCommentPopover";
import type { MemoryComment } from "./stagingTypes";
import { Tooltip, Markdown } from "@/shared/ui";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import type { PersonalMemoryRecord } from "@/services/memoryService";

export interface PersonalMemoryDossierCardProps {
  personalMemory: PersonalMemoryRecord | null;
  displayedRecord: PersonalMemoryRecord | null;
  versions: PersonalMemoryRecord[];
  onSelectVersion: (rec: PersonalMemoryRecord) => void;
  onCommitActiveVersion: (version: number) => Promise<void>;
  isRestoringVersion: boolean;
  onCopyDoc: () => void;
  copied: boolean;
  onRegenerateFromFacts: () => void;
  isRegenerating: boolean;
  saving: boolean;
  leftFlash: boolean;
  veilCycle: number;
  onVeilReady: () => void;
  dossierContainerRef: React.RefObject<HTMLDivElement | null>;
  selectionAnchor: SelectionAnchor | null;
  onAddComment: (comment: { line: number; quotedText: string; text: string; top: number }) => void;
  onCancelComment: () => void;
  setIsComposingComment: (v: boolean) => void;
  comments: MemoryComment[];
  onSelectComment: () => void;
  drawerOpen: boolean;
  drawerBodyReady: boolean;
  hideActions?: boolean;
  title?: string;
  embedded?: boolean;
}

export const PersonalMemoryDossierCard: React.FC<PersonalMemoryDossierCardProps> = memo(({
  personalMemory,
  displayedRecord,
  versions,
  onSelectVersion,
  onCommitActiveVersion,
  isRestoringVersion,
  onCopyDoc,
  copied,
  onRegenerateFromFacts,
  isRegenerating,
  saving,
  leftFlash,
  veilCycle,
  onVeilReady,
  dossierContainerRef,
  selectionAnchor,
  onAddComment,
  onCancelComment,
  setIsComposingComment,
  comments,
  onSelectComment,
  drawerOpen,
  drawerBodyReady,
  hideActions = false,
  title,
  embedded = false,
}) => {
  // Scoped Lenis smooth scrolling for personal memory dossier
  useEffect(() => {
    if (!drawerOpen || !drawerBodyReady) return undefined;
    const el = dossierContainerRef.current;
    if (!el) return undefined;

    const lenis = new Lenis({
      wrapper: el,
      content: el,
      eventsTarget: el,
      smoothWheel: true,
      autoRaf: true,
      duration: 0.8,
    });

    return () => {
      lenis.destroy();
    };
  }, [drawerOpen, drawerBodyReady, dossierContainerRef]);

  const displayTitle = title ?? MEMORY_COPY.dossierTitle;

  return (
    <div
      className={cn(
        "relative w-full h-full min-h-0 flex flex-col overflow-hidden",
        embedded
          ? "border-none bg-transparent shadow-none p-0"
          : "glass-card rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgba(var(--card),0.65)] backdrop-blur-sm p-5 sm:p-6 shadow-2xl"
      )}
    >
      <AnimatePresence>
        {leftFlash && (
          <motion.div
            key={`veil-${veilCycle}`}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.35, ease: "easeInOut" }}
            onAnimationComplete={onVeilReady}
            className={cn(
              "absolute inset-0 z-30 overflow-hidden bg-[rgba(var(--card),0.85)] backdrop-blur-md pointer-events-none",
              !embedded && "rounded-2xl"
            )}
          >
            <PixelSynthesisCanvas active={leftFlash} />
          </motion.div>
        )}
      </AnimatePresence>

      {/* Dossier Header Bar — icon-only trailing actions with tooltips so the
          row survives narrow (modal) widths without crushing the title. */}
      <div className="flex items-center justify-between gap-3 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
        <div className="flex items-center gap-3 min-w-0 flex-1">
          <div className="w-8 h-8 rounded-xl bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] shadow-sm shrink-0">
            <FileText size={16} />
          </div>
          <div className="flex flex-col min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))] truncate">
                {displayTitle}
              </span>
            </div>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate">
              {personalMemory
                ? `${MEMORY_COPY.lastUpdated} ${new Date(personalMemory.updated_at).toLocaleDateString(undefined, {
                    month: "short",
                    day: "numeric",
                    year: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  })}`
                : MEMORY_COPY.identityLayer}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          <PersonalMemoryVersionNav
            versions={versions}
            activeVersionRecord={personalMemory}
            displayedRecord={displayedRecord}
            onSelectVersion={onSelectVersion}
            onCommitActiveVersion={onCommitActiveVersion}
            isRestoring={isRestoringVersion}
          />

          {!hideActions && (
            <>
              <Tooltip label={copied ? MEMORY_COPY.copied : MEMORY_COPY.copyDocTitle}>
                <button
                  type="button"
                  onClick={onCopyDoc}
                  disabled={!displayedRecord?.markdown && !displayedRecord?.content}
                  aria-label={MEMORY_COPY.copyDocTitle}
                  className="flex items-center justify-center w-7 h-7 rounded-xl text-[rgb(var(--foreground-muted))] bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] hover:text-[rgb(var(--foreground))] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  {copied ? <Check size={13} className="text-[rgb(var(--accent))]" /> : <Copy size={13} />}
                </button>
              </Tooltip>

              <Tooltip label={isRegenerating ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerateTooltip}>
                <button
                  type="button"
                  onClick={onRegenerateFromFacts}
                  disabled={isRegenerating || saving}
                  aria-label={MEMORY_COPY.regenerateTooltip}
                  className="flex items-center justify-center w-7 h-7 rounded-xl text-[rgb(var(--foreground-muted))] bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] hover:text-[rgb(var(--foreground))] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  <RotateCw size={13} className={cn(isRegenerating && "animate-spin text-[rgb(var(--accent))]")} />
                </button>
              </Tooltip>
            </>
          )}
        </div>
      </div>

      {/* Dossier Document Content with Inner Scrolling */}
      <div
        ref={dossierContainerRef}
        className="relative flex-1 min-h-0 overflow-y-auto custom-scrollbar px-4 sm:px-6 pt-5 sm:pt-6 pb-8 leading-relaxed max-w-none select-text"
      >
        {/* Inline text selection comment popover */}
        <PersonalMemoryCommentPopover
          anchor={selectionAnchor}
          onAddComment={onAddComment}
          onCancel={onCancelComment}
          onOpenChange={setIsComposingComment}
        />

        {/* Persistent text highlight overlay while commenting */}
        {selectionAnchor?.rects?.map((r, i) => (
          <div
            key={`sel_rect_${i}`}
            className="absolute pointer-events-none rounded-[2px] bg-[rgba(var(--accent),0.28)] transition-opacity duration-150 z-10"
            style={{
              top: `${r.top}px`,
              left: `${r.left}px`,
              width: `${r.width}px`,
              height: `${r.height}px`,
            }}
          />
        ))}

        {/* Persistent Comment Icons on Right Margin for all saved comments (Image 3 layout) */}
        {comments.map((c) => (
          <div
            key={c.id}
            className="absolute right-1 z-30 pointer-events-auto transition-transform hover:scale-110"
            style={{ top: `${Math.max(4, c.top)}px` }}
          >
              <Tooltip label={`${MEMORY_COPY.linePrefix} ${c.line}: ${c.text}`} side="left">
              <button
                type="button"
                onClick={onSelectComment}
                className="w-5 h-5 text-[rgb(var(--accent))] hover:scale-115 flex items-center justify-center cursor-pointer transition-transform opacity-80 hover:opacity-100"
              >
                <MessageSquare size={13} />
              </button>
            </Tooltip>
          </div>
        ))}

        {drawerBodyReady ? (
          displayedRecord?.markdown || displayedRecord?.content ? (
            <Markdown
              content={displayedRecord.markdown || displayedRecord.content}
              variant="document"
              autoHeadings
            />
          ) : (
            <p className="text-[rgb(var(--foreground-muted))] text-[13px] font-mono py-12 text-center">
              {MEMORY_COPY.noPersonalMemory}
            </p>
          )
        ) : (
          <div className="py-12 flex justify-center">
            <div className="w-6 h-6 rounded-full border-2 border-[rgba(var(--accent),0.4)] border-t-[rgb(var(--accent))] animate-spin" />
          </div>
        )}
      </div>
    </div>
  );
});

PersonalMemoryDossierCard.displayName = "PersonalMemoryDossierCard";
