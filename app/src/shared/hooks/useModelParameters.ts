import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { updateModelParameters } from "@/services/settingsService";

/** Debounce for coalescing rapid parameter edits (knob drags, typed input). */
const PARAM_WRITE_DEBOUNCE_MS = 500;

/**
 * Reads and writes one per-model parameter override from the `models` table.
 * Values render from the backend catalog snapshot merged with pending edits;
 * writes debounce and refresh the catalog so the UI shows persisted state.
 * A null model id disables writes (controls render fallbacks instead).
 */
export function useModelParameters(modelId: string | null | undefined) {
  const catalogParams = useSettingsStore(
    (s) => modelId ? s.modelCatalog?.model_parameters?.[modelId] : undefined
  );
  const loadModelCatalog = useSettingsStore((s) => s.loadModelCatalog);
  const [pending, setPending] = useState<Record<string, unknown>>({});
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingRef = useRef<Record<string, unknown>>({});
  const modelRef = useRef(modelId);

  useEffect(() => {
    modelRef.current = modelId;
  }, [modelId]);

  const flush = useCallback(async () => {
    const target = modelRef.current;
    const patch = pendingRef.current;
    pendingRef.current = {};
    if (!target || Object.keys(patch).length === 0) return;
    try {
      await updateModelParameters(target, patch);
      console.info(`[Providers] modelParameters ok model=${target} keys=${Object.keys(patch).join(",")}`);
      await loadModelCatalog();
    } catch (err) {
      console.error(`[Providers] modelParameters failed model=${target}:`, err);
    }
  }, [loadModelCatalog]);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
      timer.current = null;
      void flush();
    };
  }, [flush]);

  const setParam = useCallback(
    (key: string, value: unknown) => {
      if (!modelRef.current) return;
      pendingRef.current = { ...pendingRef.current, [key]: value };
      setPending({ ...pendingRef.current });
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        timer.current = null;
        setPending({});
        void flush();
      }, PARAM_WRITE_DEBOUNCE_MS);
    },
    [flush]
  );

  const params = useMemo(
    () => ({ ...(catalogParams ?? {}), ...pending }),
    [catalogParams, pending]
  );

  return { params, setParam };
}
