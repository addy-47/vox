use serde::{Deserialize, Serialize};

use crate::core::defaults::{
    DEFAULT_DICTATION_ENABLED, DEFAULT_DICTATION_HOTKEY,
    DEFAULT_DICTATION_SILENCE_AUTO_STOP_MS,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DictationInteractionMode {
    Passive,
    #[default]
    Ptt,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DictationOutputMode {
    #[default]
    Paste,
    Clipboard,
    Tray,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct DictationSettings {
    pub enabled: bool,
    pub interaction_mode: DictationInteractionMode,
    pub hotkey: String,
    pub output_mode: DictationOutputMode,
    pub silence_auto_stop_ms: u64,
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_DICTATION_ENABLED,
            interaction_mode: DictationInteractionMode::Ptt,
            hotkey: DEFAULT_DICTATION_HOTKEY.into(),
            output_mode: DictationOutputMode::Paste,
            silence_auto_stop_ms: DEFAULT_DICTATION_SILENCE_AUTO_STOP_MS,
        }
    }
}
