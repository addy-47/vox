/**
 * Canonical mapping dictionary and lookup helper for Realtime provider subkeys.
 * Bridges alias IDs ("gemini", "openai", "deepgram", "elevenlabs")
 * to their concrete configuration subkeys.
 */

export const REALTIME_SUBKEY_MAP: Record<string, string> = {
  gemini_live: "gemini_live",
  gemini: "gemini_live",
  openai_realtime: "openai_realtime",
  openai: "openai_realtime",
  deepgram_voice_agent: "deepgram_voice_agent",
  deepgram: "deepgram_voice_agent",
  elevenlabs_convai: "elevenlabs_convai",
  elevenlabs: "elevenlabs_convai",
};

/**
 * Resolves any provider id or alias into a canonical realtime configuration subkey.
 * Defaults safely to "gemini_live".
 */
export function resolveRealtimeSubkey(providerId?: string | null): string {
  if (!providerId) return "gemini_live";
  return REALTIME_SUBKEY_MAP[providerId] || "gemini_live";
}

/**
 * Checks if a realtime provider is currently enabled in production.
 * Currently only Gemini Live and Deepgram Voice Agent are operational.
 */
export function isRealtimeProviderDisabled(providerId?: string | null): boolean {
  return (
    providerId !== "gemini_live" &&
    providerId !== "deepgram_voice_agent"
  );
}
