/**
 * Canonical mapping dictionary and lookup helper for Realtime provider subkeys.
 * Bridges alias IDs ("gemini", "openai", "deepgram", "elevenlabs")
 * to their concrete configuration subkeys.
 */
export const REALTIME_SUBKEY_MAP: Record<string, string> = {};

/**
 * Passthrough: realtime subkey is the provider id itself (backend-defined).
 */
export function resolveRealtimeSubkey(providerId?: string | null): string {
  if (!providerId) return providerId ?? "";
  return REALTIME_SUBKEY_MAP[providerId] || providerId;
}

export function isRealtimeProviderDisabled(_providerId?: string | null): boolean {
  return false;
}
