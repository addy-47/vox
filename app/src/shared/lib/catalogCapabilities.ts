import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";

/**
 * SSOT for the capabilities cache lookup of one model id. The disk cache is
 * keyed by `${provider_kind}:${model_id}` and, for legacy entries, by bare id.
 * A miss is a normal state (the model was never probed), never a crash.
 */
export function resolveCapabilitiesById(
  modelId: string | null | undefined,
  cache?: Record<string, ModelCapabilities>
): ModelCapabilities | undefined {
  if (!modelId) return undefined;
  return (
    cache?.[`server:${modelId}`] ??
    cache?.[`cloud:${modelId}`] ??
    cache?.[`embedded:${modelId}`] ??
    cache?.[modelId]
  );
}

/**
 * Single SSOT for resolving effective probe capabilities for a model.
 * Priority: live probe → model payload → capabilities cache.
 */
export function resolveCapabilities(
  model: LlmModelInfo,
  probingMap: Record<string, { capabilities?: ModelCapabilities }>,
  cache?: Record<string, ModelCapabilities>
): ModelCapabilities | undefined {
  return (
    probingMap[model.id]?.capabilities ??
    model.capabilities ??
    resolveCapabilitiesById(model.id, cache)
  );
}
