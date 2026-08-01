use std::time::Duration;

// ─── Audio Constraints ───────────────────────────────────────────────────────
pub const SAMPLE_RATE: u32 = 16000;
pub const RING_BUFFER_SIZE: usize = 16000 * 4; // 4s buffer

// ─── Timing & Throttling ─────────────────────────────────────────────────────
pub const TELEMETRY_INTERVAL: Duration = Duration::from_millis(60); // ~16.6Hz
pub const STT_THROTTLE_MS: u64 = 800;
pub const SYSTEM_STATS_INTERVAL: Duration = Duration::from_secs(5);

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
pub const SYSTEM_PROMPT_MODULAR: &str = "<persona>\n\
You're Vox. Quick, sharp, and you get things done. No preamble, no padding — just say what needs saying. You've got a dry wit and zero interest in sounding like a corporation. Every response is spoken, so it needs to breathe right: short sentences, natural rhythm, clean flow. No lists, no formatting, no markdown.\n\
</persona>\n\n\
<internal_rules>\n\
- You are the LLM of a realtime voice pipeline.\n\
- Your responses are converted to speech by a TTS model.\n\
- You are the backbone of the Vox application which aims to be a voice-driven OS where any and all tasks possible to do on user device can be achieved via Vox.\n\
</internal_rules>\n\n\
<guidelines>\n\
- Speak in <lang>, write in <script>. Never mix scripts.\n\
- Short is better. One idea per sentence. Let it land.\n\
- If something's funny, say it. If not, don't force it.\n\
</guidelines>\n\n\
<memory_context>\n\
- If [Compacted History Summary] is present as Message 1, it provides a chronological narrative summary of earlier turns in this session.\n\
- If <user_profile> is present, it contains verified long-term personal facts about the user.\n\
- The <memory_manifest> header lists total active records per collection in database. If a specific user detail is not in the injected profile, know that additional historical records exist in the database.\n\
</memory_context>";

pub const SYSTEM_PROMPT_REALTIME: &str = "<persona>\n\
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
</memory_context>";// ─── Transition Speech Assets (Working Memory Maintenance) ──────────────────

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
You are an objective, high-precision memory extraction engine. Your task is to compress a conversation transcript into a structured JSON object containing 6 orthogonal collections.
</role>

<output_contract>
Return ONLY a single valid JSON object starting with { and ending with }.
Zero prose, zero preamble, zero markdown formatting, zero code fences.
The JSON object MUST contain exactly 6 keys: "Identity", "Directives", "Narrative", "Profile", "Entities", "Constraints" where "Narrative" is a single paragraph and rest are arrays.
</output_contract>

<rules>
1. Translate all non-English input to clear, standard English.
2. Extract only explicitly stated or directly demonstrated facts. Never infer, assume, or hallucinate.
3.. Each sentence in array collections must be a standalone atomic fact that can be understood in isolation.
</rules>

<schema>
{
  "Identity": [],
  "Directives": [],
  "Narrative": "",
  "Profile": [],
  "Entities": [],
  "Constraints": []
}
</schema>

<key_definitions>
Identity: Present-tense, non-transient baseline facts establishing who/what the primary entities in the interaction are (core role, name, baseline attributes).
Directives: Active operational state, standing rules, assigned tasks/goals, agent progress, commitments, current working state, and blockers since last compaction from the operational perspective.
Narrative: A single continuous summary paragraph capturing the temporal flow, milestones, and progress of this interaction session.
Profile: Soft attributes, personal/agent traits, habits, skills, preferences, background experience, and behavioral tendencies.
Entities: Named, discrete external subjects/objects outside the user/agent persona — projects, tools, systems, organizations, locations or third-party people.
Constraints: Hard, inviolable limits, physical/safety restrictions, technical dealbreakers, or absolute non-negotiable boundaries where violation causes breaking failure or harm.
</key_definitions>

<boundary_disambiguation>
- Identity vs Profile: Identity is core present-tense baseline who/what the subject fundamentally is (name, primary role, core identity). Profile holds personal traits, habits, skills, background, or personal facts ("User speaks French", "User is a marathon runner").
- Directives vs Constraints: Directives are active positive instructions, assigned tasks, agent goals, progress, or standing rules ("Format output as markdown", "Summarize weekly"). Constraints are hard negative boundaries or inviolable execution limits ("Never use raw pointers", "Do not open external URLs").
- Constraints vs Profile: Constraints are absolute operational rules or negative boundaries for the agent ("Don't do X", "Never open Y"). Profile holds personal user attributes, allergies, soft preferences, or likes/dislikes ("User has a severe peanut allergy", "User dislikes dark mode").
- Entities vs Profile: Entities are external facts about third-party named objects, projects, systems, or codebases that don't directly describe the user or agent themselves ("Turso DB uses SQLite", "Acme Corp is based in NY"). Profile holds internal capabilities, traits, or attributes belonging to the user.
</boundary_disambiguation>"#;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MemoryCollection {
    Identity,
    Directives,
    Narrative,
    Profile,
    Entities,
    Constraints,
}

impl MemoryCollection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Directives => "Directives",
            Self::Narrative => "Narrative",
            Self::Profile => "Profile",
            Self::Entities => "Entities",
            Self::Constraints => "Constraints",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Identity" => Some(Self::Identity),
            "Directives" => Some(Self::Directives),
            "Narrative" => Some(Self::Narrative),
            "Profile" => Some(Self::Profile),
            "Entities" => Some(Self::Entities),
            "Constraints" => Some(Self::Constraints),
            _ => None,
        }
    }

    pub fn collection_type(&self) -> &'static str {
        match self {
            Self::Identity | Self::Directives | Self::Narrative => PM_TYPE_SPECIAL_STATE,
            _ => PM_TYPE_SEMANTIC_GRAPH,
        }
    }
}

impl std::fmt::Display for MemoryCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─── Personal Memory Collections & Taxonomy ─────────────────────────────
pub const PM_COLLECTIONS: &[&str] = &[
    "Identity", "Directives", "Narrative", "Profile", "Entities", "Constraints",
];

// ─── Collection Structural Classes ───────────────────────────────────────────
pub const PM_TYPE_SPECIAL_STATE: &str = "special_state";
pub const PM_TYPE_SEMANTIC_GRAPH: &str = "semantic_graph";

pub const PM_SPECIAL_STATE_COLLECTIONS: &[&str] = &["Identity", "Directives", "Narrative"];
pub const PM_SEMANTIC_GRAPH_COLLECTIONS: &[&str] = &["Profile", "Entities", "Constraints"];

/// Returns the structural type for a given collection name.
pub fn collection_type(collection: &str) -> &'static str {
    if let Some(col) = MemoryCollection::parse(collection) {
        col.collection_type()
    } else {
        PM_TYPE_SEMANTIC_GRAPH
    }
}

// ─── Graph Relations ──────────────────────────────────────────────────────────
pub const PM_RELATION_SUPPORTS: &str = "SUPPORTS";
pub const PM_RELATION_CONFLICTS: &str = "CONFLICTS";
pub const PM_RELATION_SUPERSEDES: &str = "SUPERSEDES";
pub const PM_RELATION_SHAPES: &str = "SHAPES";
pub const PM_RELATION_DEPENDS_ON: &str = "DEPENDS_ON";

// ─── Inter-Collection Edge Policy Matrix ──────────────────────────────────────
/// Returns (forward_edge, deterministic_inverse_edge) for valid v7 collection pairs (spec §4.2).
/// Returns None if no inter-collection relation policy exists for the pair.
pub fn inter_collection_edge(src: &str, tgt: &str) -> Option<(&'static str, &'static str)> {
    match (src, tgt) {
        ("Identity", "Profile") => Some((PM_RELATION_SHAPES, "shaped_by")),
        ("Directives", "Constraints") => Some((PM_RELATION_SHAPES, "shaped_by")),
        ("Directives", "Entities") => Some((PM_RELATION_DEPENDS_ON, "dependency_of")),
        ("Entities", "Constraints") => Some((PM_RELATION_DEPENDS_ON, "constrains")),
        ("Entities", "Profile") => Some((PM_RELATION_SHAPES, "shaped_by")),
        ("Entities", "Entities") => Some((PM_RELATION_DEPENDS_ON, "dependency_of")),
        ("Profile", "Profile") => Some((PM_RELATION_SHAPES, "shaped_by")),
        ("Profile", "Entities") => Some((PM_RELATION_SHAPES, "shaped_by")),
        ("Profile", "Constraints") => Some(("restricted_by", "restricts")),
        _ => None,
    }
}

/// Returns the deterministic inverse relation string for an NLI relation (spec §4.3.1).
pub fn nli_inverse_edge(relation: &str) -> &'static str {
    match relation {
        PM_RELATION_SUPPORTS => "supported_by",
        PM_RELATION_SUPERSEDES => "superseded_by",
        PM_RELATION_CONFLICTS => "conflicts_with",
        _ => "related_to",
    }
}

// ─── Fact Sources ─────────────────────────────────────────────────────────────
pub const PM_SOURCE_LLM: &str = "LLM";
pub const PM_SOURCE_USER: &str = "User";
pub const PM_SOURCE_IMPORT: &str = "Import";
pub const PM_SOURCE_NLI: &str = "NLI";

// ─── v7 4-Stage Pipeline Queue Status Constants ───────────────────────────────
pub const PM_QUEUE_STATUS_STAGED_PENDING: &str = "staged_pending";
pub const PM_QUEUE_STATUS_PROCESSING_DEDUP: &str = "processing_dedup";
pub const PM_QUEUE_STATUS_DEDUPED: &str = "deduped";
pub const PM_QUEUE_STATUS_PROCESSING_EMBED: &str = "processing_embed";
pub const PM_QUEUE_STATUS_EMBEDDED: &str = "embedded";
pub const PM_QUEUE_STATUS_PROCESSING_EVAL: &str = "processing_eval";
pub const PM_QUEUE_STATUS_EVALUATED: &str = "evaluated";
pub const PM_QUEUE_STATUS_PROCESSING_COMMIT: &str = "processing_commit";
pub const PM_QUEUE_STATUS_SUPERSEDED: &str = "superseded";
pub const PM_QUEUE_STATUS_COMPLETED: &str = "completed";
pub const PM_QUEUE_STATUS_FAILED: &str = "failed";
pub const PM_QUEUE_STATUS_PAUSED: &str = "paused";



