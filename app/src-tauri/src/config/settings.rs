use serde::{Deserialize, Serialize};

use super::files::ProvidersConfigFile;
use crate::{
    core::{
        defaults::{
            DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_CADENCE,
            DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_TIME,
            DEFAULT_PERSONAL_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            DEFAULT_PERSONAL_MEMORY_PIPELINE_PROCESSING_ENABLED,
            DEFAULT_PERSONAL_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
            DEFAULT_PERSONAL_MEMORY_SUGGESTION_POLICY, DEFAULT_PERSONAL_MEMORY_TOP_K_FACTS,
            DEFAULT_SYSTEM_PROMPT_MODULAR, DEFAULT_SYSTEM_PROMPT_REALTIME, DEFAULT_UI_ACCENT_SEED,
            DEFAULT_UI_THEME, DEFAULT_WORKING_MEMORY_AUTO_COMPACTION,
            DEFAULT_WORKING_MEMORY_MAX_CONTEXT_SHARE, DEFAULT_WORKING_MEMORY_PRIVATE_MODE,
            DEFAULT_WORKING_MEMORY_WEB_SEARCH_ENABLED,
        },
        events::{InteractionMode, PipelineMode},
    },
    pipeline::dictation::DictationSettings,
    services::{
        llm::LlmSettings, realtime::RealtimeSettings, stt::SttSettings, tts::TtsSettings,
        vad::VadSettings,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum AudioOutputMode {
    #[default]
    Speaker,
    Headset,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: String,
    pub accent_seed: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_UI_THEME.into(),
            accent_seed: DEFAULT_UI_ACCENT_SEED.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(default)]
pub struct InteractionSettings {
    pub mode: InteractionMode,
    pub pipeline_mode: PipelineMode,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct WorkingMemorySettings {
    pub private_mode: bool,
    pub auto_compaction: bool,
    pub max_context_share: f32,
    pub web_search_enabled: bool,
}

impl Default for WorkingMemorySettings {
    fn default() -> Self {
        Self {
            private_mode: DEFAULT_WORKING_MEMORY_PRIVATE_MODE,
            auto_compaction: DEFAULT_WORKING_MEMORY_AUTO_COMPACTION,
            max_context_share: DEFAULT_WORKING_MEMORY_MAX_CONTEXT_SHARE,
            web_search_enabled: DEFAULT_WORKING_MEMORY_WEB_SEARCH_ENABLED,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct PersonalMemorySettings {
    pub context_retrieval_enabled: bool,
    pub pipeline_processing_enabled: bool,
    pub top_k_facts: u32,
    pub semantic_similarity_cutoff: f32,
    pub consolidation_cadence: String,
    pub consolidation_time: String,
    pub suggestion_policy: String,
}

impl Default for PersonalMemorySettings {
    fn default() -> Self {
        Self {
            context_retrieval_enabled: DEFAULT_PERSONAL_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            pipeline_processing_enabled: DEFAULT_PERSONAL_MEMORY_PIPELINE_PROCESSING_ENABLED,
            top_k_facts: DEFAULT_PERSONAL_MEMORY_TOP_K_FACTS,
            semantic_similarity_cutoff: DEFAULT_PERSONAL_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
            consolidation_cadence: DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_CADENCE.to_string(),
            consolidation_time: DEFAULT_PERSONAL_MEMORY_CONSOLIDATION_TIME.to_string(),
            suggestion_policy: DEFAULT_PERSONAL_MEMORY_SUGGESTION_POLICY.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PersonaSettings {
    pub modular_prompt: String,
    pub realtime_prompt: String,
}

impl Default for PersonaSettings {
    fn default() -> Self {
        Self {
            modular_prompt: DEFAULT_SYSTEM_PROMPT_MODULAR.into(),
            realtime_prompt: DEFAULT_SYSTEM_PROMPT_REALTIME.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SystemSettings {
    pub setup_completed: bool,
}

/// The unified application settings aggregating domain subsystem configurations.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct VoxSettings {
    pub appearance: AppearanceSettings,
    pub vad: VadSettings,
    pub interaction: InteractionSettings,
    pub dictation: DictationSettings,
    pub system: SystemSettings,
    pub working_memory: WorkingMemorySettings,
    pub personal_memory: PersonalMemorySettings,
    pub persona: PersonaSettings,
    pub stt: SttSettings,
    pub llm: LlmSettings,
    pub tts: TtsSettings,
    pub realtime: RealtimeSettings,
    pub providers: ProvidersConfigFile,
}

pub fn get_preset_colors() -> Vec<String> {
    vec![
        "#00DBE9".to_string(),
        "#8B5CF6".to_string(),
        "#EC4899".to_string(),
        "#F59E0B".to_string(),
        "#10B981".to_string(),
    ]
}
