import React, { memo, useMemo } from "react";
import { motion } from "framer-motion";
import { FlaskConical, Play, X, Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { TEST_CLIPS, TEST_CLIPS_COPY, type TestClip } from "@/data/homeCopy";

interface TestClipsPopoverProps {
  panelRef: React.RefObject<HTMLDivElement | null>;
  onSelectClip: (clipId: string) => void;
  onClose?: () => void;
  testingClip?: string | null;
}

interface ClipGroupProps {
  title: string;
  count: number;
  clips: TestClip[];
  onSelectClip: (clipId: string) => void;
  onClose?: () => void;
  testingClip?: string | null;
}

const ClipGroup = memo(({ title, count, clips, onSelectClip, onClose, testingClip }: ClipGroupProps) => {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between px-1.5 pt-1">
        <span className="text-[10px] font-mono tracking-widest text-[rgb(var(--foreground-muted))] uppercase font-semibold">
          {title}
        </span>
        <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60">
          {count}
        </span>
      </div>
      <div className="flex flex-col gap-1">
        {clips.map((clip) => {
          const isThisClipActive = testingClip === clip.id;
          const isAnyClipActive = Boolean(testingClip);

          return (
            <button
              key={clip.id}
              type="button"
              disabled={isAnyClipActive}
              onClick={() => {
                onSelectClip(clip.id);
                onClose?.();
              }}
              className={cn(
                "group w-full text-left px-3 py-2.5 rounded-xl transition-all duration-150 border border-[rgba(var(--border),0.06)] flex items-center justify-between gap-3 cursor-pointer",
                isThisClipActive
                  ? "border-[rgba(var(--accent),0.4)] bg-[rgba(var(--accent),0.12)] shadow-sm"
                  : "hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.08)] bg-[rgba(var(--card),0.4)] active:scale-[0.99]",
                isAnyClipActive && !isThisClipActive && "opacity-50 cursor-not-allowed"
              )}
            >
              <div className="flex flex-col min-w-0 flex-1">
                <span className={cn(
                  "text-[13px] font-semibold transition-colors truncate",
                  isThisClipActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))]"
                )}>
                  {clip.label}
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5 leading-snug line-clamp-1">
                  {clip.desc}
                </span>
              </div>
              <div className={cn(
                "shrink-0 p-1.5 rounded-lg transition-all",
                isThisClipActive
                  ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]"
                  : "bg-[rgba(var(--foreground),0.05)] group-hover:bg-[rgb(var(--accent))]/15 text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))]"
              )}>
                {isThisClipActive ? (
                  <Loader2 size={12} className="animate-spin text-[rgb(var(--accent))]" />
                ) : (
                  <Play size={12} className="group-hover:translate-x-0.5 transition-transform" />
                )}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
});

ClipGroup.displayName = "ClipGroup";

export const TestClipsPopover = memo(({
  panelRef,
  onSelectClip,
  onClose,
  testingClip,
}: TestClipsPopoverProps) => {
  const englishClips = useMemo(() => TEST_CLIPS.filter((c) => c.lang === "en"), []);
  const hindiClips = useMemo(() => TEST_CLIPS.filter((c) => c.lang === "hi"), []);

  return (
    <motion.div
      key="test-mode-panel"
      ref={panelRef}
      initial={{ opacity: 0, y: 12, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 12, scale: 0.97 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
      className="fixed bottom-16 right-4 w-[360px] p-3 flex flex-col gap-2 z-50 glass-card rounded-2xl shadow-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.85)] backdrop-blur-xl"
    >
      {/* ── Popover Header ────────────────── */}
      <div className="flex items-center justify-between px-1.5 pb-2 border-b border-[rgba(var(--border),0.08)]">
        <div className="flex items-center gap-2">
          <div className="p-1 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))]">
            <FlaskConical size={14} />
          </div>
          <span className="text-[11px] font-mono tracking-widest text-[rgb(var(--foreground))] uppercase font-bold">
            {TEST_CLIPS_COPY.title}
          </span>
          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded-full bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] font-semibold">
            {TEST_CLIPS.length}
          </span>
        </div>

        {onClose && (
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer"
            aria-label={TEST_CLIPS_COPY.closePanel}
          >
            <X size={14} />
          </button>
        )}
      </div>

      {/* ── Inner Scrolling Clips Container ────────────── */}
      <div className="max-h-[360px] overflow-y-auto pr-1 space-y-3.5 scrollbar-thin scrollbar-thumb-[rgba(var(--foreground),0.15)] scrollbar-track-transparent">
        <ClipGroup
          title={TEST_CLIPS_COPY.englishTitle}
          count={englishClips.length}
          clips={englishClips}
          onSelectClip={onSelectClip}
          onClose={onClose}
          testingClip={testingClip}
        />

        <ClipGroup
          title={TEST_CLIPS_COPY.hindiTitle}
          count={hindiClips.length}
          clips={hindiClips}
          onSelectClip={onSelectClip}
          onClose={onClose}
          testingClip={testingClip}
        />
      </div>
    </motion.div>
  );
});

TestClipsPopover.displayName = "TestClipsPopover";
