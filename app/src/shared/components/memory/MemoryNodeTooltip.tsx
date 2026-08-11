import React, { useState, useCallback, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Edit3, Trash2, Check, GitBranch, ArrowRight, ArrowLeft, Tag, Clock, Layers } from "lucide-react";
import {
  MemoryFactDetail,
  editFactContent,
  reassignFactCollection,
  softDeleteFact,
} from "@/services/memoryService";
import { getCollectionColor, getRelationStyle } from "@/shared/components/memory/MemoryGraph";

interface MemoryNodeTooltipProps {
  factDetail: MemoryFactDetail | null;
  isLoading: boolean;
  pos: { x: number; y: number } | null;
  onClose: () => void;
  onRefresh: () => void;
}

const CATEGORIES = [
  "Identity",
  "Profile",
  "Directives",
  "Narrative",
  "Entities",
  "Constraints",
];

function formatDate(timestamp: number): string {
  if (!timestamp) return "Unknown";
  const date = new Date(timestamp);
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export const MemoryNodeTooltip: React.FC<MemoryNodeTooltipProps> = ({
  factDetail,
  isLoading,
  pos,
  onClose,
  onRefresh,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const [isReassigning, setIsReassigning] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (factDetail) {
      setEditText(factDetail.fact || "");
      setIsEditing(false);
      setIsReassigning(false);
      setConfirmDelete(false);
    }
  }, [factDetail]);

  const handleEditSave = useCallback(async () => {
    if (!factDetail || !editText.trim()) return;
    setIsSaving(true);
    try {
      await editFactContent(factDetail.id, editText.trim());
      setIsEditing(false);
      onRefresh();
    } catch (e) {
      console.error("Edit fact content failed:", e);
    } finally {
      setIsSaving(false);
    }
  }, [factDetail, editText, onRefresh]);

  const handleCategoryReassign = useCallback(
    async (newCol: string) => {
      if (!factDetail || newCol === factDetail.collection) return;
      setIsSaving(true);
      try {
        await reassignFactCollection(factDetail.id, newCol);
        setIsReassigning(false);
        onRefresh();
      } catch (e) {
        console.error("Reassign collection failed:", e);
      } finally {
        setIsSaving(false);
      }
    },
    [factDetail, onRefresh]
  );

  const handleDelete = useCallback(async () => {
    if (!factDetail) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      setTimeout(() => setConfirmDelete(false), 3000);
      return;
    }
    try {
      await softDeleteFact(factDetail.id);
      onClose();
      onRefresh();
    } catch (e) {
      console.error("Soft delete failed:", e);
    }
  }, [factDetail, confirmDelete, onClose, onRefresh]);

  if (!pos) return null;

  const clampedX = Math.min(window.innerWidth - 340, Math.max(16, pos.x + 15));
  const clampedY = Math.min(window.innerHeight - 400, Math.max(70, pos.y - 40));

  const colPalette = factDetail
    ? getCollectionColor(factDetail.collection, factDetail.is_superseded)
    : getCollectionColor("Identity", false);

  const compactId = factDetail
    ? factDetail.id.startsWith("mem_")
      ? `MEM-${factDetail.id.split("_")[1]?.slice(0, 6) || factDetail.id.slice(4, 10)}`
      : factDetail.id
    : "MEM-LOADING";

  return (
    <AnimatePresence>
      <motion.div
        key={factDetail?.id || "loading"}
        initial={{ opacity: 0, scale: 0.92, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.92, y: 6 }}
        transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
        style={{
          left: clampedX,
          top: clampedY,
        }}
        className="fixed z-40 w-[310px] glass-card p-4 rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgba(10,12,14,0.92)] backdrop-blur-2xl shadow-2xl flex flex-col gap-3 pointer-events-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
          <div className="flex items-center gap-2">
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: colPalette.main }}
            />
            <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
              {compactId}
            </span>
            <span
              className="text-[9px] font-mono px-1.5 py-0.5 rounded font-bold uppercase"
              style={{ backgroundColor: `${colPalette.main}20`, color: colPalette.main }}
            >
              {factDetail?.collection || "COLLECTION"}
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

        {/* Loading Spinner or Content */}
        {isLoading ? (
          <div className="py-6 flex flex-col items-center justify-center gap-2 text-[rgb(var(--accent))]">
            <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
            <span className="text-[10px] font-mono tracking-widest uppercase">Lazy loading fact details...</span>
          </div>
        ) : factDetail ? (
          <>
            {/* Fact Content or Edit Textarea */}
            {isEditing ? (
              <div className="flex flex-col gap-2">
                <textarea
                  className="w-full bg-black/40 border border-[rgba(var(--accent),0.3)] rounded-xl p-2.5 text-[12px] text-[rgb(var(--foreground))] font-normal leading-relaxed outline-none resize-none"
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
                    className="px-3 py-1 rounded-lg bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/40 text-[10px] font-mono font-bold uppercase cursor-pointer"
                  >
                    {isSaving ? "Saving..." : "Save Content"}
                  </button>
                </div>
              </div>
            ) : (
              <p className="text-[12px] font-normal leading-relaxed text-[rgb(var(--foreground))]/95 italic">
                "{factDetail.fact}"
              </p>
            )}

            {/* Collection Category Reassignment Picker */}
            {isReassigning ? (
              <div className="p-2 rounded-xl bg-white/[0.03] border border-white/[0.06] flex flex-col gap-1.5">
                <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))]/70 uppercase">
                  Reassign Collection
                </span>
                <div className="grid grid-cols-2 gap-1">
                  {CATEGORIES.map((cat) => (
                    <button
                      key={cat}
                      onClick={() => handleCategoryReassign(cat)}
                      className="px-2 py-1 rounded-lg text-[10px] font-mono text-left bg-white/[0.04] hover:bg-[rgb(var(--accent))]/15 text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                    >
                      {cat}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}

            {/* Provenance Metadata Section */}
            <div className="grid grid-cols-2 gap-1.5 py-1 text-[10px] font-mono border-t border-white/[0.04]">
              <div className="flex items-center gap-1.5 text-[rgb(var(--foreground-muted))]/70">
                <Clock size={11} className="text-[rgb(var(--accent))]" />
                <span>{formatDate(factDetail.created_at)}</span>
              </div>
              <div className="flex items-center gap-1.5 text-[rgb(var(--foreground-muted))]/70">
                <Tag size={11} className="text-[rgb(var(--accent))]" />
                <span className="truncate">Session: {factDetail.session_id || "Direct"}</span>
              </div>
            </div>

            {/* Connected Incoming & Outgoing Relations */}
            <div className="flex flex-col gap-1.5 border-t border-white/[0.06] pt-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <GitBranch size={12} className="text-[rgb(var(--foreground-muted))]/60" />
                  <span className="text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/70">
                    Connected Relations
                  </span>
                </div>
                <span className="text-[10px] font-mono font-bold text-[rgb(var(--accent))]">
                  {factDetail.incoming_relations.length + factDetail.outgoing_relations.length}
                </span>
              </div>

              <div className="flex flex-col gap-1 max-h-[100px] overflow-y-auto custom-scrollbar pr-1">
                {factDetail.outgoing_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`out_${rel.id}`}
                      className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-white/[0.02] border border-white/[0.04] text-[10px] font-mono"
                    >
                      <ArrowRight size={10} className="text-[rgb(var(--foreground-muted))]/50 shrink-0" />
                      <span
                        className="font-bold uppercase text-[9px] px-1 rounded"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                      <span className="text-[rgb(var(--foreground))]/70 truncate flex-1">{rel.to_id}</span>
                    </div>
                  );
                })}

                {factDetail.incoming_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`inc_${rel.id}`}
                      className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-white/[0.02] border border-white/[0.04] text-[10px] font-mono"
                    >
                      <ArrowLeft size={10} className="text-[rgb(var(--foreground-muted))]/50 shrink-0" />
                      <span
                        className="font-bold uppercase text-[9px] px-1 rounded"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                      <span className="text-[rgb(var(--foreground))]/70 truncate flex-1">{rel.from_id}</span>
                    </div>
                  );
                })}

                {factDetail.incoming_relations.length === 0 && factDetail.outgoing_relations.length === 0 && (
                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40 italic">
                    No connected graph relations
                  </span>
                )}
              </div>
            </div>

            {/* Actions Bar */}
            <div className="flex items-center justify-between border-t border-white/[0.06] pt-2">
              <span className="text-[9px] font-mono font-bold text-[rgb(var(--foreground-muted))]/60 uppercase">
                {factDetail.is_superseded ? "STATUS: HISTORICAL" : "STATUS: ACTIVE"}
              </span>

              <div className="flex items-center gap-1.5">
                <button
                  onClick={() => setIsReassigning((v) => !v)}
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors cursor-pointer"
                  title="Reassign Collection"
                >
                  <Layers size={13} />
                </button>
                <button
                  onClick={() => setIsEditing(true)}
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] hover:bg-white/[0.04] transition-colors cursor-pointer"
                  title="Edit Fact Content"
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
                  title={confirmDelete ? "Click to confirm soft delete" : "Soft Delete Fact"}
                >
                  {confirmDelete ? <Check size={13} /> : <Trash2 size={13} />}
                </button>
              </div>
            </div>
          </>
        ) : (
          <div className="py-4 text-center text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
            Fact details unavailable
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
};
