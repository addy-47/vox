import { useCallback, useEffect, useRef, useState } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  listVoices,
  recordVoiceUsed,
  syncRemoteVoices,
  synthesizeVoicePreview,
  toggleVoiceFavorite,
  voicePreviewUrl,
  type UnifiedVoiceDto,
} from "@/services/voiceService";

export interface UseVoiceCatalogOptions {
  provider?: string;
  /**
   * When true, no fetch runs and the list stays empty. Used to hold the
   * catalog until backend scope resolution lands, so another engine's (or
   * all providers') voices are never rendered in between.
   */
  suspended?: boolean;
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

/**
 * Unified voice catalog state: Turso-backed list, draft staging, favorites,
 * on-demand preview synthesis, and interactive playback scrub bar.
 */
export function useVoiceCatalog(
  options: UseVoiceCatalogOptions = {}
): UseVoiceCatalogResult {
  const { provider, suspended = false } = options;
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const draftVoiceId = useSettingsStore((s) => s.draftSettings?.tts?.voice_id);

  const [voices, setVoices] = useState<UnifiedVoiceDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stagedId, setStagedId] = useState<string | null>(null);

  const [playingId, setPlayingId] = useState<string | null>(null);
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const [previewProgress, setPreviewProgress] = useState(0);
  const [previewCurrentTime, setPreviewCurrentTime] = useState(0);
  const [previewDuration, setPreviewDuration] = useState(0);
  const [previewPrompt, setPreviewPrompt] = useState(DEFAULT_PREVIEW_PROMPT);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const remoteSyncedRef = useRef(false);

  const reload = useCallback(() => {
    let isMounted = true;
    setLoading(true);
    setError(null);
    // Never render a stale engine's voices: clear the list up front so an
    // engine switch shows loading, not the previous engine's rows.
    setVoices([]);
    listVoices(provider)
      .then((list) => {
        if (!isMounted) return;
        setVoices(list);
        // Drop a staged id that is not in the newly loaded engine scope;
        // otherwise the strip shows one voice while Select commits another.
        setStagedId((prev) =>
          prev != null && list.some((v) => v.id === prev) ? prev : null
        );
      })
      .catch((e) => {
        if (isMounted) setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });
    return () => {
      isMounted = false;
    };
  }, [provider]);

  useEffect(() => {
    let alive = true;
    if (suspended) {
      setVoices([]);
      setLoading(true);
      return () => {
        alive = false;
      };
    }
    const teardown = reload();
    // Non-blocking background sync of remote voices (Edge TTS) runs once on mount
    if (!remoteSyncedRef.current) {
      remoteSyncedRef.current = true;
      syncRemoteVoices()
        .then(() => {
          listVoices(provider).then((list) => {
            if (alive) {
              setVoices(list);
              setStagedId((prev) =>
                prev != null && list.some((v) => v.id === prev) ? prev : null
              );
            }
          }).catch(() => {});
        })
        .catch(() => {});
    }
    return () => {
      alive = false;
      teardown();
    };
  }, [reload, provider, suspended]);

  useEffect(() => {
    return () => {
      if (audioRef.current) {
        audioRef.current.pause();
        audioRef.current.src = "";
        audioRef.current = null;
      }
    };
  }, []);

  const stopPreview = useCallback(() => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current.src = "";
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
      updateDraft("tts", "voice_id", id);
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
    setVoices((prev) =>
      prev.map((v) => (v.id === id ? { ...v, is_favorite: !v.is_favorite } : v))
    );
    toggleVoiceFavorite(id)
      .then((nowFav) => {
        setVoices((prev) =>
          prev.map((v) => (v.id === id ? { ...v, is_favorite: nowFav } : v))
        );
      })
      .catch(() => {
        setVoices((prev) =>
          prev.map((v) =>
            v.id === id ? { ...v, is_favorite: !v.is_favorite } : v
          )
        );
      });
  }, []);

  const togglePreview = useCallback(
    (voice: UnifiedVoiceDto) => {
      if (playingId === voice.id) {
        stopPreview();
        return;
      }
      stopPreview();
      setPlayingId(voice.id);
      setIsSynthesizing(true);
      setStagedId(voice.id);

      const promptToUse =
        previewPrompt.trim().length > 0
          ? previewPrompt
          : voice.sample_text || DEFAULT_PREVIEW_PROMPT;

      synthesizeVoicePreview(voice.id, promptToUse)
        .then((result) => {
          setIsSynthesizing(false);
          if (!result.path) {
            setPlayingId(null);
            return;
          }
          const audioSrc = voicePreviewUrl(result.path);
          const audio = new Audio(audioSrc);
          audioRef.current = audio;
          audio.onloadedmetadata = () => {
            setPreviewDuration(audio.duration || result.duration_ms / 1000);
          };
          audio.onended = () => {
            setPlayingId(null);
            setPreviewProgress(0);
            setPreviewCurrentTime(0);
          };
          audio.onerror = (e) => {
            console.error("[VoiceCatalog] Audio playback error for", audioSrc, e);
            setPlayingId(null);
            setPreviewProgress(0);
            setIsSynthesizing(false);
          };
          audio.ontimeupdate = () => {
            if (Number.isFinite(audio.duration) && audio.duration > 0) {
              setPreviewProgress(audio.currentTime / audio.duration);
              setPreviewCurrentTime(audio.currentTime);
            }
          };
          audio.play().catch((err) => {
            console.error("[VoiceCatalog] Audio play() promise rejected:", err);
            setPlayingId(null);
            setIsSynthesizing(false);
          });
        })
        .catch((err) => {
          console.error("[VoiceCatalog] synthesizeVoicePreview failed:", err);
          setIsSynthesizing(false);
          setPlayingId(null);
        });
    },
    [playingId, previewPrompt, stopPreview]
  );

  useEffect(() => {
    // Adopt the persisted draft voice only when it belongs to the currently
    // loaded engine scope — never stage another engine's voice id.
    if (!stagedId && draftVoiceId && voices.some((v) => v.id === draftVoiceId)) {
      setStagedId(draftVoiceId);
    }
  }, [stagedId, draftVoiceId, voices]);

  return {
    voices,
    loading,
    error,
    reload,
    stagedId,
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

