import { memo, useCallback } from "react";
import { HelpCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useHelp } from "./HelpDrawerProvider";
import { HELP_DRAWER_COPY } from "@/data/helpCopy";

interface HelpTriggerButtonProps {
  deepLink?: string | null;
  className?: string;
  size?: "sm" | "md";
  label?: string;
}

const HelpTriggerButtonInner = memo(
  ({ deepLink, className, size = "sm", label }: HelpTriggerButtonProps) => {
    const { openHelp } = useHelp();

    const handleClick = useCallback(() => {
      openHelp(deepLink ?? null);
    }, [openHelp, deepLink]);

    const isMd = size === "md";
    const dims = isMd ? "w-9 h-9" : "w-8 h-8";
    const iconSize = isMd ? 16 : 14;

    return (
      <Tooltip
        label={label ?? HELP_DRAWER_COPY.triggerLabel}
        side="bottom"
      >
        <button
          onClick={handleClick}
          className={cn(
            "inline-flex items-center justify-center rounded-xl border transition-all cursor-pointer shrink-0",
            "border-[rgba(var(--accent),0.15)] bg-[rgb(var(--foreground))]/[0.03]",
            "text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--accent))]/10 hover:text-[rgb(var(--accent))]",
            "focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
            dims,
            className
          )}
          aria-label={label ?? HELP_DRAWER_COPY.triggerLabel}
        >
          <HelpCircle size={iconSize} />
        </button>
      </Tooltip>
    );
  }
);
HelpTriggerButtonInner.displayName = "HelpTriggerButton";

export const HelpTriggerButton = HelpTriggerButtonInner;
