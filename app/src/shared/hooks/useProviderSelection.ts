import { useState, useMemo, useCallback, useEffect } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useShallow } from "zustand/react/shallow";
import { activeSelection, getEffectiveCloudKeyMap, getEffectiveRealtimeApiKey, resolveProviderModel } from "@/shared/lib/activeSelection";
import { requestSpeechTab } from "@/shared/lib/speechTabSync";
import type { CloudProvider } from "@/data/providersCopy";

export interface UseProviderSelectionOptions {
  category?: "STT" | "LLM" | "TTS";
  onNavigateToModels?: (provider: CloudProvider) => void;
  onSelectProvider?: (provider: CloudProvider) => void;
}

export function useProviderSelection(options?: UseProviderSelectionOptions) {
  const category = options?.category ?? "LLM";
  const isTts = category === "TTS";
  const isStt = category === "STT";

  const draftSettings = useSettingsStore(useShallow((s) => s.draftSettings));
  const committedSettings = useSettingsStore(useShallow((s) => s.settings));
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState<string>("");

  const activeCategoryType = isTts
    ? (draftSettings?.tts?.active?.category ?? "embedded")
    : isStt
    ? (draftSettings?.stt?.active?.category ?? "embedded")
    : (draftSettings?.llm?.active?.category ?? "embedded");

  const activeCloudProviderId = useMemo(() => {
    if (activeCategoryType !== "cloud") return null;
    if (isTts) {
      return draftSettings?.tts?.active?.provider ?? draftSettings?.tts?.active?.model ?? null;
    }
    if (isStt) {
      return draftSettings?.stt?.active?.provider ?? draftSettings?.stt?.active?.model ?? null;
    }
    return draftSettings?.llm?.active?.provider ?? null;
  }, [
    activeCategoryType,
    isTts,
    isStt,
    draftSettings?.tts?.active?.provider,
    draftSettings?.tts?.active?.model,
    draftSettings?.stt?.active?.provider,
    draftSettings?.stt?.active?.model,
    draftSettings?.llm?.active?.provider,
  ]);

  const getProviderKey = useCallback(
    (providerId: string) => {
      const isRealtime = draftSettings?.interaction.pipeline_mode === "realtime";
      if (isRealtime) {
        return getEffectiveRealtimeApiKey(draftSettings, committedSettings, providerId) || "";
      }
      if (isTts) {
        type TtsRuntime = {
          tts?: Record<string, { key?: string }>;
          providers?: { tts?: Record<string, { key?: string }> };
        };
        const ds = draftSettings as TtsRuntime | null;
        return ds?.tts?.[providerId]?.key ?? ds?.providers?.tts?.[providerId]?.key;
      }
      const keys = draftSettings?.llm?.cloud_keys ?? draftSettings?.providers?.llm?.cloud_keys;
      return keys?.[providerId];
    },
    [isTts, draftSettings, committedSettings]
  );

  // Save-gated ordering: the sort follows committed keys so selecting only
  // re-highlights; the reorder (with smooth scroll) lands on save.
  const pinnedProviderId = useMemo(() => {
    if (isTts) {
      if (committedSettings?.tts?.active?.category !== "cloud") return undefined;
      return committedSettings?.tts?.active?.provider ?? committedSettings?.tts?.active?.model ?? undefined;
    }
    if (isStt) {
      if (committedSettings?.stt?.active?.category !== "cloud") return undefined;
      return committedSettings?.stt?.active?.provider ?? committedSettings?.stt?.active?.model ?? undefined;
    }
    if (committedSettings?.llm?.active?.category !== "cloud") return undefined;
    return committedSettings?.llm?.active?.provider ?? undefined;
  }, [
    isTts,
    isStt,
    committedSettings?.tts?.active?.category,
    committedSettings?.tts?.active?.provider,
    committedSettings?.tts?.active?.model,
    committedSettings?.stt?.active?.category,
    committedSettings?.stt?.active?.provider,
    committedSettings?.stt?.active?.model,
    committedSettings?.llm?.active?.category,
    committedSettings?.llm?.active?.provider,
  ]);

  const getCommittedProviderKey = useCallback(
    (providerId: string) => {
      const isRealtime = committedSettings?.interaction.pipeline_mode === "realtime";
      if (isRealtime) {
        return getEffectiveRealtimeApiKey(null, committedSettings, providerId) || "";
      }
      if (isTts) {
        type Ct = { providers?: { tts?: Record<string, { key?: string }> } };
        return (committedSettings as Ct | null)?.providers?.tts?.[providerId]?.key;
      }
      const keys = committedSettings?.providers?.llm?.cloud_keys ?? committedSettings?.llm?.cloud_keys;
      return keys?.[providerId];
    },
    [isTts, committedSettings]
  );

  useEffect(() => {
    return () => {
      useCatalogStore.getState().clearRemoteModelsError();
    };
  }, []);

  const handleSelectProvider = useCallback(
    (provider: CloudProvider) => {
      useCatalogStore.getState().clearRemoteModelsError();
      if (isTts) {
        // A TTS cloud engine's model id IS the engine/provider id.
        updateDraft(
          "tts",
          "active",
          activeSelection(
            "cloud",
            resolveProviderModel(
              { category: "cloud", provider: provider.id },
              committedSettings?.tts?.active ?? null,
              provider.id
            ),
            provider.id
          )
        );
        requestSpeechTab("tts");
      } else if (isStt) {
        updateDraft(
          "stt",
          "active",
          activeSelection(
            "cloud",
            resolveProviderModel(
              { category: "cloud", provider: provider.id },
              committedSettings?.stt?.active ?? null
            ),
            provider.id
          )
        );
        requestSpeechTab("stt");
      } else {
        const isRealtime = draftSettings?.interaction.pipeline_mode === "realtime";
        if (isRealtime) {
          const curModel = draftSettings?.realtime.active.model;
          updateDraft("realtime", "active", {
            category: "cloud",
            provider: provider.id,
            model: curModel || (provider.id === "gemini_live" ? "gemini-2.5-flash" : "gpt-5.2"),
          });
        } else {
          // Never carry the previous provider's model across the switch: it is
          // what left `llm.active` permanently dirty after reverting.
          updateDraft(
            "llm",
            "active",
            activeSelection(
              "cloud",
              resolveProviderModel(
                { category: "cloud", provider: provider.id },
                committedSettings?.llm?.active ?? null
              ),
              provider.id
            )
          );
        }
      }
      options?.onSelectProvider?.(provider);
    },
    [
      isTts,
      isStt,
      committedSettings,
      updateDraft,
      options,
      draftSettings?.interaction.pipeline_mode,
      draftSettings?.realtime,
    ]
  );

  const isLocalActive = isTts
    ? activeCategoryType !== "cloud" && activeCategoryType !== "server"
    : activeCategoryType === "embedded";

  const handleSelectLocal = useCallback(() => {
    useCatalogStore.getState().clearRemoteModelsError();
    if (isTts) {
      updateDraft(
        "tts",
        "active",
        activeSelection(
          "embedded",
          resolveProviderModel(
            { category: "embedded", provider: null },
            committedSettings?.tts?.active ?? null,
            draftSettings?.tts?.last_selected?.embedded ?? ""
          )
        )
      );
      requestSpeechTab("tts");
    } else if (isStt) {
      updateDraft(
        "stt",
        "active",
        activeSelection(
          "embedded",
          resolveProviderModel(
            { category: "embedded", provider: null },
            committedSettings?.stt?.active ?? null,
            draftSettings?.stt?.last_selected?.embedded ?? ""
          )
        )
      );
      requestSpeechTab("stt");
    } else {
      updateDraft(
        "llm",
        "active",
        activeSelection(
          "embedded",
          resolveProviderModel(
            { category: "embedded", provider: null },
            committedSettings?.llm?.active ?? null,
            draftSettings?.llm?.last_selected?.embedded ?? ""
          )
        )
      );
    }
  }, [
    isTts,
    isStt,
    committedSettings?.tts?.active,
    committedSettings?.stt?.active,
    committedSettings?.llm?.active,
    draftSettings?.tts?.last_selected?.embedded,
    draftSettings?.stt?.last_selected?.embedded,
    draftSettings?.llm?.last_selected?.embedded,
    updateDraft,
  ]);

  const handleStartInlineEdit = useCallback((id: string, currentKey: string) => {
    setEditingProviderId(id);
    setEditingKeyValue(currentKey);
  }, []);

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
  }, []);

  const handleSaveInlineKey = useCallback(
    async (providerId: string) => {
      const trimmed = editingKeyValue.trim();
      const isRealtime = draftSettings?.interaction.pipeline_mode === "realtime";
      if (isRealtime) {
        const currentKeys = {
          ...(draftSettings?.providers?.realtime?.api_keys ?? draftSettings?.realtime?.api_keys ?? {}),
        };
        if (trimmed) {
          currentKeys[providerId] = trimmed;
        } else {
          delete currentKeys[providerId];
        }
        updateDraft("realtime", "api_keys", currentKeys);
        if (providerId === "gemini_live") {
          const cloudKeys = getEffectiveCloudKeyMap(draftSettings, committedSettings);
          if (trimmed) cloudKeys.gemini = trimmed;
          else delete cloudKeys.gemini;
          updateDraft("llm", "cloud_keys", cloudKeys);
        }
        await useSettingsStore.getState().commitChanges({ domain: "realtime", key: "api_keys" });
      } else if (isTts) {
        type Rt = { tts?: Record<string, Record<string, unknown>>; providers?: { tts?: Record<string, Record<string, unknown>> } };
        const rt = draftSettings as Rt | null;
        const base = (rt?.tts?.[providerId] ?? rt?.providers?.tts?.[providerId] ?? {
          region_or_endpoint: "eastus",
          key: "",
          voice: "",
        }) as Record<string, unknown>;
        const current = { ...base };
        current.key = trimmed;
        (updateDraft as (a: string, b: string, c: unknown) => void)("tts", providerId, current);
        await (useSettingsStore.getState().commitChanges as (x: { domain: string; key: string }) => Promise<void>)({ domain: "tts", key: providerId });
      } else {
        const currentKeys = getEffectiveCloudKeyMap(draftSettings, committedSettings);
        if (trimmed) {
          currentKeys[providerId] = trimmed;
        } else {
          delete currentKeys[providerId];
        }

        updateDraft("llm", "cloud_keys", currentKeys);
        await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });
      }

      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [
      isTts,
      editingKeyValue,
      draftSettings,
      committedSettings,
      updateDraft,
    ]
  );

  return {
    category,
    activeCloudProviderId,
    isLocalActive,
    onSelectLocal: handleSelectLocal,
    getProviderKey,
    pinnedProviderId,
    getCommittedProviderKey,
    onSelectProvider: handleSelectProvider,
    editingProviderId,
    editingKeyValue,
    setEditingKeyValue,
    onStartInlineEdit: handleStartInlineEdit,
    onCancelInlineKey: handleCancelInlineKey,
    onSaveInlineKey: handleSaveInlineKey,
    onNavigateToModels: options?.onNavigateToModels,
  };
}
