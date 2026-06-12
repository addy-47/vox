import { memo } from "react";
import { RotateCcw } from "lucide-react";
import { useSettings } from "@/shared/context/SettingsContext";

export const RestoreDefaultsButton = memo(() => {
  const { restoreDefaults } = useSettings();

  const handleRestore = () => {
    const confirmRestore = window.confirm("Are you sure you want to restore all settings to their default values? This cannot be undone.");
    if (confirmRestore) {
      restoreDefaults();
    }
  };

  return (
    <button
      onClick={handleRestore}
      className="flex items-center justify-center w-8 h-8 rounded-full border border-transparent bg-transparent hover:bg-[rgba(var(--accent),0.08)] text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-all duration-300 shadow-none group relative"
      aria-label="Restore default settings"
    >
      <RotateCcw size={13} className="group-hover:rotate-45 transition-transform duration-300" />
      {/* Tooltip */}
      <div className="absolute bottom-10 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none whitespace-nowrap px-2.5 py-1.5 rounded-lg bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] shadow-lg text-[10px] font-medium text-[rgb(var(--foreground-muted))]/80 z-50">
        Restore default settings
      </div>
    </button>
  );
});

RestoreDefaultsButton.displayName = "RestoreDefaultsButton";
