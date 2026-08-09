import React, { useState, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Edit3, Trash2, Check } from "lucide-react";
import { MemoryFactEntry, editMemoryFact, deleteMemoryFact } from "@/services/memoryService";
import { getCollectionColor } from "@/shared/components/memory/MemoryGraph";

interface MemoryNodeTooltipProps {
  fact: MemoryFactEntry | null;
  pos: { x: number; y: number } | null;
  onClose: () => void;
  onRefresh: () => void;
}

export const MemoryNodeTooltip: React.FC<MemoryNodeTooltipProps> = ({
  fact,
  pos,
  onClose,
  onRefresh,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

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

  const colPalette = getCollectionColor(fact.collection);

  // Clamp tooltip within viewport bounds
  const clampedX = Math.min(window.innerWidth - 300, Math.max(16, pos.x + 15));
  const clampedY = Math.min(window.innerHeight - 240, Math.max(70, pos.y - 40));

  return (
    <AnimatePresence>
      <motion.div
        key={fact.id}
        initial={{ opacity: 0, scale: 0.9, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.9, y: 6 }}
        transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
        style={{
          left: clampedX,
          top: clampedY,
        }}
        className="fixed z-40 w-[270px] glass-card p-3.5 rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgba(10,12,14,0.85)] backdrop-blur-xl shadow-2xl flex flex-col gap-2.5 pointer-events-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
          <div className="flex items-center gap-1.5">
            <span
              className="w-2 h-2 rounded-full shrink-0"
              style={{ backgroundColor: colPalette.main }}
            />
            <span
              className="text-[10px] font-mono font-bold tracking-wider uppercase"
              style={{ color: colPalette.main }}
            >
              {fact.collection}
            </span>
          </div>
          <button
            onClick={onClose}
            className="text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--foreground))] transition-colors"
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
          <p className="text-[12px] font-light italic leading-relaxed text-[rgb(var(--foreground))]/90">
            "{fact.fact}"
          </p>
        )}

        {/* Footer Actions */}
        {!isEditing && (
          <div className="flex items-center justify-between border-t border-white/[0.06] pt-2">
            <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/40 uppercase">
              {fact.is_superseded ? "INACTIVE" : "ACTIVE"}
            </span>

            <div className="flex items-center gap-1.5">
              <button
                onClick={handleEditStart}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors"
                title="Edit Fact"
              >
                <Edit3 size={12} />
              </button>
              <button
                onClick={handleDelete}
                className={`p-1 rounded-lg transition-colors ${
                  confirmDelete ? "text-red-400 bg-red-500/10" : "text-[rgb(var(--foreground-muted))]/60 hover:text-red-400 hover:bg-red-500/5"
                }`}
                title={confirmDelete ? "Click to confirm delete" : "Delete Fact"}
              >
                {confirmDelete ? <Check size={12} /> : <Trash2 size={12} />}
              </button>
            </div>
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
};
