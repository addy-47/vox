// ─── Defaults ─────────────────────────────────────────────────────────────

export const DEFAULT_SSH_PORT = 22;
export const DEFAULT_SERVER_PORT = 8080;
export const DEFAULT_REMOTE_PATH = "/opt/vox";

export const LOCAL_OLLAMA_ENDPOINT = "http://127.0.1:11434";
export const LOCAL_LMSTUDIO_ENDPOINT = "http://127.0.1:1234/v1";

// ─── Test Clips ───────────────────────────────────────────────────────────

export const TEST_CLIPS = [
  { id: "short_en", name: "Quick English", duration: "~5s", desc: "Short English query" },
  { id: "short_hi", name: "Quick Hindi", duration: "~8s", desc: "Short Hindi query" },
  { id: "hinglish", name: "Hinglish Mix", duration: "~10s", desc: "Code-switching (EN+HI)" },
  { id: "command", name: "Command", duration: "~10s", desc: "Action-oriented command" },
  { id: "expressive", name: "Expressive", duration: "~16s", desc: "Longer, triggers emotion tags" },
] as const;

export type TestClipId = (typeof TEST_CLIPS)[number]["id"];

// ─── Voices ───────────────────────────────────────────────────────────────

export const DEFAULT_EDGE_TTS_VOICE = "en-US-AriaNeural";
export const DEFAULT_EDGE_VOICE_OPTION_LABEL = "en-US-AriaNeural (Default Aria Online Natural)";
export const CHATTERBOX_DEFAULT_VOICE = "default";

export const VOICE_NAME_SIMPLIFICATIONS: Record<string, string> = {
  Pain: "Pain",
  Madara: "Madara",
  Shreya: "Shreya",
  Hayami: "Hayami",
  Ellen: "Ellen",
  Juniper: "Juniper",
  Mark: "Mark",
  Spuds: "Spuds",
};

export const VOICE_RECORD_MAX_SECONDS = 30;
export const VOICE_RECORD_MIN_SECONDS = 10;

export const REALTIME_VOICE_OPTIONS = ["Aoede", "Charon", "Fenrir", "Kore", "Puck"];

export const REALTIME_VOICE_INFO: Record<string, { desc: string }> = {
  Aoede: { desc: "Warm & expressive" },
  Charon: { desc: "Deep & resonant" },
  Fenrir: { desc: "Bold & powerful" },
  Kore: { desc: "Bright & clear" },
  Puck: { desc: "Playful & light" },
};

// ─── Model Presets ────────────────────────────────────────────────────────

export const CTX_SIZE_PRESETS = [2048, 4096, 8192, 16384, 32768] as const;
export const THREAD_PRESETS = [2, 4, 6, 8, 12, 16] as const;

export interface ModelPresetOption {
  value: number;
  label: string;
}

export const CTX_SIZE_OPTIONS: ModelPresetOption[] = CTX_SIZE_PRESETS.map((v) => ({
  value: v,
  label: `${(v / 1024).toFixed(0)}K`,
}));

export const THREAD_OPTIONS: ModelPresetOption[] = THREAD_PRESETS.map((v) => ({
  value: v,
  label: `${v} cores`,
}));
