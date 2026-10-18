import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";

/**
 * Single SSOT for resolving effective probe capabilities for a model.
 * Priority: live probe → model payload → capabilities cache
 * (`server:` → `cloud:` → `embedded:` → bare id).
 */
export function resolveCapabilities(
  model: LlmModelInfo,
  probingMap: Record<string, { capabilities?: ModelCapabilities }>,
  cache?: Record<string, ModelCapabilities>
): ModelCapabilities | undefined {
  return (
    probingMap[model.id]?.capabilities ??
    model.capabilities ??
    cache?.[`server:${model.id}`] ??
    cache?.[`cloud:${model.id}`] ??
    cache?.[`embedded:${model.id}`] ??
    cache?.[model.id]
  );
}
