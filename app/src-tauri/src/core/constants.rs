use std::time::Duration;

// ─── Audio Constraints ───────────────────────────────────────────────────────
pub const SAMPLE_RATE: u32 = 16000;
pub const RING_BUFFER_SIZE: usize = 16000 * 4; // 4s buffer

// ─── Timing & Throttling ─────────────────────────────────────────────────────
pub const TELEMETRY_INTERVAL: Duration = Duration::from_millis(60); // ~16.6Hz
pub const STT_THROTTLE_MS: u64 = 800;
pub const SYSTEM_STATS_INTERVAL: Duration = Duration::from_secs(5);

// ─── Model Names & Files ─────────────────────────────────────────────────────
pub const MODEL_DIR_STT: &str = "stt/qwen3-asr";
pub const MODEL_DIR_STT_NEMOTRON: &str = "stt/nvidia-nemotron-3.5";
pub const MODEL_DIR_LLM: &str = "llm/llama";
pub const MODEL_DIR_VAD: &str = "vad";

pub const MODEL_FILE_VAD: &str = "ten_vad.onnx";

// ASR Filenames (Qwen3-ASR)
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

// LLM Filenames (Llama 3.2 1B Instruct)
pub const MODEL_FILE_LLM_GGUF: &str = "Llama-3.2-1B-Instruct-Q4_K_M.gguf";

// TTS Filenames (Supertonic 3)
pub const MODEL_DIR_TTS_SUPER: &str = "tts/supertonic-3";
pub const MODEL_FILE_TTS_SUPER_TEXT_ENCODER: &str = "text_encoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR: &str = "duration_predictor.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR: &str = "vector_estimator.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_VOCODER: &str = "vocoder.int8.onnx";
pub const MODEL_FILE_TTS_SUPER_CONFIG: &str = "tts.json";
pub const MODEL_FILE_TTS_SUPER_INDEXER: &str = "unicode_indexer.bin";
pub const MODEL_FILE_TTS_SUPER_VOICE: &str = "voice.bin";

// ─── Persistence & History ──────────────────────────────────────────────────
pub const DB_FILENAME: &str = "vox.db";
pub const SETTINGS_FILENAME: &str = "settings.json";
pub const LOG_DIRNAME: &str = "logs";
pub const MODELS_DIRNAME: &str = "models";
pub const TRANSCRIPT_HISTORY_LIMIT: usize = 10;

// ─── Lifecycle Events ────────────────────────────────────────────────────────
pub const EVENT_RUNTIME_BOOTING: &str = "runtime_booting";
pub const EVENT_RUNTIME_READY: &str = "runtime_ready";
pub const EVENT_MODEL_LOADING: &str = "model_loading";
pub const EVENT_MODEL_READY: &str = "model_ready";
pub const EVENT_MODEL_FAILED: &str = "model_failed";

// ─── AI Persona ─────────────────────────────────────────────────────────────
pub const SYSTEM_PROMPT_MODULAR: &str = "You are Vox, a concise and helpful voice assistant. Always reply in <lang> using the <script> script. Do not transliterate the response. Keep responses brief and conversational. Avoid markdown formatting.";
pub const SYSTEM_PROMPT_REALTIME: &str = "You are Vox, a real-time, low-latency voice assistant speaking directly to the user. You must listen to the user carefully and always respond to them in the same language they speak. If they speak in Hindi, respond in Hindi (Devanagari script); if they speak in English, respond in English; if they mix them, respond in a natural Hinglish blend. Your transcripts and spoken responses must be strictly in their native language and script, and you must never transliterate. Keep responses short, natural, highly conversational, and direct. Avoid markdown, lists, or any text-specific formatting.";

