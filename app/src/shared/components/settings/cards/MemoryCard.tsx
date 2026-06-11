import { memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { Database } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export const MemoryCard = memo(() => {
  const { draftSettings, updateDraft } = useSettings();

  if (!draftSettings) return null;
  const { persistence } = draftSettings;

  return (
    <div className="w-full lg:w-[320px] bg-transparent lg:bg-black/15 lg:backdrop-blur-md border-0 lg:border border-[rgba(var(--accent),0.10)] rounded-none lg:rounded-2xl p-0 lg:p-5 shadow-none lg:shadow-xl shadow-black/30 text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85">
      {/* Header */}
      <div className="flex items-center gap-2 mb-4 shrink-0">
        <Database className="text-[rgb(var(--accent))]" size={16} />
        <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
          Memory & Privacy
        </span>
      </div>

      <div className="space-y-4">
        {/* Toggle Private Mode */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-bold text-[rgb(var(--foreground))]/80">Private Mode</span>
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70">Incognito (No logs)</span>
          </div>
          <button
            onClick={() => updateDraft("persistence", "private_mode", !persistence.private_mode)}
            className={cn(
              "w-10 h-5 rounded-full relative transition-all duration-300",
              persistence.private_mode ? "bg-[rgb(var(--accent))]" : "bg-[rgb(var(--foreground))]/15"
            )}
            role="switch"
            aria-checked={persistence.private_mode}
            aria-label="Private mode toggle"
          >
            <div
              className={cn(
                "absolute top-0.5 w-4 h-4 rounded-full bg-white transition-all duration-300 shadow-sm",
                persistence.private_mode ? "left-5" : "left-0.5"
              )}
            />
          </button>
        </div>

        {/* Max Sessions */}
        <div className="space-y-1">
          <div className="flex justify-between items-center">
            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
              Max Sessions
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">
              {persistence.max_sessions}
            </span>
          </div>
          <input
            type="range"
            min="5"
            max="500"
            step="5"
            value={persistence.max_sessions}
            onChange={(e) => updateDraft("persistence", "max_sessions", Number(e.target.value))}
            className="w-full"
          />
        </div>

        {/* Retention Days */}
        <div className="space-y-1">
          <div className="flex justify-between items-center">
            <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
              Retention period
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold">
              {persistence.retention_days} days
            </span>
          </div>
          <input
            type="range"
            min="1"
            max="365"
            step="1"
            value={persistence.retention_days}
            onChange={(e) => updateDraft("persistence", "retention_days", Number(e.target.value))}
            className="w-full"
          />
        </div>
      </div>
    </div>
  );
});

MemoryCard.displayName = "MemoryCard";
