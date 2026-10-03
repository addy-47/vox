import { memo } from "react";
import { Sun, Moon } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";

export const ThemeToggleButton = memo(() => {
  const theme = useSettingsStore((s) => s.draftSettings?.appearance.theme ?? s.settings?.appearance.theme ?? "dark");
  const toggleTheme = useSettingsStore((s) => s.toggleTheme);
  const isDark = theme !== "light";

  return (
    <Tooltip label={isDark ? "Switch to light mode" : "Switch to dark mode"} side="bottom">
      <button
        onClick={toggleTheme}
        aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
        className={cn(
          "inline-flex items-center justify-center w-8 h-8 rounded-xl border transition-all cursor-pointer pointer-events-auto shrink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
          "border-[rgba(var(--border),0.15)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.06)]"
        )}
      >
        {isDark ? <Sun size={14} strokeWidth={1.75} /> : <Moon size={14} strokeWidth={1.75} />}
      </button>
    </Tooltip>
  );
});

ThemeToggleButton.displayName = "ThemeToggleButton";
