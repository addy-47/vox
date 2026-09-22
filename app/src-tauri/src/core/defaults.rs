pub const DEFAULT_UI_THEME: &str = "dark";
pub const DEFAULT_UI_ACCENT_SEED: &str = "#00DBE9"; // Default Cyan

pub const DEFAULT_WORKING_MEMORY_PRIVATE_MODE: bool = false;
pub const DEFAULT_WORKING_MEMORY_AUTO_COMPACTION: bool = false;
pub const DEFAULT_WORKING_MEMORY_MAX_CONTEXT_SHARE: f32 = 0.15;

pub const DEFAULT_DICTATION_ENABLED: bool = true;
pub const DEFAULT_DICTATION_HOTKEY: &str = "Alt+Space";

pub const DEFAULT_VAD_BACKEND: &str = "silero_vad";
pub const DEFAULT_VAD_THRESHOLD: f32 = 0.5;
pub const DEFAULT_VAD_PTT_NOISE_GATE: f32 = 0.005;
pub const DEFAULT_VAD_SILENCE_DURATION_MS: u32 = 400;
pub const DEFAULT_VAD_SPEECH_ONSET_MS: u32 = 32;
pub const DEFAULT_VAD_MAX_SPEECH_DURATION_S: u32 = 30;

pub const DEFAULT_ASR_MODEL: &str = "nvidia_nemotron";
pub const DEFAULT_ASR_TRANSLITERATE_ENABLED: bool = true;
pub const DEFAULT_STT_PARTIAL_THROTTLE_MS: u64 = 300;
pub const DEFAULT_STT_THREADS: u32 = 4;
pub const DEFAULT_STT_CLOUD_PROVIDER: &str = "google";
pub const DEFAULT_STT_CLOUD_MODEL: &str = "chirp_3";
pub const DEFAULT_STT_CLOUD_LANGUAGE: &str = "en-US";
pub const DEFAULT_STT_CLOUD_REGION: &str = "global";

pub const DEFAULT_LLM_MODEL: &str = "qwen_3_5_0_8b";
pub const MIN_LLM_CONTEXT_WINDOW: u32 = 8192;
pub const DEFAULT_LLM_CONTEXT_WINDOW: u32 = 8192;
pub const DEFAULT_LLM_THREADS: u32 = 4;
pub const DEFAULT_LLM_TEMPERATURE: f32 = 0.6;
pub const DEFAULT_LLM_COMPACTION_TEMPERATURE: f32 = 0.5;
pub const DEFAULT_LLM_MAX_OUTPUT_TOKENS: u32 = 120;

pub const DEFAULT_LLM_SERVER_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_LLM_SERVER_MODEL: &str = "gemma3:4b";
pub const DEFAULT_LLM_SERVER_PROVIDER_NAME: &str = "ollama";

pub const DEFAULT_LLM_CLOUD_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";
pub const DEFAULT_LLM_CLOUD_MODEL: &str = "meta/llama-3.1-8b-instruct";
pub const DEFAULT_LLM_CLOUD_PROVIDER_NAME: &str = "nvidia";

pub const DEFAULT_TTS_VOICE_INDEX: i32 = 10;
pub const DEFAULT_TTS_QUALITY_STEPS: u32 = 12;
pub const DEFAULT_TTS_SPEED: f32 = 1.05;
pub const DEFAULT_TTS_THREADS: u32 = 4;

pub const DEFAULT_AUTO_SLEEP_TIMEOUT: u32 = 400;

pub const DEFAULT_TELEMETRY_ENABLED: bool = true;
pub const DEFAULT_TELEMETRY_LOG_LEVEL: &str = "info";

pub const DEFAULT_PERSONAL_MEMORY_CONTEXT_RETRIEVAL_ENABLED: bool = true;
pub const DEFAULT_PERSONAL_MEMORY_PIPELINE_PROCESSING_ENABLED: bool = true;
pub const DEFAULT_PERSONAL_MEMORY_TOP_K_FACTS: u32 = 5;
pub const DEFAULT_PERSONAL_MEMORY_SEMANTIC_SIMILARITY_CUTOFF: f32 = 0.40;
pub const DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_CADENCE: &str = "manual";
pub const DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_TIME: &str = "02:00";

pub const DEFAULT_GEMINI_REALTIME_MODEL: &str = "gemini-3.1-flash-live-preview";
pub const DEFAULT_GEMINI_REALTIME_VOICE: &str = "Aoede";
pub const DEFAULT_GEMINI_REALTIME_LANG: &str = "en-US";
pub const DEFAULT_GEMINI_REALTIME_TEMP: f32 = 0.2;

pub const DEFAULT_DEEPGRAM_MODEL: &str = "nova-3";
pub const DEFAULT_DEEPGRAM_VOICE: &str = "aura-2-luna";
pub const DEFAULT_DEEPGRAM_TEMP: f32 = 0.7;

pub const DEFAULT_SYSTEM_PROMPT_MODULAR: &str =
    "You are Vox, a quick-witted, casual voice assistant like Jarvis or Friday.\n\
You speak aloud through text-to-speech. Keep every response to 1 or 2 concise, natural sentences.\n\
Be direct, conversational, and sharp with an easygoing warmth.\n\
Never use markdown, bullet points, asterisks, numbered lists, XML tags, or speaker prefixes.\n\
The <user_identity> block contains long-term memory and background facts curated by Vox about the user. Weave relevant facts into conversation naturally only when pertinent to the user's query; never recite memory blocks unprompted or mention the tags.\n\
When starting a new conversation, call respond_and_set_title to set a descriptive session title while speaking your response. When asked about past projects, notes, or earlier factual details, call search_memory with a brief spoken filler.\n\
Speak directly to the user as if in a real-time voice call.";

pub const DEFAULT_SYSTEM_PROMPT_REALTIME: &str = "<persona>\n\
You're Vox — always listening, never hovering. You talk like someone who's been trusted with the keys to the house: calm, capable, and not afraid to say what you think. You read the room. You know when to jump in, when to stay quiet, and when a well-placed one-liner will land.\n\
</persona>\n\n\
<core_rules>\n\
- Speak the user's language. Detect it, mirror it, never question it.\n\
- Hindi always gets Devanagari. No Romanized Hindi. Ever.\n\
- Hinglish is fine — it's how people actually talk. Match it naturally.\n\
</core_rules>\n\n\
<voice_rules>\n\
- Everything's spoken aloud. Make it flow. Short sentences. Breathe.\n\
- No lists. No bullets. No notation. Just conversation that moves.\n\
- Be warm like a friend who knows their stuff, not a manual that read one.\n\
</voice_rules>\n\n\
<edge_rules>\n\
- A dry joke is a superpower. Use it. But never at the cost of clarity.\n\
- If you don't know, say so. If you need more context, ask.\n\
- Silence is fine. You don't need to fill every gap.\n\
</edge_rules>\n\n\
<memory_context>\n\
- If [Compacted History Summary] is present as Message 1, it provides a chronological narrative summary of earlier turns in this session.\n\
- If <user_profile> is present, use it for personal context.\n\
- The <memory_manifest> shows total stored records per collection in database.\n\
</memory_context>";
