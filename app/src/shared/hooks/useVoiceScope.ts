import { useEffect, useState } from "react";
import { getVoiceScope, type VoiceScopeDto } from "@/services/voiceService";

export type { VoiceScopeDto };

const FALLBACK_SCOPE: VoiceScopeDto = {
  voice_provider: "",
  can_clone: false,
  cloud_sync: false,
};

export interface VoiceScopeState {
  scope: VoiceScopeDto;
  /** True once the backend has resolved this engine (never render unscoped before). */
  ready: boolean;
}

/**
 * Resolves the active TTS engine to its voice catalog scope via the backend
 * (SSOT). The mapping — engine to DB provider filter, `ProviderCaps.clone`,
 * cloud-sync hint — lives in `services/tts/voice.rs` so the frontend never
 * branches on provider id literals (Invariant 6: model agnosticism).
 */
const SCOPE_CACHE = new Map<string, VoiceScopeDto>();

export function useVoiceScope(engine: string | undefined | null): VoiceScopeState {
  const cached = engine ? SCOPE_CACHE.get(engine) : undefined;
  const [scope, setScope] = useState<VoiceScopeDto>(cached ?? FALLBACK_SCOPE);
  const [ready, setReady] = useState(Boolean(cached));
  const [prevEngine, setPrevEngine] = useState(engine);

  if (prevEngine !== engine) {
    setPrevEngine(engine);
    const nowCached = engine ? SCOPE_CACHE.get(engine) : undefined;
    setScope(nowCached ?? FALLBACK_SCOPE);
    setReady(Boolean(nowCached));
  }

  useEffect(() => {
    const key = engine ?? "";
    if (SCOPE_CACHE.has(key)) {
      setScope(SCOPE_CACHE.get(key)!);
      setReady(true);
      return;
    }
    let isMounted = true;
    if (!cached) setReady(false);
    getVoiceScope(engine ?? undefined)
      .then((s) => {
        SCOPE_CACHE.set(key, s);
        if (isMounted) {
          setScope(s);
          setReady(true);
        }
      })
      .catch(() => {
        if (isMounted) {
          setScope(FALLBACK_SCOPE);
          setReady(true);
        }
      });
    return () => {
      isMounted = false;
    };
  }, [engine, cached]);

  return { scope, ready };
}
