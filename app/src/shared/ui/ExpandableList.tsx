import { createContext, memo, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { Maximize2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Modal } from "@/shared/ui/Modal";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useExpandedModalStore } from "@/store/expandedModalStore";
import { useSettingsStore } from "@/store/settingsStore";

export interface ExpandableListContextValue {
  isExpanded: boolean;
  close: () => void;
  open: () => void;
}

export const ExpandableListContext = createContext<ExpandableListContextValue>({
  isExpanded: false,
  close: () => {},
  open: () => {},
});

export const useExpandableList = () => useContext(ExpandableListContext);

export interface ExpandableListProps {
  /** Modal heading. Also used as the dialog's accessible name fallback. */
  title: React.ReactNode;
  icon?: React.ReactNode;
  subtitle?: React.ReactNode;
  /** Optional subtitle specifically for the modal header. Defaults to undefined to avoid redundant subtext. */
  modalSubtitle?: React.ReactNode;
  /** Rendered in the modal header, left of the close button. */
  headerActions?: React.ReactNode;
  /** Rendered in the modal body, below the list. */
  footer?: React.ReactNode;
  /** Tooltip + aria-label for the expand trigger. */
  expandLabel: string;
  ariaLabel: string;
  /** Caps the inline slot via height classes. Omit to let flex sizing govern.
   * The modal slot is always full height. */
  inlineMaxHeightClass?: string;
  /** Optional trailing content for the inline action row. Row placement only. */
  inlineActions?: React.ReactNode;
  /** "floating" overlaps the list's bottom-right corner; "row" appends an action row. */
  triggerPlacement?: "floating" | "row";
  className?: string;
  children: React.ReactNode;
  /** Optional dedicated modal body when expanded. If provided, replaces children inside the modal. */
  modalContent?: React.ReactNode | ((props: { close: () => void }) => React.ReactNode);
  /** Optional back button handler rendered on the extreme left of the modal header */
  onBack?: () => void;
  /** Settings domain to discard on modal exit if uncommitted changes exist */
  domainId?: string;
  /** Explicit callback when modal closes without committing */
  onDiscard?: () => void;
}

/**
 * A list too long to scan at its inline card height. The inline slot stays
 * authoritative for selection and keeps rendering while the modal is open —
 * the modal is a larger viewport onto the same state, not a second editor
 * (design-spec §13, Tier 2b dense-list expansion).
 *
 * When expanded, it can either render `children` or a dedicated `modalContent`
 * tailored for spacious multi-column viewports.
 */
export const ExpandableList = memo(
  ({
    title,
    icon,
    subtitle: _subtitle,
    modalSubtitle,
    headerActions,
    footer,
    expandLabel,
    ariaLabel,
    inlineMaxHeightClass,
    inlineActions,
    triggerPlacement = "row",
    className,
    children,
    modalContent,
    onBack,
    domainId,
    onDiscard,
  }: ExpandableListProps) => {
    const [expanded, setExpanded] = useState(false);

    const open = useCallback(() => setExpanded(true), []);
    const handleClose = useCallback(() => {
      if (domainId) {
        useSettingsStore.getState().discardDomainChanges(domainId);
      }
      onDiscard?.();
      setExpanded(false);
    }, [domainId, onDiscard]);

    // While the modal owns the commit flow, background card footers stand down.
    const pushModal = useExpandedModalStore((s) => s.pushModal);
    const popModal = useExpandedModalStore((s) => s.popModal);
    useEffect(() => {
      if (!expanded) return;
      pushModal();
      return () => {
        popModal();
      };
    }, [expanded, pushModal, popModal]);

    const inlineContext = useMemo(
      () => ({ isExpanded: false, close: handleClose, open }),
      [handleClose, open]
    );

    const modalContext = useMemo(
      () => ({ isExpanded: true, close: handleClose, open }),
      [handleClose, open]
    );

    return (
      <div className={cn("relative flex flex-col min-h-0", className)}>
        <ExpandableListContext.Provider value={inlineContext}>
          <div className={cn("flex-1 min-h-0 overflow-hidden", inlineMaxHeightClass)}>
            {children}
          </div>
        </ExpandableListContext.Provider>

        {triggerPlacement === "floating" ? (
          <Tooltip label={expandLabel} side="top" asChild>
            <button
              type="button"
              onClick={open}
              aria-label={expandLabel}
              className="absolute -bottom-2 right-1 z-10 w-8 h-8 flex items-center justify-center rounded-lg border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.85)] backdrop-blur-sm text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.95)] transition-colors cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] shadow-sm"
            >
              <Maximize2 size={13} strokeWidth={1.75} />
            </button>
          </Tooltip>
        ) : (
          <div className="flex items-center justify-end gap-1.5 shrink-0 pt-1">
            {inlineActions}
            <Tooltip label={expandLabel} side="top" asChild>
              <button
                type="button"
                onClick={open}
                aria-label={expandLabel}
                className="w-8 h-8 flex items-center justify-center rounded-lg border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
              >
                <Maximize2 size={13} strokeWidth={1.75} />
              </button>
            </Tooltip>
          </div>
        )}

        <Modal
          open={expanded}
          onClose={handleClose}
          onBack={onBack}
          position="global"
          icon={icon}
          title={title}
          subtitle={modalSubtitle}
          headerActions={
            headerActions ? (
              <ExpandableListContext.Provider value={modalContext}>
                {headerActions}
              </ExpandableListContext.Provider>
            ) : undefined
          }
          footer={footer}
          ariaLabel={ariaLabel}
          className="w-[min(480px,92vw)] h-[min(540px,72vh)] lg:w-[min(1120px,86vw)] lg:h-[min(760px,76vh)] max-w-[calc(100vw-32px)] max-h-[calc(100dvh-96px)] rounded-2xl bg-[rgba(var(--card),0.92)] backdrop-blur-2xl border border-[rgba(var(--border),0.2)] shadow-2xl"
          bodyClassName="h-full flex flex-col min-h-0 overflow-hidden p-3.5 sm:p-5"
        >
          <ExpandableListContext.Provider value={modalContext}>
            <div className="flex-1 min-h-0 flex flex-col">
              {modalContent
                ? typeof modalContent === "function"
                  ? modalContent({ close: handleClose })
                  : modalContent
                : children}
            </div>
          </ExpandableListContext.Provider>
        </Modal>
      </div>
    );
  }
);

ExpandableList.displayName = "ExpandableList";
