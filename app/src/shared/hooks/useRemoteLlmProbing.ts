import { useState, useCallback, useEffect, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  probeModelCapabilities,
  listLlmModels,
} from "@/services/settingsService";
import type { LlmModelInfo, ModelCapabilities, LlmProviderConfig } from "@/store/settingsStore";

export function useRemoteLlmProbing(
  provider: LlmProviderConfig | null,
  activePipelineTab: string,
  isRemoteLlm: boolean
) {
  const remoteModels = useSettingsStore((s) => s.remoteModels);
  const loadingRemoteModels = useSettingsStore((s) => s.loadingRemoteModels);
  const remoteModelsError = useSettingsStore((s) => s.remoteModelsError);
  const loadRemoteModels = useSettingsStore((s) => s.loadRemoteModels);
  const [probingMap, setProbingMap] = useState<Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>>({});
  const [customModelId, setCustomModelId] = useState("");
  const [customModelStatus, setCustomModelStatus] = useState<'idle' | 'checking' | 'valid' | 'invalid'>('idle');

  const capabilitiesCache = useSettingsStore((s) => s.capabilitiesCache);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  // Load disk capabilities cache and model catalog on initial mount
  useEffect(() => {
    useSettingsStore.getState().loadCapabilitiesCache();
    if (!useSettingsStore.getState().modelCatalog) {
      useSettingsStore.getState().loadModelCatalog();
    }
  }, []);

  // Sync disk capabilities cache into component state on mount / update
  useEffect(() => {
    if (capabilitiesCache && Object.keys(capabilitiesCache).length > 0) {
      setProbingMap((prev) => {
        const next = { ...prev };
        for (const [_, caps] of Object.entries(capabilitiesCache)) {
          const mId = caps.model_id;
          if (!next[mId] || next[mId].status === "idle") {
            next[mId] = { status: "success", capabilities: caps };
          }
        }
        return next;
      });
    }
  }, [capabilitiesCache]);

  const fetchRemoteModels = useCallback(async (force = false) => {
    if (!provider || provider.kind === "embedded" || !provider.base_url) return;
    await loadRemoteModels(provider, force);
  }, [provider, loadRemoteModels]);

  useEffect(() => {
    if (activePipelineTab === "llm" && isRemoteLlm && provider && provider.kind !== "embedded" && provider.base_url) {
      fetchRemoteModels();
    }
  }, [activePipelineTab, isRemoteLlm, provider, fetchRemoteModels]);

  const handleProbeCapabilities = useCallback(
    async (modelId?: string) => {
      if (!provider) return;
      const targetId = modelId || (provider.kind !== "embedded" ? provider.model : "embedded");
      if (!targetId) return;

      setProbingMap((prev) => ({
        ...prev,
        [targetId]: { status: "testing" },
      }));

      try {
        const caps = await probeModelCapabilities(provider, targetId);
        // Unmount-guarded (no seq gate here on purpose: concurrent probes for
        // different models must all commit; a global seq would strand one in
        // "testing" forever).
        if (!mountedRef.current) return;
        setProbingMap((prev) => ({
          ...prev,
          [targetId]: { status: "success", capabilities: caps },
        }));
        setRemoteModels((prev) =>
          prev.map((m) => (m.id === targetId ? { ...m, capabilities: caps } : m))
        );
        useSettingsStore.setState((state) => ({
          capabilitiesCache: {
            ...state.capabilitiesCache,
            [`${caps.provider_kind}:${caps.model_id}`]: caps,
          },
        }));
      } catch (err) {
        if (!mountedRef.current) return;
        console.error("[CapabilityProbe] Failed to probe model:", err);
        setProbingMap((prev) => ({
          ...prev,
          [targetId]: { status: "error", error: String(err) },
        }));
      }
    },
    [provider]
  );

  const handleValidateCustomModel = async () => {
    if (!customModelId.trim() || !provider) return;
    setCustomModelStatus("checking");
    const mId = customModelId.trim();
    const draft = useSettingsStore.getState().draftSettings;
    const activeLlm = provider.kind !== "embedded" ? provider.kind : (draft?.llm?.active || "embedded");

    try {
      const caps = await probeModelCapabilities(provider, mId);
      if (!mountedRef.current) return;
      if (activeLlm === "server" && draft?.llm?.server) {
        updateDraft("llm", "server", { ...draft.llm.server, model: mId });
      } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
        updateDraft("llm", "cloud", { ...draft.llm.cloud, model: mId });
      }
      if (provider && "base_url" in provider) {
        updateDraft("llm", "provider", { ...provider, model: mId });
      }
      updateDraft("llm", "model", mId);
      setProbingMap((prev) => ({
        ...prev,
        [mId]: { status: "success", capabilities: caps },
      }));
      setCustomModelStatus("valid");
    } catch (_) {
      if (!mountedRef.current) return;
      if (activeLlm === "server" && draft?.llm?.server) {
        updateDraft("llm", "server", { ...draft.llm.server, model: mId });
      } else if (activeLlm === "cloud" && draft?.llm?.cloud) {
        updateDraft("llm", "cloud", { ...draft.llm.cloud, model: mId });
      }
      if (provider && "base_url" in provider) {
        updateDraft("llm", "provider", { ...provider, model: mId });
      }
      updateDraft("llm", "model", mId);
      setCustomModelStatus("invalid");
    }
  };

  return {
    remoteModels,
    loadingRemoteModels,
    remoteModelsError,
    probingMap,
    customModelId,
    setCustomModelId,
    customModelStatus,
    fetchRemoteModels,
    handleProbeCapabilities,
    handleValidateCustomModel,
  };
}
