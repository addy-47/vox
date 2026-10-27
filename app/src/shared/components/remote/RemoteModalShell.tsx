import { memo, type ReactNode } from "react";
import { X } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { Modal } from "@/shared/ui/Modal";
import { cn } from "@/shared/lib/utils";

export interface RemoteModalShellProps {
  open: boolean;
  onClose: () => void;
  /** Accessible dialog name. */
  ariaLabel: string;
  /** Leading kicker glyph (e.g. Shield, Link2) rendered at accent tone. */
  kickerIcon: ReactNode;
  /** Uppercase tracked kicker label above the title. */
  kickerLabel: string;
  title: string;
  description: string;
  /** Standardized left column (md:col-span-7). */
  leftContent: ReactNode;
  /** Standardized right column (md:col-span-5). */
  rightContent: ReactNode;
  /** Optional full-width action row below the two-column body. */
  footer?: ReactNode;
  /** Optional full-width body override for error states (replaces the grid). */
  errorState?: ReactNode;
  /** Extra classes on the left body column (e.g. mobile order). */
  leftColumnClassName?: string;
  /** Extra classes on the right body column (e.g. mobile order). */
  rightColumnClassName?: string;
  /** Stable key for AnimatePresence — change it to cross-fade the whole body (e.g. "pairing" → "auth"). */
  contentKey?: string;
  /** Additional classes on the Modal panel (e.g. min height). */
  className?: string;
}

/**
 * Shared shell for the Tier 2b remote-control surfaces (pairing and device
 * authorization). Owns the Modal contract (`global`, `fitContent`, 760px cap),
 * the padding rhythm, and the header grammar (accent kicker + 26px display
 * title + muted description + circular close) so the two surfaces cannot
 * diverge again. Baseline layout is the authorization screen's: a two-column
 * `md:grid-cols-12` body with centered columns and an optional footer row.
 *
 * Bodies remain owned by each caller — this shell standardizes structure only.
 */
export const RemoteModalShell = memo(
  ({
    open,
    onClose,
    ariaLabel,
    kickerIcon,
    kickerLabel,
    title,
    description,
    leftContent,
    rightContent,
    footer,
    errorState,
    leftColumnClassName,
    rightColumnClassName,
    contentKey,
    className,
  }: RemoteModalShellProps) => {
    return (
      <Modal
        open={open}
        onClose={onClose}
        position="global"
        fitContent
        ariaLabel={ariaLabel}
        className={cn("max-w-[760px] w-[94vw] md:w-full overflow-hidden", className)}
      >
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={contentKey ?? "default"}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
            className="flex flex-col p-6 sm:p-8 md:p-9 select-none"
          >
          {/* Header: kicker + title + description, close at trailing edge */}
          <div className="flex items-start justify-between gap-4">
            <div className="flex flex-col min-w-0">
              <div className="flex items-center gap-1.5 text-[rgb(var(--accent))]">
                {kickerIcon}
                <span className="text-[11px] font-mono tracking-[0.2em] uppercase font-bold text-[rgb(var(--accent))]">
                  {kickerLabel}
                </span>
              </div>
              <h2 className="text-2xl sm:text-[26px] font-bold tracking-tight text-[rgb(var(--foreground))] mt-1">
                {title}
              </h2>
              <p className="text-[12.5px] text-[rgb(var(--foreground-muted))] mt-1 leading-normal max-w-[480px]">
                {description}
              </p>
            </div>

            <button
              type="button"
              onClick={onClose}
              aria-label="Close"
              className="w-8 h-8 rounded-full bg-[rgba(var(--foreground),0.06)] hover:bg-[rgba(var(--foreground),0.12)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] flex items-center justify-center transition-colors cursor-pointer shrink-0"
            >
              <X size={16} />
            </button>
          </div>

          {/* Two-column body: 12-col grid; columns stretch to equal height so
              the left steps column can distribute evenly against the QR column */}
          {!errorState && (
            <div className="grid grid-cols-1 md:grid-cols-12 gap-6 md:gap-8 items-stretch mt-7">
              <div className={cn("md:col-span-7 flex flex-col min-w-0", leftColumnClassName)}>{leftContent}</div>
              <div className={cn("md:col-span-5 flex flex-col items-center justify-center", rightColumnClassName)}>
                {rightContent}
              </div>
            </div>
          )}

          {errorState && (
            <div className="mt-7 flex flex-col items-center justify-center gap-3 text-center">
              {errorState}
            </div>
          )}

          {footer && (
            <div className="flex items-center gap-4 mt-8 pt-2 shrink-0">{footer}</div>
          )}
          </motion.div>
        </AnimatePresence>
      </Modal>
    );
  }
);

RemoteModalShell.displayName = "RemoteModalShell";
