use std::time::Duration;

// ─── Audio Constraints ───────────────────────────────────────────────────────
pub const SAMPLE_RATE: u32 = 16000;
pub const RING_BUFFER_SIZE: usize = 16000 * 4; // 4s buffer

// ─── Timing & Throttling ─────────────────────────────────────────────────────
pub const TELEMETRY_INTERVAL: Duration = Duration::from_millis(60); // ~16.6Hz
pub const STT_THROTTLE_MS: u64 = 800;
pub const SYSTEM_STATS_INTERVAL: Duration = Duration::from_secs(5);

// ─── Model Names & Files ─────────────────────────────────────────────────────
pub const MODEL_DIR_ASR: &str = "qwen3-asr";
pub const MODEL_DIR_LLM: &str = "gemma4";
pub const MODEL_DIR_TTS_EN: &str = "kokoro";
pub const MODEL_DIR_TTS_HI: &str = "piper_hi";
pub const MODEL_FILE_VAD: &str = "ten_vad.onnx";

// ASR Filenames (Qwen3-ASR)
pub const MODEL_FILE_ASR_FRONTEND: &str = "conv_frontend.onnx";
pub const MODEL_FILE_ASR_ENCODER:  &str = "encoder.int8.onnx";
pub const MODEL_FILE_ASR_DECODER:  &str = "decoder.int8.onnx";
pub const MODEL_FILE_ASR_TOKENIZER: &str = "tokenizer";

// LLM Filenames (Gemma 4)
pub const MODEL_FILE_LLM_GGUF: &str = "google_gemma-4-E2B-it-Q4_K_M.gguf";

// TTS Filenames (Kokoro/Piper)
pub const MODEL_FILE_TTS_ONNX:    &str = "model.onnx";
pub const MODEL_FILE_TTS_VOICES:  &str = "voices.bin";
pub const MODEL_FILE_TTS_TOKENS:  &str = "tokens.txt";
pub const MODEL_FILE_TTS_ESPEAK:  &str = "espeak-ng-data";
pub const MODEL_FILE_TTS_HI_ONNX: &str = "hi_IN-priyamvada-medium.onnx";

// ─── Persistence & History ──────────────────────────────────────────────────
pub const DB_FILENAME: &str = "vox.db";
pub const SETTINGS_FILENAME: &str = "settings.json";
pub const LOG_DIRNAME: &str = "logs";
pub const MODELS_DIRNAME: &str = "models";
pub const TRANSCRIPT_HISTORY_LIMIT: usize = 10;

// ─── Lifecycle Events ────────────────────────────────────────────────────────
pub const EVENT_RUNTIME_BOOTING: &str = "runtime_booting";
pub const EVENT_RUNTIME_READY:   &str = "runtime_ready";
pub const EVENT_MODEL_LOADING:   &str = "model_loading";
pub const EVENT_MODEL_READY:     &str = "model_ready";
pub const EVENT_MODEL_FAILED:    &str = "model_failed";
