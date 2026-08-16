import { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Check, ArrowLeft, Trash2, Info, Lock } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";

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

export const SubModelCard = memo<SubModelCardProps>(({
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
          className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[12px] font-bold uppercase tracking-wider shrink-0 hover:scale-[1.02] active:scale-95 transition-all cursor-pointer shadow-md"
        >
          Get
        </button>
      );
    }

    if (isRequired) {
      return (
        <Tooltip label="Mandatory core model (cannot be deleted)">
          <div
            className="p-1.5 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))] cursor-not-allowed shrink-0 flex items-center gap-1"
          >
            <Lock size={12} className="opacity-70" />
            <span className="text-[11px] font-mono font-bold uppercase tracking-wider">Required</span>
          </div>
        </Tooltip>
      );
    }

    if (isConfirmingDelete) {
      return (
        <div className="flex items-center gap-1.5 transition-all duration-300 shrink-0">
          <span className="text-[11px] text-rose-400 font-bold uppercase tracking-wider">Delete?</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              deleteModel();
              setConfirmDeleteId(null);
            }}
            className="p-1.5 rounded-lg bg-rose-500/20 text-rose-400 hover:bg-rose-500/30 transition-colors flex items-center justify-center cursor-pointer"
            aria-label="Confirm Delete"
          >
            <Check size={14} className="font-bold" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1.5 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.1)] transition-colors flex items-center justify-center cursor-pointer"
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
        className="p-1.5 rounded-lg text-rose-400 hover:text-rose-300 hover:bg-rose-500/15 transition-colors shrink-0 cursor-pointer"
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
        "p-4 rounded-lg border transition-all duration-300 flex flex-col justify-between gap-2.5 glass-card min-h-[105px]",
        isDownloaded && !isActive && "cursor-pointer hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.02)]",
        isActive && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5 shadow-md"
      )}
    >
      <div className="space-y-0.5">
        <div className="flex items-start justify-between gap-2">
          <Tooltip label={name}>
            <span className={cn("text-[13px] font-bold text-[rgb(var(--foreground))]", layoutMode === "small" ? "" : "truncate max-w-[170px]")}>
              {name}
            </span>
          </Tooltip>

          {hasTooltip && (
            <div className="relative group inline-block shrink-0 mt-0.5">
              <Info size={16} className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-help" />
              <div className="absolute right-full top-0 mr-2 hidden group-hover:block group-hover:opacity-100 w-52 p-2.5 rounded-lg bg-[rgb(var(--card))]/95 border border-[rgba(var(--accent),0.25)] text-[12px] text-[rgb(var(--foreground-muted))] shadow-2xl leading-normal z-50 transition-opacity duration-200 pointer-events-none">
                <div className="space-y-1">
                  <div className="flex justify-between border-b border-[rgba(var(--accent),0.08)] pb-0.5 mb-1 font-bold">
                    <span className="text-[11px] text-[rgb(var(--accent))] uppercase tracking-wider">Specs</span>
                    <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">{parameters}</span>
                  </div>
                  {description && <div className="text-[11px] text-[rgb(var(--foreground))] leading-normal mb-1">{description}</div>}
                  {ramUsage && (
                    <div className="text-[11px] text-[rgb(var(--foreground-muted))] font-mono">
                      RAM: {ramUsage}
                    </div>
                  )}
                  {tradeoffs && (
                    <div className="text-[11px] text-[rgb(var(--foreground-muted))] italic border-t border-[rgba(var(--accent),0.08)] pt-1 mt-1 leading-normal">
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
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-normal">
              {description}
              {ramUsage && ` · RAM: ${ramUsage}`}
              {parameters && ` · ${parameters}`}
            </p>
          ) : (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-normal line-clamp-2">
              {description}
            </p>
          )
        )}
      </div>

      <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.08)] h-6 shrink-0">
        <span className={cn(
          "text-[12px] font-bold uppercase tracking-wider",
          isDownloaded
            ? (isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]")
            : "text-[rgb(var(--foreground-muted))]/60"
        )}>
          {isDownloaded ? (isActive ? "Active" : "Ready") : null}
        </span>
        {renderAction()}
      </div>
    </div>
  );
});

SubModelCard.displayName = "SubModelCard";
