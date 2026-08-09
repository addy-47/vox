import React, { useState, useCallback, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Edit3, Trash2, Check, GitBranch, ArrowRight, ArrowLeft } from "lucide-react";
import { MemoryFactEntry, MemoryRelationEntry, editMemoryFact, deleteMemoryFact } from "@/services/memoryService";
import { getCollectionColor, getRelationStyle } from "@/shared/components/memory/MemoryGraph";

interface MemoryNodeTooltipProps {
  fact: MemoryFactEntry | null;
  allFacts: MemoryFactEntry[];
  allRelations: MemoryRelationEntry[];
  pos: { x: number; y: number } | null;
  onClose: () => void;
  onRefresh: () => void;
}

export const MemoryNodeTooltip: React.FC<MemoryNodeTooltipProps> = ({
  fact,
  allFacts,
  allRelations,
  pos,
  onClose,
  onRefresh,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Map of connected edges for this fact
  const connectedEdges = useMemo(() => {
    if (!fact) return [];
    const factMap = new Map(allFacts.map((f) => [f.id, f]));

    return allRelations
      .filter((r) => r.from_id === fact.id || r.to_id === fact.id)
      .map((r) => {
        const isOutgoing = r.from_id === fact.id;
        const otherId = isOutgoing ? r.to_id : r.from_id;
        const otherFact = factMap.get(otherId);
        const relStyle = getRelationStyle(r.relation);

        return {
          id: r.id,
          relation: r.relation,
          isOutgoing,
          otherFactText: otherFact?.fact || otherId,
          color: relStyle.color,
          isDashed: relStyle.isDashed,
        };
      });
  }, [fact, allFacts, allRelations]);

  const handleEditStart = useCallback(() => {
    if (!fact) return;
    setEditText(fact.fact);
    setIsEditing(true);
  }, [fact]);

  const handleEditSave = useCallback(async () => {
    if (!fact || !editText.trim()) return;
    setIsSaving(true);
    try {
      await editMemoryFact(fact.id, editText.trim(), fact.collection);
      setIsEditing(false);
      onRefresh();
    } catch (e) {
      console.error("Edit failed:", e);
    } finally {
      setIsSaving(false);
    }
  }, [fact, editText, onRefresh]);

  const handleDelete = useCallback(async () => {
    if (!fact) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      setTimeout(() => setConfirmDelete(false), 3000);
      return;
    }
    try {
      await deleteMemoryFact(fact.id);
      onClose();
      onRefresh();
    } catch (e) {
      console.error("Delete failed:", e);
    }
  }, [fact, confirmDelete, onClose, onRefresh]);

  if (!fact || !pos) return null;

  const colPalette = getCollectionColor(fact.collection, fact.is_superseded);

  const clampedX = Math.min(window.innerWidth - 320, Math.max(16, pos.x + 15));
  const clampedY = Math.min(window.innerHeight - 340, Math.max(70, pos.y - 40));

  return (
    <AnimatePresence>
      <motion.div
        key={fact.id}
        initial={{ opacity: 0, scale: 0.92, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.92, y: 6 }}
        transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
        style={{
          left: clampedX,
          top: clampedY,
        }}
        className="fixed z-40 w-[290px] glass-card p-3.5 rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgba(10,12,14,0.90)] backdrop-blur-2xl shadow-2xl flex flex-col gap-3 pointer-events-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
          <div className="flex items-center gap-2">
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: colPalette.main }}
            />
            <span
              className="text-[11px] font-sans font-semibold uppercase tracking-wider"
              style={{ color: colPalette.main }}
            >
              {fact.collection}
            </span>
          </div>
          <button
            onClick={onClose}
            className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
            aria-label="Close tooltip"
          >
            <X size={13} />
          </button>
        </div>

        {/* Fact Text or Edit Area */}
        {isEditing ? (
          <div className="flex flex-col gap-2">
            <textarea
              className="w-full bg-black/40 border border-[rgba(var(--accent),0.3)] rounded-xl p-2 text-[12px] text-[rgb(var(--foreground))] font-light leading-relaxed outline-none resize-none"
              rows={3}
              value={editText}
              onChange={(e) => setEditText(e.target.value)}
              autoFocus
            />
            <div className="flex gap-2 justify-end">
              <button
                onClick={() => setIsEditing(false)}
                className="px-2.5 py-1 rounded-lg text-[10px] font-mono text-[rgb(var(--foreground-muted))]"
              >
                Cancel
              </button>
              <button
                onClick={handleEditSave}
                disabled={isSaving}
                className="px-3 py-1 rounded-lg bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/40 text-[10px] font-mono font-bold uppercase"
              >
                {isSaving ? "Saving..." : "Save"}
              </button>
            </div>
          </div>
        ) : (
          <p className="text-[13px] font-light leading-relaxed text-[rgb(var(--foreground))]/90 italic">
            "{fact.fact}"
          </p>
        )}

        {/* Connected Edges & Relations Details */}
        <div className="flex flex-col gap-1.5 border-t border-white/[0.06] pt-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <GitBranch size={12} className="text-[rgb(var(--foreground-muted))]/60" />
              <span className="text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/70">
                Connected Edges
              </span>
            </div>
            <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
              {connectedEdges.length}
            </span>
          </div>

          {connectedEdges.length > 0 ? (
            <div className="flex flex-col gap-1.5 max-h-[100px] overflow-y-auto custom-scrollbar pr-1">
              {connectedEdges.map((edge) => (
                <div
                  key={edge.id}
                  className="flex items-center gap-2 p-1.5 rounded-lg bg-white/[0.02] border border-white/[0.04] text-[10px]"
                >
                  <div className="flex items-center gap-1 shrink-0">
                    {edge.isOutgoing ? (
                      <ArrowRight size={10} className="text-[rgb(var(--foreground-muted))]/40" />
                    ) : (
                      <ArrowLeft size={10} className="text-[rgb(var(--foreground-muted))]/40" />
                    )}
                    <span
                      className="font-mono font-bold tracking-wider uppercase px-1 py-0.5 rounded text-[9px]"
                      style={{ color: edge.color, backgroundColor: `${edge.color}15` }}
                    >
                      {edge.relation}
                    </span>
                  </div>
                  <span className="text-[11px] font-light text-[rgb(var(--foreground))]/70 truncate flex-1">
                    {edge.otherFactText}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40 italic">
              No direct semantic relations
            </span>
          )}
        </div>

        {/* Footer Actions */}
        {!isEditing && (
          <div className="flex items-center justify-between border-t border-white/[0.06] pt-2">
            <span className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))]/50 uppercase">
              {fact.is_superseded ? "STATUS: INACTIVE" : "STATUS: ACTIVE"}
            </span>

            <div className="flex items-center gap-1.5">
              <button
                onClick={handleEditStart}
                className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors cursor-pointer"
                title="Edit Fact"
              >
                <Edit3 size={13} />
              </button>
              <button
                onClick={handleDelete}
                className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                  confirmDelete
                    ? "text-red-400 bg-red-500/10"
                    : "text-[rgb(var(--foreground-muted))]/70 hover:text-red-400 hover:bg-red-500/5"
                }`}
                title={confirmDelete ? "Click to confirm delete" : "Delete Fact"}
              >
                {confirmDelete ? <Check size={13} /> : <Trash2 size={13} />}
              </button>
            </div>
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
};
