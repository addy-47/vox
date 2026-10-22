use serde::{Deserialize, Serialize};

use super::files::ProvidersConfigFile;
use crate::{
    core::{
        defaults::{
            DEFAULT_INTERACTION_WEB_SEARCH_ENABLED, DEFAULT_MEMORY_AUTO_COMPACTION,
            DEFAULT_MEMORY_CONSOLIDATION_CADENCE, DEFAULT_MEMORY_CONSOLIDATION_TIME,
            DEFAULT_MEMORY_CONTEXT_RETRIEVAL_ENABLED, DEFAULT_MEMORY_MAX_CONTEXT_SHARE,
            DEFAULT_MEMORY_PIPELINE_PROCESSING_ENABLED, DEFAULT_MEMORY_PRIVATE_MODE,
            DEFAULT_MEMORY_SEMANTIC_SIMILARITY_CUTOFF, DEFAULT_MEMORY_SUGGESTION_POLICY,
            DEFAULT_MEMORY_TOP_K_FACTS, DEFAULT_REMOTE_ACCEPTS_CONTROLLERS,
            DEFAULT_SYSTEM_PROMPT_MODULAR, DEFAULT_SYSTEM_PROMPT_REALTIME, DEFAULT_UI_ACCENT_SEED,
            DEFAULT_UI_THEME,
        },
        events::{InteractionMode, PipelineMode, RemoteExecutionMode},
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct InteractionSettings {
    pub mode: InteractionMode,
    pub pipeline_mode: PipelineMode,
    pub web_search_enabled: bool,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            mode: InteractionMode::default(),
            pipeline_mode: PipelineMode::default(),
            web_search_enabled: DEFAULT_INTERACTION_WEB_SEARCH_ENABLED,
        }
    }
}

/// Memory policy settings. One flat scope covering working-memory behaviour
/// (privacy, compaction, context share) and personal-memory behaviour
/// (retrieval, fact budgets, consolidation, suggestions).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct MemorySettings {
    pub context_retrieval_enabled: bool,
    pub pipeline_processing_enabled: bool,
    pub private_mode: bool,
    pub consolidation_cadence: String,
    pub consolidation_time: String,
    pub suggestion_policy: String,
    pub auto_compaction: bool,
    pub max_context_share: f32,
    pub top_k_facts: u32,
    pub semantic_similarity_cutoff: f32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            context_retrieval_enabled: DEFAULT_MEMORY_CONTEXT_RETRIEVAL_ENABLED,
            pipeline_processing_enabled: DEFAULT_MEMORY_PIPELINE_PROCESSING_ENABLED,
            private_mode: DEFAULT_MEMORY_PRIVATE_MODE,
            consolidation_cadence: DEFAULT_MEMORY_CONSOLIDATION_CADENCE.to_string(),
            consolidation_time: DEFAULT_MEMORY_CONSOLIDATION_TIME.to_string(),
            suggestion_policy: DEFAULT_MEMORY_SUGGESTION_POLICY.to_string(),
            auto_compaction: DEFAULT_MEMORY_AUTO_COMPACTION,
            max_context_share: DEFAULT_MEMORY_MAX_CONTEXT_SHARE,
            top_k_facts: DEFAULT_MEMORY_TOP_K_FACTS,
            semantic_similarity_cutoff: DEFAULT_MEMORY_SEMANTIC_SIMILARITY_CUTOFF,
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

/// The remote-control settings domain.
///
/// Two independent settings, one per role (remote-control spec §4.2):
/// - `accepts_controllers` is host-side. When false the host behaves exactly
///   like a standalone installation.
/// - `execution_mode` is controller-side. `local` runs the device's own
///   pipeline; `remote` makes it a thin client for a paired host.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct RemoteSettings {
    pub accepts_controllers: bool,
    pub execution_mode: RemoteExecutionMode,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            accepts_controllers: DEFAULT_REMOTE_ACCEPTS_CONTROLLERS,
            execution_mode: RemoteExecutionMode::default(),
        }
    }
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
    pub remote: RemoteSettings,
    pub memory: MemorySettings,
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
