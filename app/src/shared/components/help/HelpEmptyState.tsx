import { memo } from "react";
import { Compass } from "lucide-react";
import { HELP_DRAWER_COPY } from "@/data/helpCopy";

interface HelpEmptyStateProps {
  onClose: () => void;
}

const HelpEmptyStateInner = memo(({ onClose }: HelpEmptyStateProps) => {
  return (
    <div className="flex-1 flex flex-col items-center justify-center text-center px-8 py-12 gap-3">
      <div className="p-3 rounded-2xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]">
        <Compass size={28} />
      </div>
      <h3 className="font-display text-[16px] font-black uppercase tracking-[0.16em] text-[rgb(var(--foreground))]">
        {HELP_DRAWER_COPY.emptyStateTitle}
      </h3>
      <p className="text-[13px] leading-[1.6] text-[rgb(var(--foreground-muted))] max-w-md">
        {HELP_DRAWER_COPY.emptyStateBody}
      </p>
      <button
        onClick={onClose}
        className="mt-2 px-3.5 py-1.5 rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[12px] font-bold uppercase tracking-wider hover:brightness-110 transition-all cursor-pointer"
      >
        {HELP_DRAWER_COPY.emptyClose}
      </button>
    </div>
  );
});
HelpEmptyStateInner.displayName = "HelpEmptyState";

export const HelpEmptyState = HelpEmptyStateInner;
