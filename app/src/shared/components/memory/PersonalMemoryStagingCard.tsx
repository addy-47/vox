import React, { useState, useEffect, useRef, memo } from "react";
import {
  Upload,
  Edit3,
  Check,
  X,
  Sparkles,
  ArrowLeft,
  MessageSquare,
  Trash2,
  Layers,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

export type StagingMode = "idle" | "import" | "edit" | "comment";

export interface MemoryComment {
  id: string;
  line: number;
  quotedText: string;
  text: string;
  top: number;
  createdAt: number;
}

export interface PersonalMemoryStagingCardProps {
  canonicalContent: string | null;
  mode: StagingMode;
  onModeChange: (mode: StagingMode) => void;
  onSave: (content: string) => Promise<void>;
  onRegenerateWithComments?: (comments: MemoryComment[], policy?: "pause_compaction" | "queue") => Promise<void>;
  comments?: MemoryComment[];
  onDeleteComment?: (id: string) => void;
  onUpdateComment?: (id: string, text: string) => void;
  onClearComments?: () => void;
  unconsolidatedCount: number;
  isSaving: boolean;
  isConsolidating?: boolean;
  isCommitting: boolean;
}

export const PersonalMemoryStagingCard: React.FC<PersonalMemoryStagingCardProps> = memo(
  ({
    canonicalContent,
    mode,
    onModeChange,
    onSave,
    onRegenerateWithComments,
    comments = [],
    onDeleteComment,
    onUpdateComment,
    onClearComments,
    unconsolidatedCount: _unconsolidatedCount,
    isSaving,
    isConsolidating = false,
    isCommitting,
  }) => {
    const [draft, setDraft] = useState("");
    const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
    const [editingCommentText, setEditingCommentText] = useState("");
    const [conflictInPlace, setConflictInPlace] = useState(false);

    const prevModeRef = useRef(mode);

    // Sync draft only when MODE CHANGES (entry), never on canonicalContent updates.
    // This prevents background consolidations from clobbering an in-progress edit.
    useEffect(() => {
      const entered = prevModeRef.current !== mode;
      prevModeRef.current = mode;
      if (!entered) return;
      setConflictInPlace(false);
      if (mode === "edit") setDraft(canonicalContent ?? "");
      else if (mode === "import") setDraft("");
    }, [mode]); // deliberately omit canonicalContent

    const handleCommit = async () => {
      if (!draft.trim()) return;
      await onSave(draft);
    };

    const handleRegenerate = async (policy?: "pause_compaction" | "queue") => {
      if (!onRegenerateWithComments || comments.length === 0) return;
      try {
        await onRegenerateWithComments(comments, policy);
        setConflictInPlace(false);
      } catch (e: unknown) {
        const msg = (e as { message?: string })?.message || String(e);
        if (msg.includes("active compaction is in progress")) {
          setConflictInPlace(true);
        } else {
          console.error("[PersonalMemoryStagingCard] Regenerate failed:", e);
        }
      }
    };

    return (
      <div
        className={cn(
          "relative w-full h-full min-h-0 rounded-2xl p-5 sm:p-6 flex flex-col transition-all duration-500 overflow-hidden",
          "glass-card border bg-[rgba(var(--card),0.45)] backdrop-blur-sm contain-paint transform-gpu",
          mode === "edit" || mode === "import" || mode === "comment"
            ? "border-[rgba(var(--accent),0.25)] shadow-lg"
            : "border-[rgba(var(--accent),0.18)] hover:border-[rgba(var(--accent),0.35)] shadow-2xl",
          (isSaving || isConsolidating || isCommitting) && "opacity-40 pointer-events-none select-none"
        )}
      >
        {/* Dynamic Header Bar */}
        <div className="flex items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
          <div className="flex items-center gap-3">
            <div
              className={cn(
                "w-8 h-8 rounded-xl border flex items-center justify-center transition-colors shadow-sm",
                mode === "comment"
                  ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]"
                  : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]"
              )}
            >
              {mode === "comment" ? (
                <MessageSquare size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "edit" ? (
                <Edit3 size={16} className="text-[rgb(var(--accent))]" />
              ) : mode === "import" ? (
                <Upload size={16} className="text-[rgb(var(--accent))]" />
              ) : (
                <Layers size={16} />
              )}
            </div>

            <div className="flex flex-col">
              <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))]">
                {mode === "comment"
                  ? MEMORY_COPY.documentComments
                  : mode === "edit"
                  ? "Direct In-Place Edit"
                  : mode === "import"
                  ? "Import / Paste Memory"
                  : "Staging Mirror"}
              </span>
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                {mode === "comment"
                  ? `${comments.length} line-anchored ${comments.length === 1 ? "comment" : "comments"} pending`
                  : mode === "edit"
                  ? "Changes flow directly to persistent memory"
                  : mode === "import"
                  ? "Paste markdown to replace current profile"
                  : "Interactive draft workspace"}
              </span>
            </div>
          </div>

          {/* Action controls in header - ONLY shown during active modes */}
          <div className="flex items-center gap-2">
            {isCommitting ? (
              <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.14)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] shadow-sm transition-all duration-300">
                <Check size={12} className="text-[rgb(var(--accent))]" />
                <span>Saved</span>
              </div>
            ) : isConsolidating ? (
              <span className="px-2.5 py-1 rounded-full text-[10px] font-mono font-medium tracking-wide bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))] animate-pulse">
                Synthesizing…
              </span>
            ) : mode === "comment" ? (
              <>
                {conflictInPlace ? (
                  <div className="flex items-center gap-1.5 p-0.5 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.3)] animate-in fade-in duration-150">
                    <button
                      type="button"
                      disabled={isSaving}
                      onClick={() => handleRegenerate("pause_compaction")}
                      className="px-2 py-1 rounded-lg text-[10.5px] font-mono font-medium text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-colors cursor-pointer disabled:opacity-50"
                    >
                      {isSaving ? "Pausing…" : "Pause & Run"}
                    </button>
                    <button
                      type="button"
                      disabled={isSaving}
                      onClick={() => handleRegenerate("queue")}
                      className="px-2 py-1 rounded-lg text-[10.5px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer disabled:opacity-50"
                    >
                      {isSaving ? "Queueing…" : "Queue"}
                    </button>
                    <button
                      type="button"
                      disabled={isSaving}
                      onClick={() => setConflictInPlace(false)}
                      className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer"
                    >
                      <X size={11} />
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => handleRegenerate()}
                    disabled={isSaving || comments.length === 0}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                    title="Regenerate document applying all anchored comments"
                  >
                    <Sparkles size={13} className={cn(isSaving && "animate-spin")} />
                    {isSaving ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerate}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => {
                    setConflictInPlace(false);
                    onClearComments?.();
                    onModeChange("idle");
                  }}
                  disabled={isSaving}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  <X size={13} /> {MEMORY_COPY.cancel}
                </button>
              </>
            ) : mode === "edit" || mode === "import" ? (
              <>
                <button
                  type="button"
                  onClick={handleCommit}
                  disabled={isSaving || !draft.trim()}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  <ArrowLeft size={13} className={cn(isSaving && "animate-pulse")} />
                  {isSaving ? "Saving…" : "Save & Commit"}
                </button>
                <button
                  type="button"
                  onClick={() => onModeChange("idle")}
                  disabled={isSaving}
                  className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                >
                  <X size={13} /> {MEMORY_COPY.cancel}
                </button>
              </>
            ) : null}
          </div>
        </div>

        {/* Mode 1: Clean Centered Action Hub (Replaces busy skeleton wireframe) */}
        {mode === "idle" && (
          <div className="flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center animate-in fade-in duration-300">
            <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] flex items-center justify-center text-[rgb(var(--accent))] mb-3.5 shadow-sm">
              <Layers size={20} />
            </div>

            <h3 className="text-[14px] font-semibold tracking-wide text-[rgb(var(--foreground))] mb-1">
              Personal Memory Staging
            </h3>
            <p className="text-[11.5px] text-[rgb(var(--foreground-muted))] max-w-xs mb-6 font-mono leading-relaxed">
              Choose an action to edit, import, or regenerate your personal dossier.
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 w-full max-w-sm">
              {/* Option 1: Comment */}
              <button
                type="button"
                onClick={() => onModeChange("comment")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <MessageSquare size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Comment
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  {comments.length > 0 ? `${comments.length} ready` : "Line directives"}
                </span>
              </button>

              {/* Option 2: Import */}
              <button
                type="button"
                onClick={() => onModeChange("import")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <Upload size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Import
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  Paste markdown
                </span>
              </button>

              {/* Option 3: Edit */}
              <button
                type="button"
                onClick={() => onModeChange("edit")}
                className="flex flex-col items-center justify-center p-3.5 rounded-xl glass-card border border-[rgba(var(--border),0.18)] hover:border-[rgba(var(--accent),0.4)] bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--accent),0.06)] transition-all cursor-pointer group shadow-sm"
              >
                <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] flex items-center justify-center mb-2 group-hover:scale-110 transition-transform">
                  <Edit3 size={15} />
                </div>
                <span className="text-[12px] font-semibold text-[rgb(var(--foreground))] mb-0.5">
                  Edit
                </span>
                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                  In-place editor
                </span>
              </button>
            </div>
          </div>
        )}

        {/* Mode 2: Comment Queue Slate */}
        {mode === "comment" && (
          <div className="flex-1 min-h-0 flex flex-col justify-between pt-3 transition-opacity duration-500">
            {comments.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center p-6 text-center">
                <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] flex items-center justify-center text-[rgb(var(--accent))] mb-3">
                  <MessageSquare size={20} />
                </div>
                <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
                  {MEMORY_COPY.noComments}
                </h4>
                <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
                  {MEMORY_COPY.selectTextPrompt}
                </p>
              </div>
            ) : (
              <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 space-y-2.5">
                {comments.map((c) => (
                  <div
                    key={c.id}
                    className="p-3 rounded-xl bg-[rgba(var(--card),0.6)] border border-[rgba(var(--accent),0.22)] shadow-sm hover:border-[rgba(var(--accent),0.4)] transition-all flex flex-col gap-1.5 group"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex items-center gap-2">
                        <span className="px-2 py-0.5 rounded-md text-[10px] font-mono font-bold  text-[rgb(var(--accent))] ]">
                          {MEMORY_COPY.linePrefix} {c.line}
                        </span>
                        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] truncate max-w-[180px] sm:max-w-[240px]">
                          &ldquo;{c.quotedText}&rdquo;
                        </span>
                      </div>

                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          type="button"
                          onClick={() => {
                            setEditingCommentId(c.id);
                            setEditingCommentText(c.text);
                          }}
                          className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] opacity-60 group-hover:opacity-100 transition-all cursor-pointer"
                          title="Edit comment"
                        >
                          <Edit3 size={13} />
                        </button>
                        {onDeleteComment && (
                          <button
                            type="button"
                            onClick={() => {
                              if (editingCommentId === c.id) setEditingCommentId(null);
                              onDeleteComment(c.id);
                            }}
                            className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-500/10 opacity-60 group-hover:opacity-100 transition-all cursor-pointer"
                            title={MEMORY_COPY.deleteCommentTooltip}
                          >
                            <Trash2 size={13} />
                          </button>
                        )}
                      </div>
                    </div>

                    {editingCommentId === c.id ? (
                      <div className="flex flex-col gap-2 pt-1 animate-in fade-in duration-150">
                        <textarea
                          autoFocus
                          value={editingCommentText}
                          onChange={(e) => setEditingCommentText(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                              if (editingCommentText.trim() && onUpdateComment) {
                                onUpdateComment(c.id, editingCommentText.trim());
                              }
                              setEditingCommentId(null);
                            } else if (e.key === "Escape") {
                              setEditingCommentId(null);
                            }
                          }}
                          rows={2}
                          className="w-full bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.4)] focus:border-[rgb(var(--accent))] rounded-lg p-2 text-[12px] text-[rgb(var(--foreground))] focus:outline-none leading-relaxed font-sans resize-none transition-colors"
                        />
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            type="button"
                            onClick={() => setEditingCommentId(null)}
                            className="px-2 py-0.5 rounded text-[11px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] cursor-pointer transition-colors"
                          >
                            Cancel
                          </button>
                          <button
                            type="button"
                            onClick={() => {
                              if (editingCommentText.trim() && onUpdateComment) {
                                onUpdateComment(c.id, editingCommentText.trim());
                              }
                              setEditingCommentId(null);
                            }}
                            disabled={!editingCommentText.trim()}
                            className="px-2.5 py-0.5 rounded text-[11px] font-mono font-medium bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] disabled:opacity-40 cursor-pointer transition-colors shadow-xs"
                          >
                            Save
                          </button>
                        </div>
                      </div>
                    ) : (
                      <p className="text-[12px] text-[rgb(var(--foreground))] leading-relaxed pl-0.5 font-sans">
                        {c.text}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            )}

            {comments.length > 0 && (
              <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-3 border-t border-[rgba(var(--border),0.08)]">
                <span>{comments.length} comments queued</span>
                {onClearComments && (
                  <button
                    type="button"
                    onClick={onClearComments}
                    className="text-[11px] text-red-400 hover:underline cursor-pointer"
                  >
                    {MEMORY_COPY.clearAllComments}
                  </button>
                )}
              </div>
            )}
          </div>
        )}

        {/* Mode 3: Import Textarea */}
        {mode === "import" && (
          <div
            className={cn(
              "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
              isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
            )}
          >
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="Paste your markdown directives here (e.g. ## Overview, ## Preferences, etc.)..."
              className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
              spellCheck={false}
              autoFocus
            />
            <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
              <span>Supports standard markdown syntax.</span>
              <span>~{Math.ceil(draft.length / 4).toLocaleString()} tokens</span>
            </div>
          </div>
        )}

        {/* Mode 4: In-Place Live Editor */}
        {mode === "edit" && (
          <div
            className={cn(
              "flex-1 min-h-0 flex flex-col gap-2 pt-3 transition-opacity duration-500",
              isCommitting ? "opacity-30 pointer-events-none" : "opacity-100"
            )}
          >
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              className="w-full flex-1 min-h-0 bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.16)] rounded-xl p-4 text-[13px] font-mono text-[rgb(var(--foreground))] leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.5)] transition-colors custom-scrollbar"
              spellCheck={false}
              autoFocus
            />
            <div className="shrink-0 flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-1">
              <span>Saving streams updates directly to the canonical database.</span>
              <span>~{Math.ceil(draft.length / 4).toLocaleString()} tokens</span>
            </div>
          </div>
        )}

      </div>
    );
  }
);

PersonalMemoryStagingCard.displayName = "PersonalMemoryStagingCard";
