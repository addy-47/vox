import React, { useState, useCallback, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  X,
  Edit3,
  Trash2,
  Check,
  ArrowRight,
  ArrowLeft,
  Layers,
} from "lucide-react";
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

  const clampedX = Math.min(window.innerWidth - 360, Math.max(16, pos.x + 15));
  const clampedY = Math.min(window.innerHeight - 440, Math.max(70, pos.y - 40));

  const colPalette = factDetail
    ? getCollectionColor(factDetail.collection, factDetail.is_superseded)
    : getCollectionColor("Identity", false);

  const compactId = factDetail
    ? factDetail.id.startsWith("mem_")
      ? `mem_fact_${factDetail.id.split("_")[1]?.slice(0, 6) || factDetail.id.slice(4, 10)}`
      : factDetail.id
    : "mem_fact_loading";

  // Subpanel 3 relation count summary calculations
  const supportsCount = factDetail
    ? factDetail.outgoing_relations.filter((r) => r.relation.toUpperCase().includes("SUPPORT")).length +
      factDetail.incoming_relations.filter((r) => r.relation.toUpperCase().includes("SUPPORT")).length
    : 0;

  const dependsCount = factDetail
    ? factDetail.outgoing_relations.filter((r) => r.relation.toUpperCase().includes("DEPEND")).length +
      factDetail.incoming_relations.filter((r) => r.relation.toUpperCase().includes("DEPEND")).length
    : 0;

  const conflictsCount = factDetail
    ? factDetail.outgoing_relations.filter((r) => r.relation.toUpperCase().includes("CONFLICT")).length +
      factDetail.incoming_relations.filter((r) => r.relation.toUpperCase().includes("CONFLICT")).length
    : 0;

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
        className="fixed z-40 w-[330px] glass-card p-4 rounded-2xl border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl flex flex-col gap-3 pointer-events-auto text-[rgb(var(--foreground))]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Subpanel 3 Header */}
        <div className="flex items-center justify-between border-b border-[rgba(var(--border),0.15)] pb-2.5">
          <div className="flex items-center gap-2">
            <span
              className="text-[10px] font-mono px-2 py-0.5 rounded-full font-bold uppercase tracking-wide"
              style={{ backgroundColor: `${colPalette.main}20`, color: colPalette.main }}
            >
              {factDetail?.collection || "COLLECTION"}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[9px] font-mono font-bold px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
              {factDetail?.is_superseded ? "HISTORICAL" : "Active Fact"}
            </span>
            <button
              onClick={onClose}
              className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              aria-label="Close tooltip"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* Loading Spinner or Content */}
        {isLoading ? (
          <div className="py-8 flex flex-col items-center justify-center gap-2 text-[rgb(var(--accent))]">
            <div className="w-5 h-5 border-2 border-current border-t-transparent rounded-full animate-spin" />
            <span className="text-[10px] font-mono tracking-widest uppercase">Loading fact details...</span>
          </div>
        ) : factDetail ? (
          <>
            {/* Metadata Grid (Subpanel 3) */}
            <div className="grid grid-cols-2 gap-2 text-[10px] font-mono p-2.5 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
              <div>
                <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[9px]">Fact ID</span>
                <span className="font-bold text-[rgb(var(--foreground))] truncate block">{compactId}</span>
              </div>
              <div>
                <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[9px]">Confidence</span>
                <span className="font-bold text-[rgb(var(--accent))]">0.98</span>
              </div>
              <div>
                <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[9px]">Created</span>
                <span className="text-[rgb(var(--foreground))] block">{formatDate(factDetail.created_at)}</span>
              </div>
              <div>
                <span className="text-[rgb(var(--foreground-muted))] block uppercase text-[9px]">Source</span>
                <span className="text-[rgb(var(--foreground))] block">compaction</span>
              </div>
            </div>

            {/* Fact Content Quote Box */}
            {isEditing ? (
              <div className="flex flex-col gap-2">
                <textarea
                  className="w-full bg-[rgb(var(--background))] border border-[rgba(var(--accent),0.3)] rounded-xl p-2.5 text-[12px] text-[rgb(var(--foreground))] font-mono leading-relaxed outline-none resize-none"
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
              <p className="text-[11px] font-sans font-normal leading-relaxed text-[rgb(var(--foreground))] italic p-3 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)]">
                "{factDetail.fact}"
              </p>
            )}

            {/* Category Reassignment Picker */}
            {isReassigning ? (
              <div className="p-2.5 rounded-xl bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.15)] flex flex-col gap-1.5">
                <span className="text-[10px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
                  Reassign Collection
                </span>
                <div className="grid grid-cols-2 gap-1">
                  {CATEGORIES.map((cat) => (
                    <button
                      key={cat}
                      onClick={() => handleCategoryReassign(cat)}
                      className="px-2 py-1 rounded-lg text-[10px] font-mono text-left bg-[rgb(var(--foreground))]/5 hover:bg-[rgb(var(--accent))]/15 text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                    >
                      {cat}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}

            {/* Subpanel 3: Relation Summary Count Pills */}
            <div className="flex flex-col gap-2 pt-1 border-t border-[rgba(var(--border),0.15)]">
              <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                Relation Summary
              </span>
              <div className="grid grid-cols-3 gap-1.5 text-[9px] font-mono text-center">
                <div className="p-1.5 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 font-bold">
                  <span>{supportsCount}</span>
                  <span className="block text-[8px] opacity-80">SUPPORTS</span>
                </div>
                <div className="p-1.5 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 font-bold">
                  <span>{dependsCount}</span>
                  <span className="block text-[8px] opacity-80">DEPENDS_ON</span>
                </div>
                <div className="p-1.5 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 font-bold">
                  <span>{conflictsCount}</span>
                  <span className="block text-[8px] opacity-80">CONFLICTS</span>
                </div>
              </div>
            </div>

            {/* Connected Relations List */}
            <div className="flex flex-col gap-1.5">
              <div className="flex flex-col gap-1 max-h-[110px] overflow-y-auto custom-scrollbar pr-1">
                {factDetail.outgoing_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`out_${rel.id}`}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)] text-[10px] font-mono"
                    >
                      <ArrowRight size={10} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                      <span
                        className="font-bold uppercase text-[8px] px-1 rounded shrink-0"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                      <span className="text-[rgb(var(--foreground))] truncate flex-1">{rel.to_id}</span>
                    </div>
                  );
                })}

                {factDetail.incoming_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`inc_${rel.id}`}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.12)] text-[10px] font-mono"
                    >
                      <ArrowLeft size={10} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                      <span
                        className="font-bold uppercase text-[8px] px-1 rounded shrink-0"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                      <span className="text-[rgb(var(--foreground))] truncate flex-1">{rel.from_id}</span>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Actions Bar */}
            <div className="flex items-center justify-between border-t border-[rgba(var(--border),0.15)] pt-2.5">
              <div className="flex items-center gap-1.5">
                <button
                  onClick={() => setIsReassigning((v) => !v)}
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/10 transition-colors cursor-pointer"
                  title="Reassign Collection"
                >
                  <Layers size={13} />
                </button>
                <button
                  onClick={() => setIsEditing(true)}
                  className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/10 transition-colors cursor-pointer"
                  title="Edit Fact Content"
                >
                  <Edit3 size={13} />
                </button>
                <button
                  onClick={handleDelete}
                  className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                    confirmDelete
                      ? "text-red-400 bg-red-500/10"
                      : "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-500/10"
                  }`}
                  title={confirmDelete ? "Click to confirm soft delete" : "Soft Delete Fact"}
                >
                  {confirmDelete ? <Check size={13} /> : <Trash2 size={13} />}
                </button>
              </div>
            </div>
          </>
        ) : (
          <div className="py-4 text-center text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            Fact details unavailable
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
};
