//! ============================================================================
//! tests/common/mod.rs — Shared Test Harness and Infrastructure Modules
//! ============================================================================

#![allow(dead_code)]

pub mod audio;
pub mod harness;
pub mod paths;
pub mod scoring;

// ============================================================================
// Ground Truth Constants for Test Clips (`test-clips/README.md` & `tests/assets/`)
// ============================================================================

pub const CLIP_01_EN_FILENAME: &str = "clip_01_en_briefing.wav";
pub const CLIP_01_EN_GROUND_TRUTH: &str =
    "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?";

pub const CLIP_02_EN_FILENAME: &str = "clip_02_en_weather.wav";
pub const CLIP_02_EN_GROUND_TRUTH: &str =
    "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?";

pub const CLIP_03_EN_FILENAME: &str = "clip_03_en_code.wav";
pub const CLIP_03_EN_GROUND_TRUTH: &str =
    "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?";

pub const CLIP_04_EN_FILENAME: &str = "clip_04_en_summary.wav";
pub const CLIP_04_EN_GROUND_TRUTH: &str =
    "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.";

pub const CLIP_05_EN_FILENAME: &str = "clip_05_en_timer.wav";
pub const CLIP_05_EN_GROUND_TRUTH: &str =
    "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.";

pub const CLIP_06_HI_FILENAME: &str = "clip_06_hi_greeting.wav";
pub const CLIP_06_HI_GROUND_TRUTH: &str =
    "हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?";

pub const CLIP_07_HI_FILENAME: &str = "clip_07_hi_weather.wav";
pub const CLIP_07_HI_GROUND_TRUTH: &str =
    "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?";

pub const CLIP_08_HI_FILENAME: &str = "clip_08_hi_reminder.wav";
pub const CLIP_08_HI_GROUND_TRUTH: &str =
    "मेरे लिए एक ज़रूरी रिमाइंडर सेट कर दो, शाम को पाँच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है।";

pub const CLIP_09_HI_FILENAME: &str = "clip_09_hi_system_cmd.wav";
pub const CLIP_09_HI_GROUND_TRUTH: &str =
    "वॉक्स, टर्मिनल खोलिए और हाई परफॉरमेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए।";

pub const CLIP_10_HI_FILENAME: &str = "clip_10_hi_qa.wav";
pub const CLIP_10_HI_GROUND_TRUTH: &str =
    "वॉक्स, मुझे समझाइए कि मशीन लर्निंग में स्पीच-टू-टेक्स्ट मॉडल इतनी तेज़ी से आवाज़ कैसे पहचानते हैं?";

// Legacy asset aliases
pub const ASSET_EDGETTS_01_EN_FILENAME: &str = "edgetts_01_en_briefing.wav";
pub const ASSET_EDGETTS_01_EN_GROUND_TRUTH: &str = CLIP_01_EN_GROUND_TRUTH;

pub const ASSET_EDGETTS_07_HI_FILENAME: &str = "edgetts_07_hi_weather.wav";
pub const ASSET_EDGETTS_07_HI_GROUND_TRUTH: &str = CLIP_07_HI_GROUND_TRUTH;

// Supertonic renders (single-utterance by construction: max internal pause
// 0.4s EN / 0.3s HI, below the 800ms production silence default).
// Ground truth shares the briefing/weather sentences (verified at runtime
// by the >= 0.90 similarity gate in passive_streaming_test.rs).
pub const ASSET_SUPERTONIC_01_EN_FILENAME: &str = "supertonic_01_en_briefing.wav";
pub const ASSET_SUPERTONIC_01_EN_GROUND_TRUTH: &str = CLIP_01_EN_GROUND_TRUTH;

pub const ASSET_SUPERTONIC_07_HI_FILENAME: &str = "supertonic_07_hi_weather.wav";
pub const ASSET_SUPERTONIC_07_HI_GROUND_TRUTH: &str = CLIP_07_HI_GROUND_TRUTH;
