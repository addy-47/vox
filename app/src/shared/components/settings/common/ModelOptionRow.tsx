import React, { memo } from "react";
import { Download, CheckCircle2, HardDrive, Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface ModelOptionProps {
  id: string;
  name: string;
  sizeGb?: string;
  vramGb?: string;
  quant?: string;
  isDownloaded?: boolean;
  isDownloading?: boolean;
  downloadProgress?: number;
  isSelected?: boolean;
  onSelect?: () => void;
  onDownload?: () => void;
  className?: string;
}

export const ModelOptionRow: React.FC<ModelOptionProps> = memo(
  ({
    name,
    sizeGb,
    vramGb,
    quant,
    isDownloaded = false,
    isDownloading = false,
    downloadProgress = 0,
    isSelected = false,
    onSelect,
    onDownload,
    className,
  }) => {
    return (
      <div
        onClick={onSelect}
        className={cn(
          "flex items-center justify-between p-3.5 rounded-xl border transition-all duration-300 cursor-pointer group",
          isSelected
            ? "bg-[rgba(var(--accent),0.08)] border-[rgba(var(--accent),0.4)]"
            : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--border),0.1)] hover:bg-[rgba(var(--foreground),0.04)]",
          className
        )}
      >
        <div className="flex items-center gap-3 min-w-0">
          <div
            className={cn(
              "w-8 h-8 rounded-lg flex items-center justify-center shrink-0 border transition-colors",
              isSelected
                ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.3)] text-[rgb(var(--accent))]"
                : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]"
            )}
          >
            <Cpu size={16} />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h5 className="text-xs font-bold text-[rgb(var(--foreground))] truncate">{name}</h5>
              {quant && (
                <span className="px-1.5 py-0.5 rounded text-[9px] font-bold font-mono tracking-wider uppercase bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]">
                  {quant}
                </span>
              )}
            </div>
            <div className="flex items-center gap-3 text-[10px] text-[rgb(var(--foreground-muted))]/70 mt-0.5">
              {sizeGb && (
                <span className="flex items-center gap-1">
                  <HardDrive size={10} />
                  {sizeGb}
                </span>
              )}
              {vramGb && <span>VRAM: {vramGb}</span>}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0 ml-3">
          {isDownloading ? (
            <div className="flex items-center gap-2">
              <div className="w-16 h-1.5 rounded-full bg-[rgba(var(--foreground),0.08)] overflow-hidden">
                <div
                  className="h-full bg-[rgb(var(--accent))] transition-all duration-300"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
              <span className="text-[10px] font-mono text-[rgb(var(--accent))]">{downloadProgress}%</span>
            </div>
          ) : isDownloaded ? (
            <span className="flex items-center gap-1 text-[10px] font-semibold text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 px-2 py-1 rounded-md">
              <CheckCircle2 size={12} />
              Ready
            </span>
          ) : onDownload ? (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onDownload();
              }}
              className="flex items-center gap-1.5 text-[11px] font-semibold text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.1)] hover:bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.3)] px-2.5 py-1 rounded-lg transition-colors"
            >
              <Download size={12} />
              Download
            </button>
          ) : null}
        </div>
      </div>
    );
  }
);

ModelOptionRow.displayName = "ModelOptionRow";
