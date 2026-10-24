use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SidecarKind {
    Llm,
    TtsClone,
    TtsCloud,
}

impl SidecarKind {
    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::Llm => "vox-llm",
            Self::TtsClone => "vox-tts-clone",
            Self::TtsCloud => "vox-tts-cloud",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Llm => "Local LLM Runtime",
            Self::TtsClone => "Voice Cloning Runtime",
            Self::TtsCloud => "Cloud Speech Runtime",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Self::Llm => 18081,
            Self::TtsClone => 18082,
            Self::TtsCloud => 18083,
        }
    }
}
