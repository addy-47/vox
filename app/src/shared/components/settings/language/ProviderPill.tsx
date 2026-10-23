import { memo } from "react";
import { ArrowRight, Pencil, Trash2 } from "lucide-react";
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
   * Triggers inline API key editing. Placed immediately to the right of the provider name.
   */
  onEdit?: () => void;
  /**
   * Deletes the configured provider key. Placed immediately to the right of the edit button.
   */
  onDelete?: () => void;
  /** Replaces the trailing arrow (unconnected rows show a plus). */
  trailing?: React.ReactNode;
  className?: string;
  /** Unconnected provider: dimmer text and a dimmed brand mark. */
  muted?: boolean;
  /** Replaces the pill content with an inline editor. */
  body?: React.ReactNode;
}

/**
 * One available provider in the language card. Brand mark, provider name, one
 * status line. The body selects the provider; the arrow is a separate hitbox
 * that opens that provider's models, matching the provider modal.
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
}: ProviderPillProps) => (
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
    title={
      onEdit || onDelete ? (
        <span className="flex items-center gap-1.5 min-w-0">
          <span className="truncate">{name}</span>
          {onEdit && (
            <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey} side="top" asChild>
              <button
                type="button"
                aria-label={`${PROVIDERS_CONFIG_DESK_COPY.llm.cloud.editKey}: ${name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onEdit();
                }}
                className="p-1 rounded hover:bg-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--accent))] transition-colors text-[rgb(var(--foreground-muted))]/60 cursor-pointer shrink-0"
              >
                <Pencil size={11} />
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
                  onDelete();
                }}
                className="p-1 rounded hover:bg-[rgba(var(--error),0.1)] hover:text-[rgb(var(--error))] transition-colors text-[rgb(var(--foreground-muted))]/40 cursor-pointer shrink-0"
              >
                <Trash2 size={11} />
              </button>
            </Tooltip>
          )}
        </span>
      ) : (
        name
      )
    }
    meta={meta}
    active={active}
    onSelect={onSelect}
    ariaLabel={name}
    className={className}
    trailing={
      trailing !== undefined ? (
        trailing
      ) : onBrowse ? (
        <Tooltip label={LANGUAGE_CARD_COPY.browseModels} side="left" asChild>
          <button
            type="button"
            aria-label={`${LANGUAGE_CARD_COPY.browseModels}: ${name}`}
            onClick={(e) => {
              e.stopPropagation();
              onBrowse();
            }}
            className="w-7 h-7 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/50 transition-colors hover:bg-[rgba(var(--accent),0.1)] hover:text-[rgb(var(--accent))] cursor-pointer"
          >
            <ArrowRight size={13} strokeWidth={2} />
          </button>
        </Tooltip>
      ) : null
    }
  />
));

ProviderPill.displayName = "ProviderPill";
