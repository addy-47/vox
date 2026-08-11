import React, { useState } from "react";
import { ChevronUp, ChevronDown, Layers } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { COLLECTION_COLORS, RELATION_STYLES } from "@/shared/components/memory/MemoryGraph";

interface MemoryLegendCardProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  selectedRelation: string;
  onSelectRelation: (rel: string) => void;
  totalFactsCount?: number;
  totalRelationsCount?: number;
}

const COLLECTIONS_LIST = [
  { id: "Identity", label: "Identity", color: COLLECTION_COLORS.Identity.main },
  { id: "Profile", label: "Profile", color: COLLECTION_COLORS.Profile.main },
  { id: "Directives", label: "Directives", color: COLLECTION_COLORS.Directives.main },
  { id: "Narrative", label: "Narrative", color: COLLECTION_COLORS.Narrative.main },
  { id: "Entities", label: "Entities", color: COLLECTION_COLORS.Entities.main },
  { id: "Constraints", label: "Constraints", color: COLLECTION_COLORS.Constraints.main },
  { id: "Inactive", label: "Inactive / Historical", color: COLLECTION_COLORS.Inactive.main },
];

const RELATIONS_LIST = [
  { id: "SUPPORTS", label: "SUPPORTS", color: RELATION_STYLES.SUPPORTS.color, isDashed: false },
  { id: "SUPERSEDES", label: "SUPERSEDES", color: RELATION_STYLES.SUPERSEDES.color, isDashed: false },
  { id: "SHAPES", label: "SHAPES", color: RELATION_STYLES.SHAPES.color, isDashed: false },
  { id: "DEPENDS_ON", label: "DEPENDS_ON", color: RELATION_STYLES.DEPENDS_ON.color, isDashed: false },
  { id: "CONFLICTS_WITH", label: "CONFLICTS_WITH", color: RELATION_STYLES.CONFLICTS_WITH.color, isDashed: true },
  { id: "OTHER", label: "OTHER", color: RELATION_STYLES.OTHER.color, isDashed: true },
];

export const MemoryLegendCard: React.FC<MemoryLegendCardProps> = ({
  selectedCollection,
  onSelectCollection,
  selectedRelation,
  onSelectRelation,
  totalFactsCount = 0,
  totalRelationsCount = 0,
}) => {
  const [minimized, setMinimized] = useState(false);

  return (
    <div className="glass-card rounded-2xl border border-[rgba(var(--accent),0.12)] bg-[rgba(10,12,14,0.80)] backdrop-blur-xl shadow-2xl pointer-events-auto transition-all duration-200">
      {/* Minimized Pill Mode */}
      {minimized ? (
        <button
          onClick={() => setMinimized(false)}
          className="flex items-center gap-2.5 px-3.5 py-2 text-[11px] font-mono text-[rgb(var(--foreground))]/80 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
        >
          <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
          <span className="font-bold">
            {totalFactsCount.toLocaleString()} Nodes · {totalRelationsCount} Edges
          </span>
          <ChevronDown size={14} className="text-[rgb(var(--accent))]" />
        </button>
      ) : (
        /* Expanded 2-Column Mode */
        <div className="p-3.5 flex flex-col gap-3">
          {/* Header with Minimize Button */}
          <div className="flex items-center justify-between border-b border-white/[0.06] pb-2">
            <div className="flex items-center gap-2">
              <Layers size={13} className="text-[rgb(var(--accent))]" />
              <span className="text-[10px] font-mono font-bold tracking-[0.15em] uppercase text-[rgb(var(--foreground))]/90">
                COLLECTION LEGEND
              </span>
            </div>
            <button
              onClick={() => setMinimized(true)}
              className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-white/[0.05] transition-colors cursor-pointer"
              title="Minimize Legend"
            >
              <ChevronUp size={14} />
            </button>
          </div>

          <div className="flex gap-6">
            {/* Column 1: Collections */}
            <div className="flex flex-col gap-2 min-w-[125px]">
              <div className="flex items-center justify-between border-b border-white/[0.04] pb-1">
                <span className="text-[11px] font-sans font-medium text-[rgb(var(--foreground))]/85">
                  Collections
                </span>
                {selectedCollection !== "all" && (
                  <button
                    onClick={() => onSelectCollection("all")}
                    className="text-[9px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    RESET
                  </button>
                )}
              </div>

              <div className="flex flex-col gap-1">
                {COLLECTIONS_LIST.map((col) => {
                  const isSelected = selectedCollection === col.id;
                  return (
                    <button
                      key={col.id}
                      onClick={() => onSelectCollection(selectedCollection === col.id ? "all" : col.id)}
                      className={cn(
                        "flex items-center gap-2.5 px-2 py-0.5 rounded-lg text-[11px] font-sans text-left transition-all duration-150 cursor-pointer",
                        isSelected
                          ? "bg-white/[0.08] text-[rgb(var(--foreground))] font-semibold"
                          : "text-[rgb(var(--foreground-muted))]/75 hover:text-[rgb(var(--foreground))] hover:bg-white/[0.03]"
                      )}
                    >
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0 shadow-[0_0_6px_rgba(0,0,0,0.5)]"
                        style={{ backgroundColor: col.color }}
                      />
                      <span className="truncate">{col.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Column 2: Relations */}
            <div className="flex flex-col gap-2 min-w-[125px] border-l border-white/[0.06] pl-5">
              <div className="flex items-center justify-between border-b border-white/[0.04] pb-1">
                <span className="text-[11px] font-sans font-medium text-[rgb(var(--foreground))]/85">
                  Relations
                </span>
                {selectedRelation !== "all" && (
                  <button
                    onClick={() => onSelectRelation("all")}
                    className="text-[9px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    RESET
                  </button>
                )}
              </div>

              <div className="flex flex-col gap-1">
                {RELATIONS_LIST.map((rel) => {
                  const isSelected = selectedRelation === rel.id;
                  return (
                    <button
                      key={rel.id}
                      onClick={() => onSelectRelation(selectedRelation === rel.id ? "all" : rel.id)}
                      className={cn(
                        "flex items-center gap-2.5 px-2 py-0.5 rounded-lg text-[10px] font-mono text-left transition-all duration-150 cursor-pointer",
                        isSelected
                          ? "bg-white/[0.08] text-[rgb(var(--foreground))] font-semibold"
                          : "text-[rgb(var(--foreground-muted))]/75 hover:text-[rgb(var(--foreground))] hover:bg-white/[0.03]"
                      )}
                    >
                      <div className="w-4 flex items-center shrink-0">
                        {rel.isDashed ? (
                          <div
                            className="w-full border-t border-dashed"
                            style={{ borderColor: rel.color, borderWidth: "1.5px" }}
                          />
                        ) : (
                          <div
                            className="w-full h-[2px] rounded-full"
                            style={{ backgroundColor: rel.color }}
                          />
                        )}
                      </div>
                      <span className="truncate tracking-wider">{rel.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
