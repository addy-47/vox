use serde::{Deserialize, Serialize};

use super::settings::{
    AppearanceSettings, InteractionSettings, PersonaSettings, PersonalMemorySettings,
    SystemSettings, VoxSettings, WorkingMemorySettings,
};
use crate::{
    pipeline::dictation::DictationSettings,
    services::{
        llm::{LlmProvidersConfig, LlmSettings},
        realtime::{RealtimeProvidersConfig, RealtimeSettings},
        stt::{SttProvidersConfig, SttSettings},
        tts::{TtsProvidersConfig, TtsSettings},
        vad::VadSettings,
    },
};

/// Tier 1: Hardware, Audio I/O, VAD, Theme, Memory, and Active Engine Pointers (`config/settings.jsonc`, chmod 0644)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SettingsConfigFile {
    pub appearance: AppearanceSettings,
    pub vad: VadSettings,
    pub interaction: InteractionSettings,
    pub dictation: DictationSettings,
    pub system: SystemSettings,
    pub working_memory: WorkingMemorySettings,
    pub personal_memory: PersonalMemorySettings,
    pub stt: SttSettings,
    pub llm: LlmSettings,
    pub tts: TtsSettings,
    pub realtime: RealtimeSettings,
}

/// Tier 2: Cloud/Server Endpoints, Custom Servers, Model IDs, API Keys (`config/providers.jsonc`, chmod 0600)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ProvidersConfigFile {
    pub llm: LlmProvidersConfig,
    pub stt: SttProvidersConfig,
    pub tts: TtsProvidersConfig,
    pub realtime: RealtimeProvidersConfig,
}

/// Tier 3: Cognitive Persona Prompts (`config/agent.jsonc`, chmod 0644)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AgentConfigFile {
    pub persona: PersonaSettings,
}

impl VoxSettings {
    pub fn to_settings_config(&self) -> SettingsConfigFile {
        SettingsConfigFile {
            appearance: self.appearance.clone(),
            vad: self.vad.clone(),
            interaction: self.interaction.clone(),
            dictation: self.dictation.clone(),
            system: self.system.clone(),
            working_memory: self.working_memory.clone(),
            personal_memory: self.personal_memory.clone(),
            stt: self.stt.clone(),
            llm: self.llm.clone(),
            tts: self.tts.clone(),
            realtime: self.realtime.clone(),
        }
    }

    pub fn to_providers_config(&self) -> ProvidersConfigFile {
        self.providers.clone()
    }

    pub fn to_agent_config(&self) -> AgentConfigFile {
        AgentConfigFile {
            persona: self.persona.clone(),
        }
    }

    pub fn merge_settings_config(&mut self, cfg: SettingsConfigFile) {
        self.appearance = cfg.appearance;
        self.vad = cfg.vad;
        self.interaction = cfg.interaction;
        self.dictation = cfg.dictation;
        self.system = cfg.system;
        self.working_memory = cfg.working_memory;
        self.personal_memory = cfg.personal_memory;
        self.stt = cfg.stt;
        self.llm = cfg.llm;
        self.tts = cfg.tts;
        self.realtime = cfg.realtime;
    }

    pub fn merge_providers_config(&mut self, cfg: ProvidersConfigFile) {
        self.providers = cfg;
    }

    pub fn merge_agent_config(&mut self, cfg: AgentConfigFile) {
        self.persona = cfg.persona;
    }
}
