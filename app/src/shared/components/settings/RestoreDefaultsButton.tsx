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
        "flex items-center justify-center w-11 h-11 rounded-full border transition-all duration-300 hover:scale-105 cursor-pointer relative group",
        isConfirming
          ? "border-red-500/80 bg-red-500/20 text-red-400 dark:text-red-400 shadow-[0_0_18px_rgba(239,68,68,0.3)] animate-pulse-slow"
          : "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] dark:bg-[rgba(var(--foreground),0.12)] dark:border-[rgba(var(--accent),0.35)] hover:bg-[rgb(var(--accent))]/20"
      )}
      aria-label={isConfirming ? "Confirm restore defaults" : "Restore default settings"}
    >
      {isConfirming ? (
        <AlertTriangle size={22} className="animate-pulse" />
      ) : (
        <RotateCcw size={22} className="group-hover:rotate-45 transition-transform duration-300" />
      )}
      {/* Tooltip */}
      <div 
        className={cn(
          "absolute bottom-14 right-0 translate-y-1 scale-95 opacity-0 group-hover:translate-y-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-300 ease-out pointer-events-none whitespace-nowrap px-3 py-1.5 rounded-xl border shadow-[0_8px_30px_rgba(0,0,0,0.12)] dark:shadow-[0_8px_30px_rgba(0,0,0,0.35)] backdrop-blur-md text-[11px] font-bold tracking-wide uppercase z-50",
          isConfirming
            ? "bg-red-950/90 border-red-500/40 text-red-300 shadow-[0_0_20px_rgba(239,68,68,0.2)]"
            : "bg-[rgb(var(--background))]/95 dark:bg-zinc-950/95 border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]"
        )}
      >
        {isConfirming ? "Click again to reset" : "Restore default settings"}
      </div>
    </button>
  );
});

RestoreDefaultsButton.displayName = "RestoreDefaultsButton";
