import type {
  VoxSettings,
  ActiveSelection,
  LlmCustomServer,
  TtsCustomServer,
} from "@/types/settings";
/**
 * Fully-typed VoxSettings wire fixtures shared by settings-facing tests.
 *
 * Valid helper per /create-test Phase 1: every literal is the exact IPC wire
 * shape the backend emits (types/settings.ts mirrors config/types.rs), and the
 * fixture is complete against VoxSettings — no `as unknown as` escape, so a
 * production field rename is a compile error here, not silent drift. Deep
 * clone on every call prevents cross-test mutation of module constants.
 */

export const EMBEDDED_ACTIVE: ActiveSelection = {
  category: "embedded",
  provider: null,
  model: "qwen",
};

export const SERVER_ACTIVE: ActiveSelection = {
  category: "server",
  provider: "srv-1",
  model: "",
};

export const SERVER_ONE: LlmCustomServer = {
  id: "srv-1",
  name: "One",
  url: "http://10.0.0.1:11434",
  model: "",
  protocol: "openai",
  api_key: null,
};

export function minimalSettings(): VoxSettings {
  return {
    vad: {
      threshold: 0.5,
      ptt_noise_gate: 0.3,
      vad_backend: "silero",
      silence_duration_ms: 800,
      speech_onset_ms: 90,
      max_speech_duration_s: 30,
    },
    stt: {
      active: EMBEDDED_ACTIVE,
      last_selected: { embedded: "whisper-small" },
      transliterate_enabled: false,
    },
    llm: {
      active: EMBEDDED_ACTIVE,
      last_selected: { embedded: "qwen" },
      cloud_keys: {},
    },
    tts: {
      active: EMBEDDED_ACTIVE,
      last_selected: { embedded: "kokoro-de" },
      active_voice_id: "",
    },
    realtime: {
      active: EMBEDDED_ACTIVE,
      active_voice_id: "",
    },
    interaction: { mode: "passive", pipeline_mode: "modular" },
    dictation: {
      enabled: false,
      interaction_mode: "passive",
      hotkey: "Alt+Space",
      output_mode: "paste",
    },
    working_memory: {
      private_mode: false,
      auto_compaction: true,
      max_context_share: 0.5,
      web_search_enabled: false,
    },
    appearance: { theme: "dark", accent_seed: "#0f766e" },
    personal_memory: {
      context_retrieval_enabled: true,
      pipeline_processing_enabled: true,
      top_k_facts: 5,
      semantic_similarity_cutoff: 0.35,
      consolidation_cadence: "daily",
      consolidation_time: "03:00",
    },
    persona: { modular_prompt: "a", realtime_prompt: "b" },
    system: { setup_completed: true },
  };
}

/** A draft variant factory that deep-clones from the committed fixture. */
export function settingsFrom(settings: VoxSettings): VoxSettings {
  return structuredClone(settings);
}

export const TTS_SERVER_ONE: TtsCustomServer = {
  id: "tts-srv-1",
  name: "Vox Server",
  url: "http://10.0.0.1:8080",
  remote_path: "v1/audio/speech",
};