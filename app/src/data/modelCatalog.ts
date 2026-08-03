export const REQUIRED_MODEL_GROUP_IDS = ["ten_vad", "vox_translit_rnn", "qwen3_asr", "nvidia_nemotron"] as const;

export const FALLBACK_MODEL_GROUP_IDS = [
  "ten_vad",
  "vox_translit_rnn",
  "qwen3_asr",
  "nvidia_nemotron",
  "gemma_4_reasoning",
  "llama_3_2_reasoning",
  "gemma_4_uncensored",
  "supertonic_tts",
  "chatterbox_tts",
] as const;

export const FILE_ID_PREFIX_TO_MODEL_GROUP = [
  ["vad", "ten_vad"],
  ["translit", "vox_translit_rnn"],
  ["stt_nemotron", "nvidia_nemotron"],
  ["stt_", "qwen3_asr"],
  ["tts_supertonic", "supertonic_tts"],
  ["tts_chatterbox", "chatterbox_tts"],
] as const;

export const LOCAL_TTS_MODEL_IDS = ["edge_tts", "supertonic_tts", "chatterbox_tts"] as const;

export const AUXILIARY_MODEL_CATEGORIES = ["classifier", "embedding", "nli", "translit"] as const;

export const AUXILIARY_CATEGORY_DESCRIPTIONS: Record<string, string> = {
  classifier: "Intent router classifying user queries into Generic or Semantic memory paths.",
  embedding: "Dense vector encoder for personal memory retrieval & semantic search.",
  nli: "Intra-collection contradiction detector ensuring memory consistency.",
  translit: "Converts Devanagari (Hindi) script to natural Hinglish phonetic spelling.",
};

export interface ModelProviderFilter {
  match: string;
  include: string[];
  exclude: string[];
}

export const PROVIDER_MODEL_FILTERS: ModelProviderFilter[] = [
  { match: "openai", include: ["gpt"], exclude: ["instruct", "embedding", "audio"] },
  { match: "gemini", include: ["gemini"], exclude: ["embedding"] },
  { match: "google", include: ["gemini"], exclude: ["embedding"] },
  { match: "anthropic", include: ["claude"], exclude: [] },
  { match: "nvidia", include: [], exclude: ["embedding", "rerank", "clip", "guard"] },
  { match: "groq", include: ["llama", "mixtral", "gemma"], exclude: ["whisper"] },
];

export const MODEL_SORT_EXPERIMENTAL_TOKENS = ["exp", "preview"] as const;

export const FILE_SIZE_UNITS = ["B", "KB", "MB", "GB"] as const;

export interface StaticModelCard {
  id: string;
  name: string;
  description: string;
  parameters: string;
  ramUsage?: string;
  required: boolean;
}

export const VAD_MODEL_CARDS: StaticModelCard[] = [
  {
    id: "earshot",
    name: "Earshot (Built-in)",
    description: "Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load.",
    parameters: "Built-in",
    ramUsage: "0 MB",
    required: true,
  },
  {
    id: "ten_vad",
    name: "TenVAD Engine",
    description: "ONNX-based voice detector. Requires downloading auxiliary neural files.",
    parameters: "ONNX",
    ramUsage: "~2 MB",
    required: false,
  },
];
