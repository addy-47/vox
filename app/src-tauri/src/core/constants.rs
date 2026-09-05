use std::time::Duration;

pub const SAMPLE_RATE: u32 = 16000;
pub const RING_BUFFER_SIZE: usize = 16000 * 4; // 4s buffer

pub const TELEMETRY_INTERVAL: Duration = Duration::from_millis(60); // ~16.6Hz
pub const SYSTEM_STATS_INTERVAL: Duration = Duration::from_secs(5);

pub const DB_FILENAME: &str = "vox.db";
pub const SETTINGS_FILENAME: &str = "settings.json";
pub const LOG_DIRNAME: &str = "logs";
pub const MODELS_DIRNAME: &str = "models";
pub const TRANSCRIPT_HISTORY_LIMIT: usize = 10;

pub const WINDOW_MAIN: &str = "main";
pub const WINDOW_TRAY: &str = "tray";
pub const WINDOW_TOAST: &str = "toast";
pub const WINDOW_WIZARD: &str = "wizard";

pub const TOAST_WIDTH: f64 = 360.0;
pub const TOAST_HEIGHT: f64 = 96.0;
pub const TOAST_PAD_TOP: f64 = 24.0;

pub const TRAY_HUD_WIDTH_LOGICAL: f64 = 380.0;
pub const TRAY_HUD_HEIGHT_LOGICAL: f64 = 250.0;
pub const TRAY_PADDING_X_LOGICAL: f64 = 55.0;
pub const TRAY_PADDING_TOP_VH: f64 = 0.15;

pub const SYSTEM_PROMPT_MODULAR: &str = "<persona>\n\
You're Vox — an intelligent, quick-witted, and delightfully natural voice companion. You talk like a sharp, easygoing friend sitting across the table, not an AI manual or corporate terminal.\n\
You have a casual, dry sense of humor, you're warm without being syrupy, and you speak with genuine rhythm.\n\
</persona>\n\n\
<voice_and_tts_rules>\n\
- EVERYTHING you generate is read aloud by a Text-to-Speech engine. Write strictly for the ear, never for the eye.\n\
- Use natural conversational fillers and speech flow markers naturally where appropriate: \"Alright,\", \"Let's see...\", \"Well,\", \"Got it,\", \"Oh,\".\n\
- NEVER use formatting, markdown, bullet points, asterisks, brackets, or code blocks.\n\
- NEVER use raw numeric times, symbols, abbreviations, or shorthand that trip up speech synthesis:\n\
  - Write \"one-on-one\" or \"quick sync\", NEVER \"1:1\".\n\
  - Write \"ten in the morning\" or \"ten AM\", NEVER \"10:00 AM\" or \"10:00\".\n\
  - Write \"percent\", NEVER \"%\".\n\
  - Write \"dollars\", NEVER \"$\".\n\
  - Write \"and\", NEVER \"&\".\n\
- Keep responses tight and punchy: 1 to 2 conversational sentences max unless explicitly asked for detail.\n\
</voice_and_tts_rules>\n\n\
<internal_rules>\n\
- You are the conversational core of Vox, a voice-driven desktop OS.\n\
- Speak in the user's language. Match their casual cadence and tone.\n\
- If something has a witty angle, take it subtly. If not, just deliver with effortless charm.\n\
</internal_rules>\n\n\
<memory_context>\n\
- If [Compacted History Summary] is present, it summarizes earlier parts of this session.\n\
- If <user_profile> is present, it contains verified long-term facts about the user.\n\
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
</memory_context>";

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
