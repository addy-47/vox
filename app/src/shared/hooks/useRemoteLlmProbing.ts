import { useState, useCallback, useEffect, useRef } from "react";
import { useCatalogStore } from "@/store/catalogStore";
import { probeModelCapabilities } from "@/services/catalogService";
import type {
  ModelCapabilities,
  LlmProviderConfig,
} from "@/store/settingsStore";

export function useRemoteLlmProbing(
  provider: LlmProviderConfig | null,
  activePipelineTab: string,
  isRemoteLlm: boolean
) {
  const remoteModels = useCatalogStore((s) => s.remoteModels);
  const loadingRemoteModels = useCatalogStore((s) => s.loadingRemoteModels);
  const remoteModelsError = useCatalogStore((s) => s.remoteModelsError);
  const loadRemoteModels = useCatalogStore((s) => s.loadRemoteModels);
  const patchRemoteModelCapabilities = useCatalogStore((s) => s.patchCapabilities);
  const [probingMap, setProbingMap] = useState<Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>>({});
 
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Load disk capabilities cache and model catalog on initial mount
  useEffect(() => {
    if (Object.keys(useCatalogStore.getState().capabilitiesCache).length === 0) {
      void useCatalogStore.getState().loadCapabilitiesCache();
    }
    if (!useCatalogStore.getState().modelCatalog) {
      void useCatalogStore.getState().loadModelCatalog();
    }
  }, []);

  // Removed redundant capabilitiesCache -> probingMap clone effect; resolveCapabilities falls back to cache.

  const fetchRemoteModels = useCallback(async (force = false) => {
    if (!provider || provider.kind === "embedded" || !provider.base_url) return;
    if (provider.kind === "cloud" && !provider.api_key?.trim()) {
      useCatalogStore.setState({
        remoteModels: [],
        remoteModelsError: "API key required. Configure your key in Providers.",
        loadingRemoteModels: false,
      });
      return;
    }
    await loadRemoteModels(provider, force);
  }, [provider, loadRemoteModels]);

  useEffect(() => {
    if (activePipelineTab === "llm" && isRemoteLlm && provider && provider.kind !== "embedded" && provider.base_url) {
      if (provider.kind === "cloud" && !provider.api_key?.trim()) {
        useCatalogStore.setState({
          remoteModels: [],
          remoteModelsError: "API key required. Configure your key in Providers.",
          loadingRemoteModels: false,
        });
        return;
      }
      void fetchRemoteModels();
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
        patchRemoteModelCapabilities(targetId, caps);
      } catch (err) {
        if (!mountedRef.current) return;
        console.error("[CapabilityProbe] Failed to probe model:", err);
        setProbingMap((prev) => ({
          ...prev,
          [targetId]: { status: "error", error: err instanceof Error ? err.message : String(err) },
        }));
      }
    },
    [provider, patchRemoteModelCapabilities]
  );

  return {
    remoteModels,
    loadingRemoteModels,
    remoteModelsError,
    probingMap,
    fetchRemoteModels,
    handleProbeCapabilities,
  };
}
