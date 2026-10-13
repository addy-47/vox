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
export function useVoiceScope(engine: string | undefined | null): VoiceScopeState {
  const [scope, setScope] = useState<VoiceScopeDto>(FALLBACK_SCOPE);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let isMounted = true;
    setReady(false);
    getVoiceScope(engine ?? undefined)
      .then((s) => {
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
  }, [engine]);

  return { scope, ready };
}
