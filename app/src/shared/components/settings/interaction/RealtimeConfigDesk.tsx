import { memo } from "react";
import { useSettingsStore, RealtimeActiveProvider } from "@/store/settingsStore";
import { REALTIME_PROVIDERS } from "@/data/providersCopy";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField } from "@/shared/ui";

interface RealtimeConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const RealtimeConfigDesk = memo(({ layoutMode }: RealtimeConfigDeskProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  if (!draftSettings) return null;

  const currentProviderId =
    draftSettings.realtime?.active ||
    draftSettings.realtime?.provider ||
    "gemini_live";

  const providerIndex = Math.max(
    0,
    REALTIME_PROVIDERS.findIndex(
      (p) => p.id === currentProviderId || p.subkey === currentProviderId
    )
  );
  const activeProvider = REALTIME_PROVIDERS[providerIndex] || REALTIME_PROVIDERS[0];
  const ActiveIcon = activeProvider.icon;

  const activeSubkey = activeProvider.subkey as "gemini_live" | "deepgram_voice_agent";
  const activeConfig =
    (draftSettings.realtime as any)?.[activeSubkey] ||
    (draftSettings.realtime as any)?.[activeProvider.id] ||
    (draftSettings.realtime as any)?.[activeSubkey === "gemini_live" ? "gemini" : "deepgram"] ||
    {};

  const apiKey = activeConfig.api_key || "";

  const handleApiKeyChange = (key: string) => {
    const updated = {
      ...activeConfig,
      api_key: key,
    };
    updateDraft("realtime", activeSubkey, updated);
  };

  return (
    <div className="flex flex-col gap-1.5 sm:gap-2 w-full mt-1.5 animate-fade-in flex-1">
      {/* ─── Top Ribbon: Live Voice Provider Selector ─── */}
      <div className="flex flex-wrap items-center justify-between gap-y-1.5 gap-x-1 w-full pb-1.5 sm:pb-2 pt-1 shrink-0 px-1.5 sm:px-3">
        {/* Left: Mode Title */}
        <div className="flex items-center gap-1 sm:gap-1.5 shrink-0 pr-0.5 sm:pr-1">
          <div className="p-0.5 sm:p-1 rounded-md text-[rgb(var(--accent))] flex items-center justify-center">
            <Radio size={13} className="shrink-0 sm:w-3.5 sm:h-3.5 animate-pulse" />
          </div>
          <span className="text-[12px] sm:text-[13px] font-black tracking-wider uppercase text-[rgb(var(--accent))] select-none">
            Live Voice
          </span>
        </div>

        {/* Center Connector Arrow */}
        <div className="flex flex-1 items-center px-1 min-w-[8px] pointer-events-none select-none overflow-hidden">
          <svg
            className="w-full h-2.5 sm:h-3 text-[rgb(var(--accent))]/50 overflow-visible"
            viewBox="0 0 100 12"
            preserveAspectRatio="none"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <line
              x1="0"
              y1="6"
              x2="97"
              y2="6"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
            <path
              d="M 92 2.5 L 98.5 6 L 92 9.5"
              stroke="currentColor"
              strokeWidth="1.25"
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        </div>

        {/* Right: Provider Underline Pills */}
        <div className="flex items-center gap-1.5 sm:gap-2.5 shrink-0 pl-0.5 sm:pl-1">
          {REALTIME_PROVIDERS.map((provider, idx, arr) => {
            const isActive = currentProviderId === provider.id || currentProviderId === provider.subkey;
            return (
              <div key={provider.id} className="flex items-center gap-1.5 sm:gap-2.5">
                <button
                  type="button"
                  onClick={() => updateDraft("realtime", "active", provider.id as RealtimeActiveProvider)}
                  className={cn(
                    "flex items-center justify-center gap-1 pb-0.5 sm:pb-1 border-b-2 transition-all duration-200 bg-transparent text-[11px] sm:text-[12px] font-black uppercase tracking-[0.08em] sm:tracking-[0.12em] outline-none cursor-pointer",
                    isActive
                      ? "text-[rgb(var(--accent))] border-[rgb(var(--accent))]"
                      : "text-[rgb(var(--foreground-muted))]/50 border-transparent hover:text-[rgb(var(--foreground-muted))]/80"
                  )}
                >
                  <span>{provider.name}</span>
                </button>
                {idx < arr.length - 1 && (
                  <span className="text-[11px] sm:text-[12px] text-[rgb(var(--foreground-muted))]/20 font-light select-none pb-0.5 sm:pb-1">
                    |
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* ─── Glass Desk: Provider Details & API Key Field ─── */}
      <div
        className={cn(
          "w-full flex flex-col justify-between rounded-xl p-3 border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] flex-1 min-h-[115px]",
          layoutMode === "small" ? "h-auto py-3 space-y-3" : "justify-between"
        )}
      >
        {/* Active Provider Info Header */}
        <div className="flex items-start justify-between gap-2 border-b border-[rgba(var(--accent),0.08)] pb-2">
          <div className="flex items-start gap-2.5 min-w-0">
            <div className="p-1.5 rounded-lg bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] flex items-center justify-center shrink-0 mt-0.5">
              <ActiveIcon className="w-3.5 h-3.5" active />
            </div>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-[12px] font-bold text-[rgb(var(--foreground))] truncate">
                  {activeProvider.name}
                </span>
                <span className="text-[10px] font-mono font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] uppercase tracking-wider border border-[rgb(var(--accent))]/20 shrink-0">
                  {activeProvider.desc}
                </span>
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-snug mt-1">
                {activeProvider.tagline}
              </p>
            </div>
          </div>
        </div>

        {/* API Key Input Field */}
        <div className="pt-1">
          <ApiKeyField
            label={REALTIME_CONFIG_DESK_COPY.apiKeyLabel}
            value={apiKey}
            onChange={handleApiKeyChange}
            placeholder={activeProvider.keyPlaceholder}
            error={!apiKey?.trim()}
          />
        </div>
      </div>
    </div>
  );
});

RealtimeConfigDesk.displayName = "RealtimeConfigDesk";
