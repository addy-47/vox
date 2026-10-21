import { memo } from "react";
import { Check, ArrowLeft, Trash2, Info, Download } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { MODEL_HUB_COPY, SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { ModelLogo } from "./ModelLogo";
import { CatalogRowCard } from "../shared/CatalogRowCard";

export interface SpeechModelCardProps {
  id: string;
  name: string;
  description: string;
  parameters: string;
  ramUsage?: string;
  tradeoffs?: string;
  isDownloaded: boolean;
  isActive: boolean;
  isRequired?: boolean;
  /** Model shipped inside the app binary (no weights on disk). */
  isBuiltIn?: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
  onSelect: () => void;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  downloadStatus?: { step: string; progress: number };
  startDownload: () => void;
  deleteModel: () => void;
  /** Hides the specs tooltip for callers that render their own detail surface. */
  showTooltip?: boolean;
  /** Overrides the brand mark; defaults to the model-family mark. */
  logo?: React.ReactNode;
  /** Tier caption beside the specs: On-device, Cloud, Built-in. */
  badge?: string;
  /** Extra action at the trailing edge, left of the status (e.g. open voices). */
  cornerAction?: React.ReactNode;
}

/**
 * A model row: mark, name, tier + footprint, one status glyph. Built for
 * scanning a picker, not for reading an essay — the specs sit behind the info
 * glyph and weight management behind the trash glyph, both on hover.
 *
 * Selection never changes geometry: the same 1px border and the same box. Only
 * the colour and a faint wash move, so the chosen row cannot read larger than
 * its neighbours.
 */
export const SpeechModelCard = memo<SpeechModelCardProps>(({
  id,
  name,
  description,
  parameters,
  ramUsage,
  tradeoffs,
  isDownloaded,
  isActive,
  isRequired = false,
  isBuiltIn = false,
  onSelect,
  confirmDeleteId,
  setConfirmDeleteId,
  downloadStatus,
  startDownload,
  deleteModel,
  showTooltip = true,
  logo,
  badge,
  cornerAction,
}) => {
  const isConfirmingDelete = confirmDeleteId === id;
  const downloading = !isDownloaded && !!downloadStatus && downloadStatus.step !== "completed";

  const specsSheet = (
    <div className="space-y-1.5 text-left">
      <div className="flex items-baseline justify-between gap-3 border-b border-[rgba(var(--foreground),0.08)] pb-1 mb-1">
        <span className="text-[10px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--accent))]">
          {MODEL_HUB_COPY.row.specs}
        </span>
        <span className="font-mono text-[10.5px] text-[rgb(var(--foreground-muted))]">
          {parameters}
        </span>
      </div>
      {description && (
        <p className="text-[11px] leading-relaxed text-[rgb(var(--foreground))]">{description}</p>
      )}
      {ramUsage && (
        <p className="font-mono text-[10.5px] text-[rgb(var(--foreground-muted))]">
          {MODEL_HUB_COPY.row.ramLabel} {ramUsage}
        </p>
      )}
      {tradeoffs && (
        <p className="border-t border-[rgba(var(--foreground),0.08)] pt-1.5 text-[10.5px] italic leading-relaxed text-[rgb(var(--foreground-muted))]">
          {tradeoffs}
        </p>
      )}
    </div>
  );

  const hasTooltip = showTooltip && !!(description || parameters || ramUsage || tradeoffs);

  const tierLabel =
    badge ?? (isBuiltIn ? MODEL_HUB_COPY.row.builtIn : SPEECH_CARD_COPY.tierOnDevice);

  // "(Cloud)" and "(Local)" duplicate the tier already shown first, and a zero
  // footprint is noise on cloud engines.
  const footprint = ramUsage
    ?.replace(/\s*\([^)]*\)\s*/g, " ")
    .trim()
    .replace(/^0\s*(MB|GB)?$/i, "");
  const visibleFootprint = footprint && footprint !== "0" ? footprint : "";
  const meta = visibleFootprint ? `${tierLabel} · ${visibleFootprint}` : tierLabel;

  const canDelete = isDownloaded && !isBuiltIn && !isRequired;

  return (
    <CatalogRowCard
      leading={logo ?? <ModelLogo modelId={id} size={36} />}
      title={name}
      meta={meta}
      active={isActive}
      disabled={!isDownloaded}
      onSelect={onSelect}
      ariaLabel={name}
      trailing={
        <>
        {!isDownloaded &&
          (downloading ? (
            <span className="px-1 font-mono text-[10.5px] font-bold text-[rgb(var(--accent))]">
              {Math.round(downloadStatus!.progress)}%
            </span>
          ) : (
            <Tooltip label={MODEL_HUB_COPY.row.downloadModel} side="left" asChild>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  startDownload();
                }}
                className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
                aria-label={MODEL_HUB_COPY.row.downloadModel}
              >
                <Download size={14} strokeWidth={2.2} />
              </button>
            </Tooltip>
          ))}

        {isConfirmingDelete ? (
          <>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                deleteModel();
                setConfirmDeleteId(null);
              }}
              className="rounded-md bg-[rgba(var(--danger),0.18)] p-1 text-[rgb(var(--danger))] transition-colors hover:bg-[rgba(var(--danger),0.3)] cursor-pointer"
              aria-label={MODEL_HUB_COPY.row.confirmDelete}
            >
              <Check size={12} strokeWidth={3} />
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                setConfirmDeleteId(null);
              }}
              className="rounded-md p-1 text-[rgb(var(--foreground-muted))] transition-colors hover:bg-[rgba(var(--foreground),0.1)] cursor-pointer"
              aria-label={MODEL_HUB_COPY.row.cancel}
            >
              <ArrowLeft size={12} strokeWidth={2.4} />
            </button>
          </>
        ) : (
          <>
            {hasTooltip && (
              <Tooltip label={specsSheet} side="left" mobileTapEnabled>
                <span
                  className="inline-flex items-center rounded-md p-1 text-[rgb(var(--foreground-muted))]/70 transition-colors hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--accent))] cursor-help"
                  aria-label={`${name} details`}
                >
                  <Info size={13} strokeWidth={2} />
                </span>
              </Tooltip>
            )}
            {canDelete && (
              <Tooltip label={MODEL_HUB_COPY.row.deleteWeights} side="left" asChild>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDeleteId(id);
                  }}
                  className="rounded-md p-1 text-[rgb(var(--foreground-muted))]/70 transition-colors hover:bg-[rgba(var(--danger),0.14)] hover:text-[rgb(var(--danger))] cursor-pointer"
                  aria-label={MODEL_HUB_COPY.row.deleteWeights}
                >
                  <Trash2 size={13} strokeWidth={2} />
                </button>
              </Tooltip>
            )}
          </>
        )}

        {cornerAction && <div className="flex items-center">{cornerAction}</div>}
        </>
      }
    />
  );
});

SpeechModelCard.displayName = "SpeechModelCard";
