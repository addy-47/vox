import { memo } from "react";
import { Cpu } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { HELP_DRAWER_COPY, type HelpTier } from "@/data/helpCopy";

interface HelpTierBadgeProps {
  tier: HelpTier;
}

const tierPalette: Record<HelpTier, { ring: string; text: string; bg: string; label: string }> = {
  "1A": {
    ring: "border-[rgba(var(--muted),0.4)]",
    text: "text-[rgb(var(--muted))]",
    bg: "bg-[rgba(var(--muted),0.08)]",
    label: "1A · CPU only",
  },
  "1B": {
    ring: "border-[rgba(var(--accent),0.35)]",
    text: "text-[rgb(var(--accent))]",
    bg: "bg-[rgba(var(--accent),0.1)]",
    label: "1B · Local + GPU",
  },
  "2A": {
    ring: "border-[rgba(var(--info),0.35)]",
    text: "text-[rgb(var(--info))]",
    bg: "bg-[rgba(var(--info),0.1)]",
    label: "2A · Remote LLM",
  },
  "2B": {
    ring: "border-[rgba(var(--success),0.35)]",
    text: "text-[rgb(var(--success))]",
    bg: "bg-[rgba(var(--success),0.1)]",
    label: "2B · Cloud LLM",
  },
  "3": {
    ring: "border-[rgba(var(--violet),0.35)]",
    text: "text-[rgb(var(--violet))]",
    bg: "bg-[rgba(var(--violet),0.1)]",
    label: "3 · Realtime",
  },
};

const HelpTierBadgeInner = memo(({ tier }: HelpTierBadgeProps) => {
  const palette = tierPalette[tier];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-xl border text-[11px] font-mono font-bold uppercase tracking-wider",
        palette.ring,
        palette.text,
        palette.bg
      )}
    >
      <Cpu size={12} className="shrink-0" />
      <span>{HELP_DRAWER_COPY.tierBadgePrefix}: {palette.label}</span>
    </span>
  );
});
HelpTierBadgeInner.displayName = "HelpTierBadge";

export const HelpTierBadge = HelpTierBadgeInner;
