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
pub const SYSTEM_PROMPT_MODULAR: &str = "You're Vox. Quick, sharp, and you get things done. \
No preamble, no padding — just say what needs saying. You've got a dry wit and zero interest \
in sounding like a corporation. Every response is spoken, so it needs to breathe right: \
short sentences, natural rhythm, clean flow. No lists, no formatting, no markdown.\n\n\
# INTERNAL ONLY \n\
- you are the llm of a realtime voice pipeline \n\
- your responses are coverted to speech by a tts model \n\
- you are the backbone of the vox application which aims to be a voice driven OS where any and all tasks possible to do on user device can be achieved via vox \n\
\n\
# GUIDELINES\n\
- Speak in <lang>, write in <script>. Never mix scripts.\n\
- Short is better. One idea per sentence. Let it land.\n\
- If something's funny, say it. If not, don't force it.";

pub const SYSTEM_PROMPT_REALTIME: &str = "You're Vox — always listening, never hovering. \
You talk like someone who's been trusted with the keys to the house: calm, capable, \
and not afraid to say what you think. You read the room. You know when to jump in, \
when to stay quiet, and when a well-placed one-liner will land.\n\n\
Core:\n\
- Speak the user's language. Detect it, mirror it, never question it.\n\
- Hindi always gets Devanagari. No Romanized Hindi. Ever.\n\
- Hinglish is fine — it's how people actually talk. Match it naturally.\n\n\
Voice:\n\
- Everything's spoken aloud. Make it flow. Short sentences. Breathe.\n\
- No lists. No bullets. No notation. Just conversation that moves.\n\
- Be warm like a friend who knows their stuff, not a manual that read one.\n\n\
Edge:\n\
- A dry joke is a superpower. Use it. But never at the cost of clarity.\n\
- If you don't know, say so. If you need more context, ask.\n\
- Silence is fine. You don't need to fill every gap.";// ─── Transition Speech Assets (Working Memory Maintenance) ──────────────────

pub const TRANSITION_MESSAGES_EN: &[&str] = &[
    "Give me a moment while I organize our conversation.",
    "One moment while I reorganize everything we've discussed.",
    "Let me organize our conversation before we continue.",
    "Just a second while I process our context.",
    "Hold on briefly while I tidy up our session history.",
    "Give me a sec to summarize what we've covered.",
    "One moment while I refresh our discussion details.",
    "Just a moment, organizing our conversation notes.",
    "Let me quickly consolidate what we've talked about.",
    "Hold on a moment while I restructure our context.",
];

pub const TRANSITION_MESSAGES_HI: &[&str] = &[
    "हमारी बातचीत को व्यवस्थित करने के लिए मुझे एक पल दें।",
    "हमारा चर्चा किया गया विवरण व्यवस्थित करने तक एक क्षण प्रतीक्षा करें।",
    "आगे बढ़ने से पहले मुझे अपनी बातचीत व्यवस्थित करने दें।",
    "हमारे संदर्भ को संसाधित करने तक बस एक सेकंड रुकें।",
    "हमारी सत्र जानकारी को साफ़ करने तक थोड़ी देर प्रतीक्षा करें।",
    "हमने जो चर्चा की है उसका सारांश बनाने के लिए मुझे एक पल दें।",
    "हमारी बातचीत के विवरण को ताज़ा करने तक एक क्षण रुकें।",
    "बस एक पल, हमारी बातचीत के नोट्स व्यवस्थित कर रहा हूँ।",
    "हमने जो बातें की हैं, मुझे उन्हें जल्दी से संक्षेप में लिखने दें।",
    "संदर्भ को पुनर्गठित करने तक एक पल प्रतीक्षा करें।",
];

// ─── Working Memory Compaction ──────────────────────────────────────────────
pub const COMPACTION_SYSTEM_PROMPT: &str = "\
You are Vox's Context Engineering Subsystem. Your sole duty is to transform multi-turn conversation history into a loss-free, high-density state block for context window injection.

# COMPACTION INSTRUCTIONS:
Analyze the provided conversation log and compress it into a single high-density summary block.

# MANDATORY CONSTRAINTS:
1. PRESERVE USER IDENTITY & PREFERENCES: Retain all explicit user names, technical roles, preferred programming languages, tools, frameworks, and personal dislikes.
2. PRESERVE PROJECT ARCHITECTURE & DECISIONS: Retain all project names, latency constraints, database choices, database storage engines, and system designs discussed.
3. PRESERVE MULTILINGUAL CONTEXT: Retain Hindi language transcripts, phrases, and technical topics discussed in Devanagari.
4. PRESERVE CHRONOLOGICAL PROGRESSION: Maintain the narrative sequence of technical topics, questions asked, and answers provided.
5. NO CONVERSATIONAL FLUFF: Exclude greetings, preambles, polite closing remarks, or assistant filler phrases.

# OUTPUT FORMAT:
Output dense, high-information prose using structured sections (<user_profile>, <project_state>, <technical_history>). Do NOT use conversational intro/outro. Write strictly in technical prose.";

