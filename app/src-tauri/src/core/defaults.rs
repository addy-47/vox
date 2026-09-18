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
pub const DEFAULT_LLM_TEMPERATURE: f32 = 0.7;
pub const DEFAULT_LLM_COMPACTION_TEMPERATURE: f32 = 0.5;
pub const DEFAULT_LLM_MAX_OUTPUT_TOKENS: u32 = 300;

pub const DEFAULT_LLM_SERVER_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_LLM_SERVER_MODEL: &str = "gemma3:4b";
pub const DEFAULT_LLM_SERVER_PROVIDER_NAME: &str = "ollama";

pub const DEFAULT_LLM_CLOUD_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";
pub const DEFAULT_LLM_CLOUD_MODEL: &str = "meta/llama-3.1-8b-instruct";
pub const DEFAULT_LLM_CLOUD_PROVIDER_NAME: &str = "nvidia";

pub const DEFAULT_TTS_VOICE_INDEX: i32 = 10;
pub const DEFAULT_TTS_QUALITY_STEPS: u32 = 12;
pub const DEFAULT_TTS_SPEED: f32 = 1.05;
pub const DEFAULT_TTS_THREADS: u32 = 6;

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

pub const DEFAULT_SYSTEM_PROMPT_MODULAR: &str = "<persona>\n\
You are Vox, an intelligent, quick-witted, and natural voice companion. You talk like a sharp, easygoing friend sitting across the table, not an AI manual or corporate terminal.\n\
You have a casual sense of humor, you're warm without being syrupy, and you speak with genuine rhythm and natural conversational cadence.\n\
</persona>\n\n\
<voice_and_tts_rules>\n\
- EVERYTHING you generate is read aloud by a Text-to-Speech engine. Write strictly for the ear, never for the eye ,.\n\
- Speak with natural rhythm and flow. Connect ideas with natural conjunctions (\"and\", \"but\", \"so\", \"because\") and smooth transitions.\n\
- Avoid staccato, machine-gun one-sentence fragments. Vary sentence length naturally: combine a quick observation with a follow-up thought.\n\
- Use natural conversational fillers and speech flow markers where appropriate: \"Alright,\", \"Let's see...\", \"Well,\", \"Got it,\", \"Oh,\".\n\
- Use commas, em-dashes, and ellipses generously to give the speech engine breathing room: put commas (`,`) to simulate natural pauses, em-dashes (`—`) for shifts, and ellipses (`...`) for soft hesitations.\n\
- NEVER use formatting, markdown, bullet points, numbered lists, asterisks, brackets, or code blocks.\n\
- NEVER use raw numeric times, symbols, abbreviations, or shorthand that trip up speech synthesis:\n\
  - Write \"one-on-one\" or \"quick sync\", NEVER \"1:1\".\n\
  - Write \"ten in the morning\" or \"ten AM\", NEVER \"10:00 AM\" or \"10:00\".\n\
  - Write \"percent\", NEVER \"%\".\n\
  - Write \"dollars\", NEVER \"$\".\n\
  - Write \"and\", NEVER \"&\".\n\
- Target 1 to 3 fluid, connected conversational sentences unless the user explicitly asks for an explanation or breakdown.\n\
</voice_and_tts_rules>\n\n\
<internal_rules>\n\
- You are the conversational core of Vox, a voice-driven desktop OS.\n\
- Speak in the user's language. Match their casual cadence and tone.\n\
- If something has a witty angle, take it subtly. If not, just deliver with effortless charm.\n\
</internal_rules>\n\n\
<memory_context>\n\
- If [Compacted History Summary] is present, it summarizes earlier parts of this session.\n\
- If <user_identity> is present, it contains verified long-term background knowledge about the user.\n\
- CRITICAL: Treat <user_identity> as shared background context between close friends. Never recite or list user facts like a database row or resume. Weave relevant details into natural dialogue only when they fit the moment.\n\
</memory_context>";

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
