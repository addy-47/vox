import { memo, useCallback, useState } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { REALTIME_PROVIDERS } from "@/data/providersCopy";
import { REALTIME_CONFIG_DESK_COPY } from "@/data/settingsCopy";
import { Radio, Check, X, Pencil } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ProviderLogo } from "@/shared/components/settings/providers/ProviderLogo";

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

  /** API keys: draft vehicle first, then the committed providers file. */
  const realtimeKeys =
    draftSettings?.realtime?.api_keys ?? draftSettings?.providers?.realtime?.api_keys ?? {};

  const getProviderKey = useCallback(
    (providerId: string) => {
      return realtimeKeys[providerId] || "";
    },
    [realtimeKeys]
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
      const currentKeys = { ...realtimeKeys };
      const trimmed = editingKeyValue.trim();
      if (trimmed) {
        currentKeys[providerId] = trimmed;
      } else {
        delete currentKeys[providerId];
      }
      updateDraft("realtime", "api_keys", currentKeys, "interaction");
      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [realtimeKeys, editingKeyValue, updateDraft]
  );

  if (!draftSettings) return null;

  return (
    <div className="flex flex-col w-full mt-0.5 animate-fade-in flex-1 min-h-0">
      {/* ─── Clean Mode Title Banner ─── */}
      <div className="flex items-center gap-1.5 px-1 shrink-0 pb-2">
        <Radio size={14} className="text-[rgb(var(--accent))] animate-pulse shrink-0" />
        <span className="text-[13px] font-bold text-[rgb(var(--foreground))]">
          {REALTIME_CONFIG_DESK_COPY.title}
        </span>
      </div>

      {/* ─── 2-Column Grid: Sleek Horizontal Cards ─── */}
      <div
        className={cn(
          "grid gap-2.5 content-start w-full px-1",
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
                "group flex items-center gap-2.5 px-3 py-2.5 rounded-xl transition-all cursor-pointer select-none border min-h-[52px]",
                isSelected
                  ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                  : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
              )}
            >
              <div
                className={cn(
                  "w-7 h-7 rounded-lg flex items-center justify-center shrink-0 border transition-all",
                  isSelected
                    ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]"
                    : "bg-[rgba(var(--foreground),0.03)] border-[rgba(var(--foreground),0.06)]"
                )}
              >
                <ProviderLogo providerId={provider.id} size={16} className="shrink-0" />
              </div>

              {isEditing ? (
                <div
                  className="flex-1 flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in min-w-0"
                  onClick={(e) => e.stopPropagation()}
                >
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
                <>
                  <div className="flex flex-col min-w-0 flex-1 justify-center">
                    <span
                      className={cn(
                        "text-[12px] truncate leading-tight",
                        isSelected
                          ? "font-semibold text-[rgb(var(--foreground))]"
                          : "font-medium text-[rgb(var(--foreground))]/90"
                      )}
                    >
                      {provider.name}
                    </span>
                    <span className="text-[10px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight font-sans">
                      {provider.tagline}
                    </span>
                  </div>

                  <div className="shrink-0 flex items-center gap-1.5 ml-auto" onClick={(e) => e.stopPropagation()}>
                    {hasKey ? (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, savedKey)}
                        className="px-1.5 py-0.5 rounded text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer flex items-center gap-1"
                        title="Edit API Key"
                        aria-label="Edit API Key"
                      >
                        <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                        <span>••••{savedKey.slice(-4)}</span>
                        <Pencil size={9} className="opacity-60" />
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleStartInlineEdit(provider.id, "")}
                        className="px-2 py-0.5 rounded-md text-[10px] font-medium text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--accent),0.1)] transition-all cursor-pointer"
                      >
                        + Key
                      </button>
                    )}

                    {isSelected && (
                      <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0">
                        <Check size={9} strokeWidth={2.5} />
                      </div>
                    )}
                  </div>
                </>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

RealtimeConfigDesk.displayName = "RealtimeConfigDesk";
