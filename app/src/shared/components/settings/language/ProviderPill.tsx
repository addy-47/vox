import { memo } from "react";
import { ArrowRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
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
 * status line, and an arrow that promises the model list behind it. The arrow
 * is decorative: the whole row is the hit area.
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
    title={name}
    meta={meta}
    active={active}
    onSelect={onSelect}
    ariaLabel={name}
    className={className}
    trailing={
      trailing ?? (
        <ArrowRight
          size={13}
          strokeWidth={2}
          className="text-[rgb(var(--foreground-muted))]/50 transition-transform group-hover:translate-x-0.5 group-hover:text-[rgb(var(--accent))]"
        />
      )
    }
  />
));

ProviderPill.displayName = "ProviderPill";
