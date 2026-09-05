import { useState, useCallback, useEffect, useRef, memo } from "react";
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
import { Tooltip } from "@/shared/ui/Tooltip";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

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

export const MemoryNodeTooltip = memo(({
  factDetail,
  isLoading,
  pos,
  onClose,
  onRefresh,
}: MemoryNodeTooltipProps) => {
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [editText, setEditText] = useState("");
  const [isReassigning, setIsReassigning] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Escape + outside-click dismissal via the global overlay stack.
  useOverlay({ onClose, ref: tooltipRef, dismissOnOutside: true });

  const [, setResizeTick] = useState(0);
  useEffect(() => {
    const handleResize = () => setResizeTick((t) => (t + 1) % 1000);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

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

  const isMobile = typeof window !== "undefined" ? window.innerWidth < 640 : false;
  const tooltipWidth = typeof window !== "undefined" ? Math.min(380, window.innerWidth - 32) : 380;
  const clampedX = isMobile
    ? (typeof window !== "undefined" ? (window.innerWidth - tooltipWidth) / 2 : 16)
    : Math.min(window.innerWidth - tooltipWidth - 16, Math.max(16, pos.x + 15));
  const clampedY = isMobile
    ? (typeof window !== "undefined" ? Math.min(window.innerHeight - 380, Math.max(80, pos.y - 20)) : 80)
    : Math.min(window.innerHeight - 460, Math.max(70, pos.y - 40));

  const colPalette = factDetail
    ? getCollectionColor(factDetail.collection, factDetail.is_superseded)
    : getCollectionColor("Identity", false);

  const compactId = factDetail
    ? factDetail.id.startsWith("mem_")
      ? `mem_${factDetail.id.split("_")[1]?.slice(0, 8) || factDetail.id.slice(4, 12)}`
      : factDetail.id
    : "mem_fact";

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
        ref={tooltipRef}
        key={factDetail?.id || "loading"}
        initial={{ opacity: 0, scale: 0.94, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.94, y: 6 }}
        transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
        style={
          isMobile
            ? {
                left: "16px",
                right: "16px",
                bottom: "76px",
                maxWidth: "380px",
                margin: "0 auto",
              }
            : {
                left: clampedX,
                top: clampedY,
                width: `${tooltipWidth}px`,
              }
        }
        className={cn(
          "fixed z-40 p-4 sm:p-5 rounded-3xl border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--card))]/98 backdrop-blur-2xl shadow-2xl flex flex-col gap-3.5 pointer-events-auto text-[rgb(var(--foreground))]",
          isMobile
            ? "max-h-[calc(100vh-150px)] overflow-y-auto custom-scrollbar"
            : "max-h-[520px] overflow-y-auto custom-scrollbar"
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header — Clean single-line entity title */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: colPalette.main }}
            />
            <span className="text-[12px] font-sans font-black uppercase tracking-wider text-[rgb(var(--foreground))]">
              {factDetail?.collection || "Identity"}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              • {compactId}
            </span>
          </div>

          <button
            onClick={onClose}
            className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer p-1 rounded-full hover:bg-black/10 dark:hover:bg-white/10"
            aria-label={MEMORY_COPY.closeTooltip}
          >
            <X size={15} />
          </button>
        </div>

        {/* Loading Spinner or Content */}
        {isLoading ? (
          <div className="py-6 flex flex-col items-center justify-center gap-2 text-[rgb(var(--accent))]">
            <div className="w-5 h-5 border-2 border-current border-t-transparent rounded-full animate-spin" />
            <span className="text-[11px] tracking-widest uppercase">{MEMORY_COPY.loadingDetail}</span>
          </div>
        ) : factDetail ? (
          <>
            {/* Fact Content Quote */}
            {isEditing ? (
              <div className="flex flex-col gap-2">
                <textarea
                  className="w-full bg-[rgb(var(--background))]/80 border border-[rgba(var(--accent),0.35)] rounded-2xl p-3 text-[12px] text-[rgb(var(--foreground))] font-sans leading-relaxed outline-none resize-none shadow-inner"
                  rows={3}
                  value={editText}
                  onChange={(e) => setEditText(e.target.value)}
                  autoFocus
                />
                <div className="flex gap-2 justify-end">
                  <button
                    onClick={() => setIsEditing(false)}
                    className="px-3 py-1 rounded-xl text-[11px] font-sans text-[rgb(var(--foreground-muted))] hover:bg-white/5 cursor-pointer"
                  >
                    {MEMORY_COPY.cancel}
                  </button>
                  <button
                    onClick={handleEditSave}
                    disabled={isSaving}
                    className="px-3.5 py-1 rounded-xl bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/40 text-[11px] font-sans font-bold uppercase cursor-pointer hover:bg-[rgb(var(--accent))]/30 transition-colors shadow-sm"
                  >
                    {isSaving ? MEMORY_COPY.saving : MEMORY_COPY.save}
                  </button>
                </div>
              </div>
            ) : (
              <div className="relative pl-3 border-l-2 border-[rgb(var(--accent))]/40 py-1">
                <p className="text-[12px] font-sans leading-relaxed text-[rgb(var(--foreground))]">
                  "{factDetail.fact}"
                </p>
                <div className="flex items-center justify-between text-[11px] font-mono text-[rgb(var(--foreground-muted))] mt-1.5">
                  <span>{formatDate(factDetail.created_at)}</span>
                  <span className="font-bold text-[rgb(var(--accent))]">
                    {factDetail.is_superseded ? MEMORY_COPY.outdated : MEMORY_COPY.activeState}
                  </span>
                </div>
              </div>
            )}

            {/* Category Reassignment Picker */}
            {isReassigning && (
              <div className="pt-2 flex flex-col gap-2">
                <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-wider">
                  {MEMORY_COPY.changeCategory}
                </span>
                <div className="grid grid-cols-2 gap-1.5">
                  {CATEGORIES.map((cat) => (
                    <button
                      key={cat}
                      onClick={() => handleCategoryReassign(cat)}
                      className="px-2.5 py-1.5 rounded-xl text-[11px] font-sans font-medium text-left bg-black/[0.03] dark:bg-white/[0.04] hover:bg-[rgb(var(--accent))]/15 text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                    >
                      {cat}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* Relation Summary Inline Stats Row */}
            <div className="flex items-center justify-between text-[11px] font-sans pt-1">
              <span className="text-[11px] font-bold uppercase text-[rgb(var(--foreground-muted))]">
                {MEMORY_COPY.connections}
              </span>
              <div className="flex items-center gap-3 font-mono text-[11px]">
                <span className="text-emerald-500 dark:text-emerald-400 font-bold">
                  {supportsCount} <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">{MEMORY_COPY.supports}</span>
                </span>
                <span className="text-amber-500 dark:text-amber-400 font-bold">
                  {dependsCount} <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">{MEMORY_COPY.depends}</span>
                </span>
                <span className="text-red-500 dark:text-red-400 font-bold">
                  {conflictsCount} <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">{MEMORY_COPY.conflicts}</span>
                </span>
              </div>
            </div>

            {/* Connected Relations List */}
            {(factDetail.outgoing_relations.length > 0 || factDetail.incoming_relations.length > 0) && (
              <div className="flex flex-col gap-1 max-h-[100px] overflow-y-auto custom-scrollbar pt-1">
                {factDetail.outgoing_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`out_${rel.id}`}
                      className="flex items-center justify-between text-[11px] font-mono py-0.5"
                    >
                      <div className="flex items-center gap-1.5 overflow-hidden">
                        <ArrowRight size={11} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                        <span className="truncate text-[rgb(var(--foreground))]">{rel.to_id}</span>
                      </div>
                      <span
                        className="text-[11px] font-bold uppercase px-2 py-0.5 rounded-full shrink-0"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                    </div>
                  );
                })}

                {factDetail.incoming_relations.map((rel) => {
                  const relStyle = getRelationStyle(rel.relation);
                  return (
                    <div
                      key={`inc_${rel.id}`}
                      className="flex items-center justify-between text-[11px] font-mono py-0.5"
                    >
                      <div className="flex items-center gap-1.5 overflow-hidden">
                        <ArrowLeft size={11} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                        <span className="truncate text-[rgb(var(--foreground))]">{rel.from_id}</span>
                      </div>
                      <span
                        className="text-[11px] font-bold uppercase px-2 py-0.5 rounded-full shrink-0"
                        style={{ color: relStyle.color, backgroundColor: `${relStyle.color}15` }}
                      >
                        {rel.relation}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}

            {/* Actions Bar */}
            <div className="flex items-center justify-between pt-2">
              <div className="flex items-center gap-1">
                <Tooltip label={MEMORY_COPY.changeCategory}>
                <button
                  onClick={() => setIsReassigning((v) => !v)}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
                >
                  <Layers size={14} />
                </button>
              </Tooltip>
              <Tooltip label={MEMORY_COPY.editMemory}>
                <button
                  onClick={() => setIsEditing(true)}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
                >
                  <Edit3 size={14} />
                </button>
              </Tooltip>
              <Tooltip label={confirmDelete ? MEMORY_COPY.confirmDeleteHint : MEMORY_COPY.deleteMemory}>
                <button
                  onClick={handleDelete}
                  className={`p-1.5 rounded-xl transition-colors cursor-pointer ${
                    confirmDelete
                      ? "text-red-400 bg-red-500/15"
                      : "text-[rgb(var(--foreground-muted))] hover:text-red-400 hover:bg-red-500/10"
                  }`}
                >
                  {confirmDelete ? <Check size={14} /> : <Trash2 size={14} />}
                </button>
              </Tooltip>
              </div>
            </div>
          </>
        ) : (
          <div className="py-3 text-center text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
            {MEMORY_COPY.detailsUnavailable}
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
});

MemoryNodeTooltip.displayName = "MemoryNodeTooltip";
