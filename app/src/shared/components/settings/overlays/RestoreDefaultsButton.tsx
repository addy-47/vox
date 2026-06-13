import { memo, useState, useEffect } from "react";
import { RotateCcw, AlertTriangle } from "lucide-react";
import { useSettings } from "@/shared/context/SettingsContext";
import { cn } from "@/shared/lib/utils";

export const RestoreDefaultsButton = memo(() => {
  const { restoreDefaults } = useSettings();
  const [isConfirming, setIsConfirming] = useState(false);

  useEffect(() => {
    if (!isConfirming) return;
    const timer = setTimeout(() => setIsConfirming(false), 4000);
    return () => clearTimeout(timer);
  }, [isConfirming]);

  const handleRestore = () => {
    if (isConfirming) {
      restoreDefaults();
      setIsConfirming(false);
    } else {
      setIsConfirming(true);
    }
  };

  return (
    <button
      onClick={handleRestore}
      className={cn(
        "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 hover:scale-105 cursor-pointer shadow-lg shadow-[rgba(var(--accent),0.06)] dark:shadow-[rgba(0,0,0,0.3)] relative group",
        isConfirming
          ? "border-red-500 bg-red-500/20 text-red-500 shadow-[0_0_18px_rgba(239,68,68,0.3)] animate-[pulse_1.5s_infinite]"
          : "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] dark:bg-black/35 dark:border-[rgba(var(--accent),0.35)] hover:bg-[rgb(var(--accent))]/20"
      )}
      aria-label={isConfirming ? "Confirm restore defaults" : "Restore default settings"}
    >
      {isConfirming ? (
        <AlertTriangle size={18} className="animate-bounce" />
      ) : (
        <RotateCcw size={18} className="group-hover:rotate-45 transition-transform duration-300" />
      )}
      {/* Tooltip */}
      <div 
        className={cn(
          "absolute bottom-12 right-0 scale-95 opacity-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-200 pointer-events-none whitespace-nowrap px-2.5 py-1.5 rounded-lg border shadow-lg text-[10px] font-medium z-50",
          isConfirming
            ? "bg-red-950/95 border-red-500/20 text-red-200"
            : "bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground-muted))]/80"
        )}
      >
        {isConfirming ? "Click again to confirm Reset" : "Restore default settings"}
      </div>
    </button>
  );
});

RestoreDefaultsButton.displayName = "RestoreDefaultsButton";
