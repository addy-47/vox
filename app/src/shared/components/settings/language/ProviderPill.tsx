import { memo, useState } from "react";
import { ArrowRight, Check, Pencil, Trash2, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { LANGUAGE_CARD_COPY, PROVIDERS_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { ProviderLogo } from "../shared/ProviderLogo";
import { CatalogRowCard } from "../shared/CatalogRowCard";

export interface ProviderPillProps {
  /** Inventory or cloud provider id. Never a display name. */
  id: string;
  name: string;
  /** Brand hint for `ProviderLogo` inference (custom servers may be named). */
  providerName?: string;
  logo?: string | null;
  /** One dim line: tier, active model, or endpoint. */
  meta: string;
  /** Overrides the brand mark (the on-device row uses a chip glyph). */
  leading?: React.ReactNode;
  active: boolean;
  onSelect: () => void;
  /**
   * Opens this provider's model catalog. Rendered as its own trailing hitbox:
   * the row body only selects, so a browse cannot happen by accident.
   */
  onBrowse?: () => void;
  /**
   * Triggers inline API key editing.
   */
  onEdit?: () => void;
  /**
   * Deletes the configured provider key.
   */
  onDelete?: () => void;
  /** Replaces the trailing arrow (unconnected rows show a plus). */
  trailing?: React.ReactNode;
  className?: string;
  /** Unconnected provider: dimmer text and a dimmed brand mark. */
  muted?: boolean;
  /** Replaces the pill content with an inline editor. */
  body?: React.ReactNode;
  /** Content rendered in an expanded bottom slot below title and meta. */
  expandedContent?: React.ReactNode;
}

/**
 * One available provider in the language card. Brand mark, provider name, one
 * status line.
 * Edit and delete controls appear only on hover in a single line with the breadcrumb.
 * Two-step delete transforms the cluster into a confirm tick and cancel cross.
 */
export const ProviderPill = memo(({
  id,
  name,
  providerName,
  logo,
  meta,
  leading,
  active,
  onSelect,
  onBrowse,
  onEdit,
  onDelete,
  trailing,
  className,
  muted = false,
  body,
  expandedContent,
}: ProviderPillProps) => {
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);

  return (
    <CatalogRowCard
      leading={
        <span className={cn("flex shrink-0 items-center justify-center", muted && "opacity-50")}>
          {leading ?? (
            <ProviderLogo
              providerId={id}
              providerName={providerName ?? name}
              logo={logo}
              size={32}
            />
          )}
        </span>
      }
      muted={muted}
      body={body}
      expandedContent={expandedContent}
      title={name}
      meta={meta}
      active={active}
      onSelect={onSelect}
      ariaLabel={name}
      className={className}
      trailing={
        trailing !== undefined ? (
          trailing
        ) : isConfirmingDelete ? (
          <div className="flex items-center gap-1 shrink-0">
            <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider} side="top" asChild>
              <button
                type="button"
                aria-label="Confirm delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete?.();
                  setIsConfirmingDelete(false);
                }}
                className="p-1 rounded-md bg-[rgba(var(--danger),0.18)] text-[rgb(var(--danger))] hover:bg-[rgba(var(--danger),0.3)] transition-colors cursor-pointer shrink-0"
              >
                <Check size={12} strokeWidth={2.75} />
              </button>
            </Tooltip>
            <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
              <button
                type="button"
                aria-label="Cancel delete"
                onClick={(e) => {
                  e.stopPropagation();
                  setIsConfirmingDelete(false);
                }}
                className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer shrink-0"
              >
                <X size={12} strokeWidth={2.5} />
              </button>
            </Tooltip>
          </div>
        ) : (
          <div className="flex items-center gap-1 shrink-0">
            {onEdit && (
              <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey} side="top" asChild>
                <button
                  type="button"
                  aria-label={`${PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey}: ${name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onEdit();
                  }}
                  className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer shrink-0 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
                >
                  <Pencil size={12} />
                </button>
              </Tooltip>
            )}
            {onDelete && (
              <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider} side="top" asChild>
                <button
                  type="button"
                  aria-label={`${PROVIDERS_CONFIG_DESK_COPY.actions.deleteProvider}: ${name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    setIsConfirmingDelete(true);
                  }}
                  className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--error))] hover:bg-[rgba(var(--error),0.1)] transition-colors cursor-pointer shrink-0 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
                >
                  <Trash2 size={12} />
                </button>
              </Tooltip>
            )}
            {onBrowse && (
              <Tooltip label={LANGUAGE_CARD_COPY.browseModels} side="left" asChild>
                <button
                  type="button"
                  aria-label={`${LANGUAGE_CARD_COPY.browseModels}: ${name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onBrowse();
                  }}
                  className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/50 transition-colors hover:bg-[rgba(var(--accent),0.1)] hover:text-[rgb(var(--accent))] cursor-pointer shrink-0"
                >
                  <ArrowRight size={13} strokeWidth={2} />
                </button>
              </Tooltip>
            )}
          </div>
        )
      }
    />
  );
});

ProviderPill.displayName = "ProviderPill";
