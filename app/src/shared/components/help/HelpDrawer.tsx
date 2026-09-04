import { memo } from "react";
import { HelpCircle, X } from "lucide-react";
import { Drawer } from "@/shared/ui/Drawer";
import { Tooltip } from "@/shared/ui/Tooltip";
import { cn } from "@/shared/lib/utils";
import { HELP_DRAWER_COPY } from "@/data/helpCopy";
import { HelpContent } from "./HelpContent";

interface HelpDrawerProps {
  open: boolean;
  onClose: () => void;
  deepLink: string | null;
}

const HelpDrawerInner = memo(({ open, onClose, deepLink }: HelpDrawerProps) => {
  return (
    <Drawer
      open={open}
      onClose={onClose}
      position="global"
      ariaLabel="Help & guide"
      height={72}
      minHeight={45}
      maxHeight={92}
      resizeHint="Drag to resize · double-click to expand"
      icon={
        <div className="p-2.5 rounded-xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))]">
          <HelpCircle size={20} />
        </div>
      }
      title={
        <h2 className="font-display text-[15px] font-bold tracking-wide text-[rgb(var(--foreground))]">
          {HELP_DRAWER_COPY.headerTitle}
        </h2>
      }
      subtitle={
        <p className="text-[11px] text-[rgb(var(--foreground-muted))] font-sans mt-0.5">
          {HELP_DRAWER_COPY.headerSubtitle}
        </p>
      }
      headerActions={
        <Tooltip label="Close" side="bottom">
          <button
            onClick={onClose}
            className={cn(
              "flex items-center justify-center w-8 h-8 rounded-full glass-card text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
            )}
            aria-label="Close help"
          >
            <X size={18} />
          </button>
        </Tooltip>
      }
    >
      <HelpContent deepLink={deepLink} onClose={onClose} />
    </Drawer>
  );
});
HelpDrawerInner.displayName = "HelpDrawer";

export const HelpDrawer = HelpDrawerInner;
