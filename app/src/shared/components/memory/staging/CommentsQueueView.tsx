import React, { useState, memo } from "react";
import { MessageSquare, Edit3, Trash2 } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { MemoryComment } from "../stagingTypes";

export interface CommentsQueueViewProps {
  comments: MemoryComment[];
  onDeleteComment?: (id: string) => void;
  onUpdateComment?: (id: string, text: string) => void;
  onClearComments?: () => void;
}

export const CommentsQueueView: React.FC<CommentsQueueViewProps> = memo(
  ({ comments, onDeleteComment, onUpdateComment, onClearComments }) => {
    const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
    const [editingCommentText, setEditingCommentText] = useState("");

    return (
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
                    <span className="px-2 py-0.5 rounded-md text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
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
    );
  }
);

CommentsQueueView.displayName = "CommentsQueueView";
