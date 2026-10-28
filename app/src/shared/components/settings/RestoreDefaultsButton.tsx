import { memo, useState, useCallback } from "react";
import { RotateCcw } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ConfirmModal } from "@/shared/ui/ConfirmModal";
import { SETTINGS_COPY } from "@/data/settingsCopy";

/**
 * Restore Defaults control for Settings view.
 * Clicking the reset button opens a ConfirmModal; confirming calls restoreDefaults.
 */
export const RestoreDefaultsButton = memo(() => {
  const restoreDefaults = useSettingsStore.getState().restoreDefaults;
  const [isOpen, setIsOpen] = useState(false);

  const handleConfirm = useCallback(async () => {
    setIsOpen(false);
    await restoreDefaults();
  }, [restoreDefaults]);

  const handleCancel = useCallback(() => {
    setIsOpen(false);
  }, []);

  return (
    <>
      <Tooltip label={SETTINGS_COPY.restoreAria} side="bottom">
        <button
          type="button"
          onClick={() => setIsOpen(true)}
          className="inline-flex items-center justify-center w-8 h-8 rounded-xl border border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)] transition-all shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))] cursor-pointer"
          aria-label={SETTINGS_COPY.restoreAria}
        >
          <RotateCcw size={14} strokeWidth={1.75} />
        </button>
      </Tooltip>

      <ConfirmModal
        open={isOpen}
        title={SETTINGS_COPY.restoreDefaults}
        description="Reset all settings to their default values. This cannot be undone."
        confirmLabel={SETTINGS_COPY.restoreDefaults}
        cancelLabel={SETTINGS_COPY.cancel}
        isDestructive={true}
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    </>
  );
});

RestoreDefaultsButton.displayName = "RestoreDefaultsButton";