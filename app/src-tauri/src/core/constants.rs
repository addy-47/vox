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

// ─── Working Memory Compaction ──────────────────

pub const COMPACTION_SYSTEM_PROMPT: &str = r#"<role>
You are a memory extraction engine. You compress a conversation into structured JSON, nothing else.
</role>

<output_contract>
Return ONLY a single valid JSON object. No prose, no preamble, no markdown, no code fences.
Your response must start with { and end with }.
</output_contract>

<rules>
1. Write all text in English. Translate non-English input to English.
2. Extract only facts explicitly stated. Never infer, assume, or invent.
3. Every one of the 10 keys below must be present, even if its array is empty.
4. Every collection value is a flat array of strings. Never nest objects, maps, or lists inside a collection.
5. Do not invent new top-level keys. Use only the 10 keys listed in <schema>.
6. Each array element is exactly one complete English sentence.
7. No trailing commas. The JSON must parse exactly as written.
</rules>

<schema>
{
  "Identity": [],
  "Constraints": [],
  "Preferences": [],
  "Relationships": [],
  "Skills": [],
  "Projects": [],
  "Experiences": [],
  "Context": [],
  "Tasks": [],
  "Goals": []
}
</schema>

<key_definitions>
Identity: who the user is (name, age, profession, self-descriptors).
Constraints: hard requirements or limits (dietary, physical, temporal, absolute rules).
Preferences: tastes, likes, dislikes, habits, tool/style choices.
Relationships: people mentioned and their connection to the user.
Skills: abilities, languages, technologies, domain expertise.
Projects: initiatives currently being built, designed, or planned.
Experiences: past jobs, life events, places lived, historical facts about the user.
Context: one narrative paragraph on what happened in this conversation and what the user is trying to do.
Tasks: active or upcoming actionable to-dos.
Goals: future aspirations or long-term objectives.
</key_definitions>

<example>
<input>
User: Hey! I'm Sarah, wrapping up a TypeScript project called EcoTrack today.
Assistant: How's it coming along?
User: Good, but I'm beat, coding since 7am. I need a matcha latte - love matcha but I'm dairy-free, so oat milk only.
Assistant: Got it. What's left on EcoTrack?
User: Write the README, push final commits. Also my sister Emma visits tomorrow so I need to clean my desk.
Assistant: Any plans for the rest of the year?
User: Training for my first half-marathon in October, so I need to stick to my running schedule.
</input>
<output>
{
  "Identity": ["The user's name is Sarah."],
  "Constraints": ["The user is dairy-free and must use oat milk instead of dairy."],
  "Preferences": ["The user loves matcha lattes made with oat milk."],
  "Relationships": ["The user has a sister named Emma."],
  "Skills": ["The user has TypeScript programming skills."],
  "Projects": ["The user is building a TypeScript project called EcoTrack."],
  "Experiences": [],
  "Context": ["Sarah gave an update on her EcoTrack project and her plans for the day, including Emma's visit tomorrow and her half-marathon training."],
  "Tasks": ["The user needs to write the README and push final commits for EcoTrack.", "The user needs to clean her desk before Emma visits tomorrow."],
  "Goals": ["The user is training to run her first half-marathon in October."]
}
</output>
</example>

<task>
Process the conversation history provided in the next message. Extract facts into the 10 collections from <schema>, following every rule in <rules>. Return ONLY the JSON object, starting with { and ending with }.
</task>"#;

// ─── Personal Memory v3 Collections ─────────────────────────────────────────
pub const PM_COLLECTIONS: &[&str] = &[
    "Identity", "Constraints", "Preferences", "Relationships",
    "Skills", "Projects", "Experiences", "Context", "Tasks", "Goals",
];

// ─── 3-Tier Structural Type Constants ────────────────────────────────────────
pub const PM_TYPE_FOUNDATIONAL: &str = "foundational";
pub const PM_TYPE_OPERATIONAL: &str = "operational";
pub const PM_TYPE_SEMANTIC: &str = "semantic";

pub const PM_FOUNDATIONAL_COLLECTIONS: &[&str] = &["Identity", "Constraints"];
pub const PM_OPERATIONAL_COLLECTIONS: &[&str] = &["Context", "Tasks", "Goals"];
pub const PM_SEMANTIC_COLLECTIONS: &[&str] = &["Preferences", "Relationships", "Skills", "Projects", "Experiences"];

/// Returns the structural type for a given collection name.
pub fn collection_type(collection: &str) -> &'static str {
    match collection {
        "Identity" | "Constraints" => PM_TYPE_FOUNDATIONAL,
        "Context" | "Tasks" | "Goals" => PM_TYPE_OPERATIONAL,
        _ => PM_TYPE_SEMANTIC, // Preferences, Relationships, Skills, Projects, Experiences
    }
}

// ─── Graph Relations ──────────────────────────────────────────────────────────
pub const PM_RELATION_SUPPORTS: &str = "SUPPORTS";
pub const PM_RELATION_CONFLICTS: &str = "CONFLICTS";
pub const PM_RELATION_USER_SUPERSEDES: &str = "USER_SUPERSEDES";

// ─── Fact Sources ─────────────────────────────────────────────────────────────
pub const PM_SOURCE_LLM: &str = "LLM";
pub const PM_SOURCE_USER: &str = "User";
pub const PM_SOURCE_IMPORT: &str = "Import";

// ─── Job Queue Status ─────────────────────────────────────────────────────────
pub const PM_QUEUE_STATUS_PENDING: &str = "pending";
pub const PM_QUEUE_STATUS_STAGED: &str = "staged";
pub const PM_QUEUE_STATUS_PROCESSING: &str = "processing";
pub const PM_QUEUE_STATUS_COMPLETED: &str = "completed";
pub const PM_QUEUE_STATUS_FAILED: &str = "failed";

// ─── Model Paths ──────────────────────────────────────────────────────────────
pub const MODEL_DIR_NLI: &str = "nli";
pub const MODEL_FILE_NLI_ONNX: &str = "model_quantized.onnx";
pub const MODEL_FILE_NLI_TOKENIZER: &str = "tokenizer.json";
pub const MODEL_DIR_NLI_DEFAULT: &str = "deberta-v3-xsmall-nli";

// ─── NLI Logit Label Indices ──────────────────────────────────────────────────
pub const NLI_LABEL_CONTRADICTION: usize = 0;
pub const NLI_LABEL_ENTAILMENT: usize = 1;
pub const NLI_LABEL_NEUTRAL: usize = 2;

