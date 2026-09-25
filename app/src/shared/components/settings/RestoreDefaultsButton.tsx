import { memo, useState, useCallback } from "react";
import { RotateCcw, Check, X } from "lucide-react";
import { useSettings } from "@/shared/hooks/useSettings";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SETTINGS_COPY } from "@/data/settingsCopy";

/**
 * Clean 2-step Restore Defaults control for Settings view.
 * Step 1: Idle reset trigger button in the top-right cluster.
 * Step 2: Unboxed inline transition: accent-colored "Restore settings" copy,
 *         red tick (confirm), and grey cross (cancel). Zero pill container, zero red background.
 */
export const RestoreDefaultsButton = memo(() => {
  const { restoreDefaults } = useSettings();
  const [isConfirming, setIsConfirming] = useState(false);

  const handleConfirm = useCallback(() => {
    restoreDefaults();
    setIsConfirming(false);
  }, [restoreDefaults]);

  const handleCancel = useCallback(() => {
    setIsConfirming(false);
  }, []);

  if (isConfirming) {
    return (
      <div className="inline-flex items-center gap-1.5 h-8 animate-fade-in shrink-0">
        <span className="text-[11px] font-bold tracking-wide text-[rgb(var(--accent))] select-none whitespace-nowrap">
          Restore settings
        </span>
        <div className="flex items-center gap-0.5">
          <Tooltip label="Confirm restore" side="bottom">
            <button
              type="button"
              onClick={handleConfirm}
              className="p-1 text-[rgb(var(--danger))] hover:text-rose-400 hover:scale-115 transition-all cursor-pointer"
              aria-label="Confirm restore defaults"
            >
              <Check size={13} strokeWidth={2.5} />
            </button>
          </Tooltip>
          <Tooltip label="Cancel" side="bottom">
            <button
              type="button"
              onClick={handleCancel}
              className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:scale-115 transition-all cursor-pointer"
              aria-label="Cancel restore defaults"
            >
              <X size={13} strokeWidth={2} />
            </button>
          </Tooltip>
        </div>
      </div>
    );
  }

  return (
    <Tooltip label={SETTINGS_COPY.restoreAria} side="bottom">
      <button
        type="button"
        onClick={() => setIsConfirming(true)}
        className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer"
        aria-label={SETTINGS_COPY.restoreAria}
      >
        <RotateCcw size={14} strokeWidth={1.75} />
      </button>
    </Tooltip>
  );
});

RestoreDefaultsButton.displayName = "RestoreDefaultsButton";
