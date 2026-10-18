import { useCallback, useEffect, useRef, useState } from "react";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import { useSettingsStore } from "@/store/settingsStore";
import {
  listVoices,
  recordVoiceUsed,
  syncRemoteVoices,
  synthesizeVoicePreview,
  toggleVoiceFavorite,
  voicePreviewUrl,
  getVoiceScope,
  type VoiceScopeDto,
  type UnifiedVoiceDto,
} from "@/services/voicesService";

export type { VoiceScopeDto };

export interface UseVoiceCatalogOptions {
  provider?: string;
  /**
   * When true, no fetch runs and the list stays empty. Used to hold the
   * catalog until backend scope resolution lands, so another engine's (or
   * all providers') voices are never rendered in between.
   */
  suspended?: boolean;
  /**
   * Currently selected TTS engine in the settings draft. Threaded into
   * preview synthesis so the backend previews with the modal context,
   * not the last saved engine.
   */
  engine?: string;
  /** True when the active engine family syncs voices remotely (e.g. cloud_sync). */
  cloudSync?: boolean;
}


export interface UseVoiceCatalogResult {
  voices: UnifiedVoiceDto[];
  loading: boolean;
  error: string | null;
  reload: () => void;
  stagedId: string | null;
  stageVoice: (id: string) => void;
  commitVoice: (id: string) => void;
  toggleFavorite: (id: string) => void;
  playingId: string | null;
  isSynthesizing: boolean;
  previewProgress: number;
  previewCurrentTime: number;
  previewDuration: number;
  previewPrompt: string;
  setPreviewPrompt: (prompt: string) => void;
  togglePreview: (voice: UnifiedVoiceDto) => void;
  seekPreview: (fraction: number) => void;
  stopPreview: () => void;
}

const DEFAULT_PREVIEW_PROMPT =
  "Vox is an on-device personal voice AI that listens and speaks naturally.";

const VOICES_CACHE = new Map<string, UnifiedVoiceDto[]>();

/**
 * Detach every media listener from an <audio> element before tearing it down.
 * Without this, clearing the source fires a synthetic `error` event that
 * re-enters the stale handler and tears down whichever preview is current —
 * which killed a just-started preview and forced a second click.
 */
function detachAudioHandlers(audio: HTMLAudioElement): void {
  audio.onloadeddata = null;
  audio.onloadedmetadata = null;
  audio.onplay = null;
  audio.onended = null;
  audio.onerror = null;
  audio.ontimeupdate = null;
}

/**
 * Unified voice catalog state: Turso-backed list, draft staging, favorites,
 * on-demand preview synthesis, and interactive playback scrub bar.
 */
export function useVoiceCatalog(
  options: UseVoiceCatalogOptions = {}
): UseVoiceCatalogResult {
  const { provider, suspended = false, engine, cloudSync = false } = options;
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const draftVoiceId = useSettingsStore((s) => s.draftSettings?.tts?.active_voice_id);

  const cachedVoices = provider ? VOICES_CACHE.get(provider) : undefined;
  const [voices, setVoices] = useState<UnifiedVoiceDto[]>(cachedVoices ?? []);
  const [loading, setLoading] = useState(!cachedVoices);
  const [error, setError] = useState<string | null>(null);
  const [stagedId, setStagedId] = useState<string | null>(null);

  const [playingId, setPlayingId] = useState<string | null>(null);
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const [previewProgress, setPreviewProgress] = useState(0);
  const [previewCurrentTime, setPreviewCurrentTime] = useState(0);
  const [previewDuration, setPreviewDuration] = useState(0);
  const [previewPrompt, setPreviewPrompt] = useState(DEFAULT_PREVIEW_PROMPT);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const currentBlobUrlRef = useRef<string | null>(null);
  const activeVoiceRef = useRef<string | null>(null);
  const remoteSyncedRef = useRef(false);

  const reload = useCallback(() => {
    let isMounted = true;
    setLoading(true);
    setError(null);
    if (provider && VOICES_CACHE.has(provider)) {
      setVoices(VOICES_CACHE.get(provider)!);
    } else {
      setVoices([]);
    }
    listVoices(provider)
      .then((list) => {
        if (!isMounted) return;
        const target = provider?.toLowerCase();
        const filtered = target
          ? list.filter((v) => v.provider_id.toLowerCase() === target)
          : list;
        if (provider) VOICES_CACHE.set(provider, filtered);
        setVoices(filtered);
        // Drop a staged id that is not in the newly loaded engine scope;
        // otherwise the strip shows one voice while Select commits another.
        setStagedId((prev) =>
          prev != null && filtered.some((v) => v.id === prev) ? prev : null
        );
      })
      .catch((e) => {
        if (isMounted) setError(ipcErrorMessage(e));
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });
    return () => {
      isMounted = false;
    };
  }, [provider]);

  useEffect(() => {
    if (suspended) return;
    let alive = true;
    let teardown: (() => void) | undefined;
    const run = async () => {
      await Promise.resolve();
      if (!alive) return;
      teardown = reload();
    };
    void run();
    // Non-blocking background sync of remote voices runs once on mount when cloudSync is enabled
    if (cloudSync && !remoteSyncedRef.current) {
      remoteSyncedRef.current = true;
      syncRemoteVoices()
        .then(() => {
          listVoices(provider).then((list) => {
            if (alive) {
              const target = provider?.toLowerCase();
              const scoped = target
                ? list.filter((v) => v.provider_id.toLowerCase() === target)
                : list;
              if (provider) VOICES_CACHE.set(provider, scoped);
              setVoices(scoped);
              setStagedId((prev) =>
                prev != null && scoped.some((v) => v.id === prev) ? prev : null
              );
            }
          }).catch(() => {});
        })
        .catch(() => {});
    }
    return () => {
      alive = false;
      if (teardown) teardown();
    };
  }, [reload, provider, suspended, cloudSync]);

  useEffect(() => {
    return () => {
      const audio = audioRef.current;
      if (audio) {
        detachAudioHandlers(audio);
        audio.pause();
        audio.removeAttribute("src");
        audioRef.current = null;
      }
      if (currentBlobUrlRef.current) {
        URL.revokeObjectURL(currentBlobUrlRef.current);
        currentBlobUrlRef.current = null;
      }
    };
  }, []);

  const stopPreview = useCallback(() => {
    activeVoiceRef.current = null;
    const audio = audioRef.current;
    if (audio) {
      detachAudioHandlers(audio);
      audio.pause();
      audio.removeAttribute("src");
      audioRef.current = null;
    }
    if (currentBlobUrlRef.current) {
      URL.revokeObjectURL(currentBlobUrlRef.current);
      currentBlobUrlRef.current = null;
    }
    setPlayingId(null);
    setIsSynthesizing(false);
    setPreviewProgress(0);
    setPreviewCurrentTime(0);
    setPreviewDuration(0);
  }, []);

  const seekPreview = useCallback((fraction: number) => {
    if (
      audioRef.current &&
      Number.isFinite(audioRef.current.duration) &&
      audioRef.current.duration > 0
    ) {
      const clamped = Math.max(0, Math.min(1, fraction));
      audioRef.current.currentTime = clamped * audioRef.current.duration;
      setPreviewProgress(clamped);
      setPreviewCurrentTime(audioRef.current.currentTime);
    }
  }, []);

  const stageVoice = useCallback(
    (id: string) => {
      setStagedId(id);
      updateDraft("tts", "active_voice_id", id);
    },
    [updateDraft]
  );

  const commitVoice = useCallback(
    (id: string) => {
      stopPreview();
      stageVoice(id);
      recordVoiceUsed(id).catch(() => {});
    },
    [stageVoice, stopPreview]
  );

  const toggleFavorite = useCallback((id: string) => {
    const trimmedId = id.trim();
    console.info("[VoiceCatalog] toggleFavorite initiated for:", trimmedId);
    setVoices((prev) => {
      const target = prev.find((v) => v.id.toLowerCase() === trimmedId.toLowerCase());
      const nextFav = !target?.is_favorite;
      console.info("[VoiceCatalog] Optimistic toggle for", trimmedId, "->", nextFav);
      const next = prev.map((v) =>
        v.id.toLowerCase() === trimmedId.toLowerCase() ? { ...v, is_favorite: nextFav } : v
      );
      if (provider) VOICES_CACHE.set(provider, next);
      return next;
    });
    toggleVoiceFavorite(trimmedId)
      .then((nowFav) => {
        console.info("[VoiceCatalog] toggleVoiceFavorite IPC confirmed for", trimmedId, "->", nowFav);
        setVoices((prev) => {
          const next = prev.map((v) =>
            v.id.toLowerCase() === trimmedId.toLowerCase() ? { ...v, is_favorite: nowFav } : v
          );
          if (provider) VOICES_CACHE.set(provider, next);
          return next;
        });
      })
      .catch((err) => {
        console.error("[VoiceCatalog] toggleVoiceFavorite IPC failed for", trimmedId, err);
        setVoices((prev) => {
          const next = prev.map((v) =>
            v.id.toLowerCase() === trimmedId.toLowerCase() ? { ...v, is_favorite: !v.is_favorite } : v
          );
          if (provider) VOICES_CACHE.set(provider, next);
          return next;
        });
      });
  }, [provider]);

  const togglePreview = useCallback(
    (voice: UnifiedVoiceDto) => {
      // Read the ref, not `playingId`: state updates are async, so two fast
      // clicks on the same voice would otherwise both (re)start playback
      // instead of toggling stop.
      if (activeVoiceRef.current === voice.id) {
        stopPreview();
        return;
      }
      stopPreview();
      activeVoiceRef.current = voice.id;
      setPlayingId(voice.id);
      setIsSynthesizing(true);
      setStagedId(voice.id);

      const promptToUse =
        previewPrompt.trim().length > 0
          ? previewPrompt
          : voice.sample_text || DEFAULT_PREVIEW_PROMPT;

      console.info("[VoiceCatalog] Requesting preview for voice:", voice.id, "engine:", engine);

      synthesizeVoicePreview(voice.id, promptToUse, engine)
        .then(async (result) => {
          if (activeVoiceRef.current !== voice.id) {
            console.info("[VoiceCatalog] Preview cancelled or voice switched for:", voice.id);
            return;
          }
          setIsSynthesizing(false);
          console.info("[VoiceCatalog] synthesizeVoicePreview result:", result);
          if (!result.path) {
            console.warn("[VoiceCatalog] Preview returned empty path for voice:", voice.id);
            setPlayingId(null);
            return;
          }

          if (result.duration_ms > 0) {
            setPreviewDuration(result.duration_ms / 1000);
          }

          let audioSrc = voicePreviewUrl(result.path);
          if (result.audio_base64) {
            audioSrc = `data:audio/wav;base64,${result.audio_base64}`;
          } else {
            try {
              const { readFile } = await import("@tauri-apps/plugin-fs");
              const fileBytes = await readFile(result.path);
              const blob = new Blob([fileBytes], { type: "audio/wav" });
              const blobUrl = URL.createObjectURL(blob);
              currentBlobUrlRef.current = blobUrl;
              audioSrc = blobUrl;
            } catch (fsErr) {
              console.warn("[VoiceCatalog] readFile failed, using asset convertFileSrc fallback:", fsErr);
            }
          }

          if (activeVoiceRef.current !== voice.id) {
            return;
          }

          const audio = new Audio(audioSrc);
          audio.loop = false;
          audioRef.current = audio;

          audio.onloadeddata = () => {
            if (audioRef.current !== audio) return;
            console.info("[VoiceCatalog] Audio loaded data for", voice.id);
          };
          audio.onloadedmetadata = () => {
            if (audioRef.current !== audio) return;
            console.info("[VoiceCatalog] Audio metadata loaded. Duration:", audio.duration);
            if (Number.isFinite(audio.duration) && audio.duration > 0) {
              setPreviewDuration(audio.duration);
            }
          };
          audio.onplay = () => {
            if (audioRef.current !== audio) return;
            console.info("[VoiceCatalog] Audio playback started for", voice.id);
          };
          audio.onended = () => {
            if (audioRef.current !== audio) return;
            console.info("[VoiceCatalog] Audio playback ended for", voice.id);
            stopPreview();
          };
          audio.onerror = (e) => {
            // Ignore errors from a superseded element: clearing `src` on teardown
            // fires a synthetic error that must not tear down the current preview.
            if (audioRef.current !== audio) return;
            const mediaErr = audio.error;
            console.error("[VoiceCatalog] Audio playback error for", audioSrc.slice(0, 50), {
              code: mediaErr?.code,
              message: mediaErr?.message,
              event: e,
            });
            stopPreview();
          };
          audio.ontimeupdate = () => {
            if (audioRef.current !== audio) return;
            if (Number.isFinite(audio.duration) && audio.duration > 0) {
              setPreviewProgress(audio.currentTime / audio.duration);
              setPreviewCurrentTime(audio.currentTime);
            }
          };

          audio.play().then(() => {
            if (audioRef.current !== audio) return;
            console.info("[VoiceCatalog] audio.play() succeeded for", voice.id);
          }).catch((err) => {
            if (audioRef.current !== audio) return;
            console.error("[VoiceCatalog] Audio play() promise rejected:", err);
            stopPreview();
          });
        })
        .catch((err) => {
          console.error("[VoiceCatalog] synthesizeVoicePreview failed:", err);
          if (activeVoiceRef.current === voice.id) {
            setIsSynthesizing(false);
            setPlayingId(null);
          }
        });
    },
    [previewPrompt, engine, stopPreview]
  );

  const effectiveVoices = suspended ? [] : voices;
  const effectiveLoading = suspended ? true : loading;
  const effectiveStagedId =
    stagedId ?? (draftVoiceId && voices.some((v) => v.id === draftVoiceId) ? draftVoiceId : null);

  return {
    voices: effectiveVoices,
    loading: effectiveLoading,
    error,
    reload,
    stagedId: effectiveStagedId,
    stageVoice,
    commitVoice,
    toggleFavorite,
    playingId,
    isSynthesizing,
    previewProgress,
    previewCurrentTime,
    previewDuration,
    previewPrompt,
    setPreviewPrompt,
    togglePreview,
    seekPreview,
    stopPreview,
  };
}

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

const SCOPE_CACHE = new Map<string, VoiceScopeDto>();

export function useVoiceScope(engine: string | undefined | null): VoiceScopeState {
  const resolvedEngine = engine ?? "";
  const cached = resolvedEngine ? SCOPE_CACHE.get(resolvedEngine) : undefined;
  const [scope, setScope] = useState<VoiceScopeDto>(cached ?? FALLBACK_SCOPE);
  const [ready, setReady] = useState(Boolean(cached));
  const [prevEngine, setPrevEngine] = useState(resolvedEngine);

  if (prevEngine !== resolvedEngine) {
    setPrevEngine(resolvedEngine);
    const nowCached = resolvedEngine ? SCOPE_CACHE.get(resolvedEngine) : undefined;
    setScope(nowCached ?? FALLBACK_SCOPE);
    setReady(Boolean(nowCached));
  }

  useEffect(() => {
    const key = resolvedEngine;
    if (SCOPE_CACHE.has(key)) {
      return;
    }
    let isMounted = true;
    getVoiceScope(resolvedEngine || undefined)
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
  }, [resolvedEngine, cached]);

  return { scope, ready };
}


