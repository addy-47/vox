import { memo, useCallback } from "react";
import { useSettingsStore, RealtimeActiveProvider } from "@/store/settingsStore";
import { REALTIME_PROVIDERS } from "@/data/providersCopy";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ApiKeyField, CarouselSelector } from "@/shared/ui";

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

  const handleProviderCycle = useCallback(
    (direction: "left" | "right") => {
      const delta = direction === "right" ? 1 : -1;
      const nextIndex =
        (providerIndex + delta + REALTIME_PROVIDERS.length) % REALTIME_PROVIDERS.length;
      updateDraft(
        "realtime",
        "active",
        REALTIME_PROVIDERS[nextIndex].id as RealtimeActiveProvider
      );
    },
    [providerIndex, updateDraft]
  );

  return (
    <div className="flex flex-col justify-between w-full mt-0.5 animate-fade-in flex-1">
      {/* ─── Clean Mode Title Banner with Subtext & Highlights ─── */}
      <div className="flex flex-col gap-1 px-1 shrink-0">
        <div className="flex items-center gap-1.5">
          <Radio size={14} className="text-[rgb(var(--accent))] animate-pulse shrink-0" />
          <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">
            Realtime Direct Voice Connection
          </span>
        </div>
        <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-relaxed">
          Full duplex speech-to-speech engine with{" "}
          <span className="text-[rgb(var(--accent))] font-semibold">sub-200ms</span> live audio streaming, native grounding, and dynamic turn detection.
        </p>
      </div>

      {/* ─── 2-Column Grid on Root Layout: Provider Carousel on Left, API Key on Right ─── */}
      <div
        className={cn(
          "grid gap-5 flex-1 min-h-0 items-end px-1 pb-1 pt-3",
          layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
        )}
      >
        {/* Left Column: Clean Borderless Provider Carousel */}
        <CarouselSelector
          label="Voice Provider"
          value={activeProvider.name}
          onPrev={() => handleProviderCycle("left")}
          onNext={() => handleProviderCycle("right")}
        />

        {/* Right Column: API Key Input Field */}
        <ApiKeyField
          label={REALTIME_CONFIG_DESK_COPY.apiKeyLabel}
          value={apiKey}
          onChange={handleApiKeyChange}
          placeholder={activeProvider.keyPlaceholder}
          error={!apiKey?.trim()}
        />
      </div>
    </div>

  );
});

RealtimeConfigDesk.displayName = "RealtimeConfigDesk";


