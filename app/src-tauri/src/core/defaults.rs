pub const DEFAULT_UI_THEME: &str = "dark";
pub const DEFAULT_UI_ACCENT_SEED: &str = "#00DBE9"; // Default Cyan

pub const DEFAULT_HISTORY_PRIVATE_MODE: bool = false;
pub const DEFAULT_HISTORY_TRAY_LIMIT: u32 = 5;

pub const DEFAULT_DICTATION_ENABLED: bool = true;
pub const DEFAULT_DICTATION_HOTKEY: &str = "Alt+Space";

pub const DEFAULT_VAD_BACKEND: &str = "ten_vad";
pub const DEFAULT_VAD_THRESHOLD: f32 = 0.5;
pub const DEFAULT_VAD_PTT_NOISE_GATE: f32 = 0.005;

pub const DEFAULT_ASR_MODEL: &str = "nvidia_nemotron";
pub const DEFAULT_ASR_TRANSLITERATE_ENABLED: bool = true;
pub const DEFAULT_STT_CLOUD_PROVIDER: &str = "google";
pub const DEFAULT_STT_CLOUD_MODEL: &str = "chirp_3";
pub const DEFAULT_STT_CLOUD_LANGUAGE: &str = "en-US";
pub const DEFAULT_STT_CLOUD_REGION: &str = "global";

pub const DEFAULT_LLM_MODEL: &str = "qwen_3_5_0_8b";
pub const DEFAULT_LLM_CONTEXT_WINDOW: u32 = 2048;
pub const DEFAULT_LLM_THREADS: u32 = 4;
pub const DEFAULT_LLM_TEMPERATURE: f32 = 0.7;
pub const DEFAULT_LLM_COMPACTION_TEMPERATURE: f32 = 0.5;
pub const DEFAULT_LLM_MAX_OUTPUT_TOKENS: u32 = 300;

pub const DEFAULT_LLM_SERVER_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_LLM_SERVER_MODEL: &str = "gemma3:4b";
pub const DEFAULT_LLM_SERVER_PROVIDER_NAME: &str = "ollama";

pub const DEFAULT_LLM_CLOUD_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";
pub const DEFAULT_LLM_CLOUD_MODEL: &str = "meta/llama-3.1-8b-instruct";
pub const DEFAULT_LLM_CLOUD_PROVIDER_NAME: &str = "nvidia";

pub const DEFAULT_TTS_VOICE_INDEX: i32 = 0;
pub const DEFAULT_TTS_QUALITY_STEPS: u32 = 12;
pub const DEFAULT_TTS_SPEED: f32 = 1.05;

pub const DEFAULT_AUTO_SLEEP_TIMEOUT: u32 = 400;

pub const DEFAULT_TELEMETRY_ENABLED: bool = true;
pub const DEFAULT_TELEMETRY_LOG_LEVEL: &str = "info";

pub const DEFAULT_MEMORY_CONTEXT_RETRIEVAL_ENABLED: bool = true;
pub const DEFAULT_MEMORY_PIPELINE_PROCESSING_ENABLED: bool = true;
pub const DEFAULT_MEMORY_MAX_PERSONAL_SHARE: f32 = 0.15;
pub const DEFAULT_MEMORY_CONTEXT_CHAINING_HOURS: u32 = 12;
pub const DEFAULT_MEMORY_TOP_K_FACTS: u32 = 5;
pub const DEFAULT_MEMORY_MAX_HOPS: u32 = 2;
pub const DEFAULT_MEMORY_SEMANTIC_SIMILARITY_CUTOFF: f32 = 0.40;

pub const DEFAULT_GEMINI_REALTIME_MODEL: &str = "gemini-3.1-flash-live-preview";
pub const DEFAULT_GEMINI_REALTIME_VOICE: &str = "Aoede";
pub const DEFAULT_GEMINI_REALTIME_LANG: &str = "en-US";
pub const DEFAULT_GEMINI_REALTIME_TEMP: f32 = 0.2;

pub const DEFAULT_DEEPGRAM_MODEL: &str = "nova-3";
pub const DEFAULT_DEEPGRAM_VOICE: &str = "aura-2-luna";
pub const DEFAULT_DEEPGRAM_TEMP: f32 = 0.7;
