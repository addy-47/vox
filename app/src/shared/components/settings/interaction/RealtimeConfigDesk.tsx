import { memo, useCallback, useState } from "react";
import { useSettingsStore, type RealtimeSubkey } from "@/store/settingsStore";
import { REALTIME_PROVIDERS } from "@/data/providersCopy";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { Radio, Check, X, Pencil } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ProviderLogo } from "@/shared/components/settings/models/ProviderLogo";
import { resolveRealtimeSubkey } from "@/shared/lib/realtimeProviders";

interface RealtimeConfigDeskProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const RealtimeConfigDesk = memo(({ layoutMode }: RealtimeConfigDeskProps) => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState("");

  const currentProviderId =
    draftSettings?.realtime?.active?.provider || "gemini_live";

  const getProviderKey = useCallback(
    (providerId: string) => {
      const subkey = resolveRealtimeSubkey(providerId);
      const conf = draftSettings?.realtime?.[subkey];
      return conf && "api_key" in conf ? (conf as { api_key?: string }).api_key || "" : "";
    },
    [draftSettings?.realtime]
  );

  const handleSelectProvider = useCallback(
    (providerId: string) => {
      updateDraft(
        "realtime",
        "active",
        {
          category: "cloud",
          provider: providerId,
          model: draftSettings?.realtime?.active?.model ?? "",
        },
        "interaction"
      );
    },
    [draftSettings?.realtime?.active?.model, updateDraft]
  );

  const handleStartInlineEdit = useCallback(
    (providerId: string, currentKey: string) => {
      setEditingProviderId(providerId);
      setEditingKeyValue(currentKey);
    },
    []
  );

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
  }, []);

  const handleSaveInlineKey = useCallback(
    (providerId: string) => {
      const subkey = resolveRealtimeSubkey(providerId);
      const currentConf = draftSettings?.realtime?.[subkey] || {};
      const updated = {
        ...currentConf,
        api_key: editingKeyValue.trim(),
      };
      updateDraft("realtime", subkey as RealtimeSubkey, updated, "interaction");
      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [draftSettings?.realtime, editingKeyValue, updateDraft]
  );

  if (!draftSettings) return null;

  return (
    <div className="flex flex-col w-full mt-0.5 animate-fade-in flex-1 min-h-0">
      {/* ─── Clean Mode Title Banner ─── */}
      <div className="flex items-center gap-1.5 px-1 shrink-0 pb-1.5">
        <Radio size={14} className="text-[rgb(var(--accent))] animate-pulse shrink-0" />
        <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">
          {REALTIME_CONFIG_DESK_COPY.title}
        </span>
      </div>

      {/* ─── 2-Column Grid: Same Provider Cards as Modal ─── */}
      <div
        className={cn(
          "grid gap-2.5 flex-1 min-h-0 overflow-y-auto custom-scrollbar overscroll-contain px-1 py-1 pb-4",
          layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
        )}
      >
        {REALTIME_PROVIDERS.map((provider) => {
          const isSelected =
            currentProviderId === provider.id || currentProviderId === provider.subkey;
          const savedKey = getProviderKey(provider.id);
          const isEditing = editingProviderId === provider.id;
          const hasKey = Boolean(savedKey?.trim());

          return (
            <div
              key={provider.id}
              onClick={() => {
                if (!isEditing) {
                  handleSelectProvider(provider.id);
                }
              }}
              className={cn(
                "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]",
                isSelected
                  ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30"
                  : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)]"
              )}
            >
              {isSelected && (
                <div className="absolute top-3 right-3 w-5 h-5 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] z-10">
                  <Check size={12} strokeWidth={2.5} />
                </div>
              )}

              <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
                <div
                  className={cn(
                    "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
                    isSelected
                      ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
                      : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
                  )}
                >
                  <ProviderLogo providerId={provider.id} size={24} />
                </div>
                <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
                  {provider.name}
                </h4>
                <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
                  {provider.tagline}
                </p>
              </div>

              <div
                className="mt-auto pt-2.5 border-t border-[rgba(var(--foreground),0.06)] h-[28px] flex items-center justify-between"
                onClick={(e) => e.stopPropagation()}
              >
                {isEditing ? (
                  <div className="flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 w-full bg-transparent animate-fade-in">
                    <input
                      type="password"
                      autoFocus
                      value={editingKeyValue}
                      onChange={(e) => setEditingKeyValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleSaveInlineKey(provider.id);
                        if (e.key === "Escape") handleCancelInlineKey();
                      }}
                      placeholder={provider.keyPlaceholder}
                      className="w-full bg-transparent text-[11px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30"
                    />
                    <button
                      type="button"
                      onClick={() => handleSaveInlineKey(provider.id)}
                      title="Save API Key"
                      aria-label="Save API Key"
                      className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0 bg-transparent cursor-pointer"
                    >
                      <Check size={13} strokeWidth={2.2} />
                    </button>
                    <button
                      type="button"
                      onClick={handleCancelInlineKey}
                      title="Cancel"
                      aria-label="Cancel"
                      className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 bg-transparent cursor-pointer"
                    >
                      <X size={13} />
                    </button>
                  </div>
                ) : (
                  <div className="flex items-center justify-between text-[11px] font-mono w-full">
                    <span className="text-[rgb(var(--foreground-muted))]/70">Realtime Voice</span>
                    {hasKey ? (
                      <div className="flex items-center gap-1.5">
                        <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" />
                        <span className="text-[rgb(var(--foreground-muted))]">
                          ••••••••{savedKey?.slice(-4)}
                        </span>
                        <button
                          type="button"
                          onClick={() => handleStartInlineEdit(provider.id, savedKey ?? "")}
                          className="p-1 rounded hover:bg-white/[0.08] hover:text-[rgb(var(--accent))] transition-colors text-[rgb(var(--foreground-muted))]/60 cursor-pointer shrink-0"
                          title="Edit API Key"
                          aria-label="Edit API Key"
                        >
                          <Pencil size={11} />
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, "")}
                        className="text-[rgb(var(--accent))] hover:underline cursor-pointer font-medium flex items-center gap-1"
                      >
                        + Connect Key
                      </button>
                    )}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
});

RealtimeConfigDesk.displayName = "RealtimeConfigDesk";
