import React from "react";
import { cn } from "@/shared/lib/utils";
import { Check, ArrowLeft, Trash2, Info, Lock } from "lucide-react";

interface SubModelCardProps {
  id: string;
  name: string;
  description: string;
  parameters: string;
  ramUsage?: string;
  tradeoffs?: string;
  isDownloaded: boolean;
  isActive: boolean;
  isRequired: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
  onSelect: () => void;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  downloadStatus?: { step: string; progress: number };
  startDownload: () => void;
  deleteModel: () => void;
  showTooltip?: boolean;
}

export const SubModelCard: React.FC<SubModelCardProps> = ({
  id,
  name,
  description,
  parameters,
  ramUsage,
  tradeoffs,
  isDownloaded,
  isActive,
  isRequired,
  layoutMode,
  onSelect,
  confirmDeleteId,
  setConfirmDeleteId,
  downloadStatus,
  startDownload,
  deleteModel,
  showTooltip = false,
}) => {
  const isConfirmingDelete = confirmDeleteId === id;

  const renderAction = () => {
    if (!isDownloaded) {
      if (downloadStatus && downloadStatus.step !== "completed") {
        return (
          <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold shrink-0">
            {Math.round(downloadStatus.progress)}%
          </span>
        );
      }
      return (
        <button
          onClick={(e) => {
            e.stopPropagation();
            startDownload();
          }}
          className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[12px] font-bold uppercase tracking-wider shrink-0 hover:scale-[1.02] active:scale-95 transition-all cursor-pointer"
        >
          Get
        </button>
      );
    }

    if (isRequired) {
      return (
        <div
          title="Mandatory core model (cannot be deleted)"
          className="p-1 rounded-lg bg-white/[0.03] border border-white/[0.06] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed shrink-0 flex items-center gap-1"
        >
          <Lock size={12} className="opacity-60" />
          <span className="text-[10px] font-mono font-bold uppercase tracking-wider">Required</span>
        </div>
      );
    }

    if (isConfirmingDelete) {
      return (
        <div className="flex items-center gap-1 transition-all duration-300 shrink-0">
          <span className="text-[11px] text-red-500 font-bold uppercase tracking-wider mr-0.5">Delete?</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              deleteModel();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-red-500/20 text-red-500 hover:bg-red-500/35 transition-colors border border-red-500/30 flex items-center justify-center cursor-pointer"
            aria-label="Confirm Delete"
          >
            <Check size={14} className="font-bold" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--foreground))]/[0.05] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.08] transition-colors border border-[rgba(var(--border),0.1)] flex items-center justify-center cursor-pointer"
            aria-label="Cancel"
          >
            <ArrowLeft size={14} />
          </button>
        </div>
      );
    }

    return (
      <button
        onClick={(e) => {
          e.stopPropagation();
          setConfirmDeleteId(id);
        }}
        className="p-1.5 rounded-lg bg-red-500/10 text-red-500 border border-red-500/20 hover:bg-red-500/20 hover:border-red-500/30 transition-colors shrink-0 cursor-pointer"
        aria-label="Delete weights"
      >
        <Trash2 size={16} />
      </button>
    );
  };

  const hasTooltip = showTooltip && !!(description || parameters || ramUsage || tradeoffs);

  return (
    <div
      onClick={() => {
        if (isDownloaded && !isActive) {
          onSelect();
        }
      }}
      className={cn(
        "p-4 rounded-lg border transition-all duration-300 flex flex-col justify-between gap-2.5 glass min-h-[105px]",
        isDownloaded && !isActive && "cursor-pointer hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.02)]",
        isActive && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
      )}
    >
      <div className="space-y-0.5">
        <div className="flex items-start justify-between gap-2">
          <span className={cn("text-[13px] font-bold text-[rgb(var(--foreground))]", layoutMode === "small" ? "" : "truncate max-w-[170px]")} title={name}>
            {name}
          </span>
          
          {hasTooltip && (
            <div className="relative group inline-block shrink-0 mt-0.5">
              <Info size={16} className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-help" />
              <div className="absolute right-full top-0 mr-2 hidden group-hover:block group-hover:opacity-100 w-52 p-2.5 rounded-lg bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.25)] text-[12px] text-[rgb(var(--foreground-muted))]/80 shadow-2xl leading-normal z-50 transition-opacity duration-200 pointer-events-none">
                <div className="space-y-1">
                  <div className="flex justify-between border-b border-[rgba(var(--accent),0.06)] pb-0.5 mb-1 font-bold">
                    <span className="text-[10px] text-[rgb(var(--accent))] uppercase tracking-wider">Specs</span>
                    <span className="font-mono text-[10px] text-[rgb(var(--foreground-muted))]/60">{parameters}</span>
                  </div>
                  {description && <div className="text-[11px] text-[rgb(var(--foreground))]/80 leading-normal mb-1">{description}</div>}
                  {ramUsage && (
                    <div className="text-[10px] text-[rgb(var(--foreground-muted))]/70 font-mono">
                      RAM: {ramUsage}
                    </div>
                  )}
                  {tradeoffs && (
                    <div className="text-[10px] text-[rgb(var(--foreground-muted))]/70 italic border-t border-[rgba(var(--accent),0.06)] pt-1 mt-1 leading-normal">
                      {tradeoffs}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {description && (
          !showTooltip ? (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))]/70 leading-normal">
              {description}
              {ramUsage && ` · RAM: ${ramUsage}`}
              {parameters && ` · ${parameters}`}
            </p>
          ) : (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))]/70 leading-normal line-clamp-2">
              {description}
            </p>
          )
        )}
      </div>

      <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.05)] h-6 shrink-0">
        <span className={cn(
          "text-[12px] font-bold uppercase tracking-wider",
          isDownloaded
            ? (isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/70")
            : "text-[rgb(var(--foreground-muted))]/40"
        )}>
          {isDownloaded ? (isActive ? "Active" : "Ready") : null}
        </span>
        {renderAction()}
      </div>
    </div>
  );
};
