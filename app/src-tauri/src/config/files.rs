use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::settings::{
    AppearanceSettings, AudioSettings, InteractionSettings, PersonalMemorySettings,
    PersonaSettings, RealtimeSettings, SystemSettings, VoxSettings, WorkingMemorySettings,
};
use crate::{
    core::defaults::{
        DEFAULT_LLM_COMPACTION_TEMPERATURE, DEFAULT_LLM_CONTEXT_WINDOW,
        DEFAULT_LLM_MAX_OUTPUT_TOKENS, DEFAULT_LLM_TEMPERATURE,
    },
    pipeline::dictation::DictationSettings,
    services::{
        llm::{LlmActiveProvider, LlmEmbeddedConfig, LlmRemoteConfig, LlmSettings},
        stt::{SttActiveProvider, SttCloudConfig, SttEmbeddedConfig, SttSettings},
        tts::{
            TtsActiveProvider, TtsChatterboxConfig, TtsChatterboxRemoteConfig, TtsEdgeConfig,
            TtsKokoroConfig, TtsSupertonicConfig, TtsSettings, TtsZipvoiceConfig,
        },
        vad::VadSettings,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SttWiringSettings {
    pub active: SttActiveProvider,
    #[serde(alias = "transliterate")]
    pub transliterate_enabled: bool,
    pub embedded: SttEmbeddedConfig,
}

impl Default for SttWiringSettings {
    fn default() -> Self {
        let d = SttSettings::default();
        Self {
            active: d.active,
            transliterate_enabled: d.transliterate_enabled,
            embedded: d.embedded,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LlmWiringSettings {
    pub active: LlmActiveProvider,
    pub threads: u32,
    pub embedded: LlmEmbeddedConfig,
}

impl Default for LlmWiringSettings {
    fn default() -> Self {
        let d = LlmSettings::default();
        Self {
            active: d.active,
            threads: d.threads,
            embedded: d.embedded,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TtsWiringSettings {
    pub active: TtsActiveProvider,
    #[serde(alias = "voice")]
    pub voice_index: i32,
    pub speed: f32,
    pub threads: u32,
    pub supertonic: TtsSupertonicConfig,
    pub kokoro: TtsKokoroConfig,
}

impl Default for TtsWiringSettings {
    fn default() -> Self {
        let d = TtsSettings::default();
        Self {
            active: d.active,
            voice_index: d.voice_index,
            speed: d.speed,
            threads: d.threads,
            supertonic: d.supertonic,
            kokoro: d.kokoro,
        }
    }
}

/// Tier 1: Hardware, Audio I/O, VAD, Theme, Active Engine Wiring (`config/settings.jsonc`)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SettingsConfigFile {
    pub audio: AudioSettings,
    pub vad: VadSettings,
    pub appearance: AppearanceSettings,
    pub interaction: InteractionSettings,
    pub dictation: DictationSettings,
    pub system: SystemSettings,
    pub stt: SttWiringSettings,
    pub llm: LlmWiringSettings,
    pub tts: TtsWiringSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SttProvidersConfig {
    pub cloud: SttCloudConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LlmProvidersConfig {
    pub server: LlmRemoteConfig,
    pub cloud: LlmRemoteConfig,
    #[serde(default)]
    pub cloud_keys: HashMap<String, String>,
}

impl Default for LlmProvidersConfig {
    fn default() -> Self {
        Self {
            server: LlmRemoteConfig::server_default(),
            cloud: LlmRemoteConfig::cloud_default(),
            cloud_keys: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TtsProvidersConfig {
    pub edge_tts: TtsEdgeConfig,
    pub chatterbox: TtsChatterboxConfig,
    pub chatterbox_remote: TtsChatterboxRemoteConfig,
    pub zipvoice: TtsZipvoiceConfig,
}

/// Tier 2: Cloud/Server Endpoints, Model IDs, API Keys (`config/providers.jsonc`, chmod 0600)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ProvidersConfigFile {
    pub llm: LlmProvidersConfig,
    pub stt: SttProvidersConfig,
    pub tts: TtsProvidersConfig,
    pub realtime: RealtimeSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct CognitiveSettings {
    pub temperature: f32,
    pub compaction_temperature: f32,
    pub max_output_tokens: u32,
    pub context_window: u32,
    pub reasoning_enabled: bool,
}

impl Default for CognitiveSettings {
    fn default() -> Self {
        Self {
            temperature: DEFAULT_LLM_TEMPERATURE,
            compaction_temperature: DEFAULT_LLM_COMPACTION_TEMPERATURE,
            max_output_tokens: DEFAULT_LLM_MAX_OUTPUT_TOKENS,
            context_window: DEFAULT_LLM_CONTEXT_WINDOW,
            reasoning_enabled: false,
        }
    }
}

/// Tier 3: Cognitive Persona, Prompts, Tools, MCP, Memory Policy (`config/agent.jsonc`)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AgentConfigFile {
    pub persona: PersonaSettings,
    pub working_memory: WorkingMemorySettings,
    pub personal_memory: PersonalMemorySettings,
    pub cognitive: CognitiveSettings,
}

impl VoxSettings {
    /// Strips the retired `zipvoice_voice_` id prefix from the stored voice selection.
    pub(crate) fn normalize_retired_voice_ids(&mut self) {
        if let Some(id) = self.tts.zipvoice.voice_id.as_mut() {
            if let Some(slug) = id.strip_prefix("zipvoice_voice_").map(|s| s.to_string()) {
                *id = slug;
            }
        }
    }

    pub fn to_settings_config(&self) -> SettingsConfigFile {
        SettingsConfigFile {
            audio: self.audio.clone(),
            vad: self.vad.clone(),
            appearance: self.appearance.clone(),
            interaction: self.interaction.clone(),
            dictation: self.dictation.clone(),
            system: self.system.clone(),
            stt: SttWiringSettings {
                active: self.stt.active,
                transliterate_enabled: self.stt.transliterate_enabled,
                embedded: self.stt.embedded.clone(),
            },
            llm: LlmWiringSettings {
                active: self.llm.active,
                threads: self.llm.threads,
                embedded: self.llm.embedded.clone(),
            },
            tts: TtsWiringSettings {
                active: self.tts.active,
                voice_index: self.tts.voice_index,
                speed: self.tts.speed,
                threads: self.tts.threads,
                supertonic: self.tts.supertonic.clone(),
                kokoro: self.tts.kokoro.clone(),
            },
        }
    }

    pub fn to_providers_config(&self) -> ProvidersConfigFile {
        ProvidersConfigFile {
            llm: LlmProvidersConfig {
                server: self.llm.server.clone(),
                cloud: self.llm.cloud.clone(),
                cloud_keys: self.llm.cloud_keys.clone(),
            },
            stt: SttProvidersConfig {
                cloud: self.stt.cloud.clone(),
            },
            tts: TtsProvidersConfig {
                edge_tts: self.tts.edge_tts.clone(),
                chatterbox: self.tts.chatterbox.clone(),
                chatterbox_remote: self.tts.chatterbox_remote.clone(),
                zipvoice: self.tts.zipvoice.clone(),
            },
            realtime: self.realtime.clone(),
        }
    }

    pub fn to_agent_config(&self) -> AgentConfigFile {
        AgentConfigFile {
            persona: self.persona.clone(),
            working_memory: self.working_memory.clone(),
            personal_memory: self.personal_memory.clone(),
            cognitive: CognitiveSettings {
                temperature: self.llm.temperature,
                compaction_temperature: self.llm.compaction_temperature,
                max_output_tokens: self.llm.max_output_tokens,
                context_window: self.llm.context_window,
                reasoning_enabled: self.llm.reasoning_enabled,
            },
        }
    }

    pub fn merge_settings_config(&mut self, cfg: SettingsConfigFile) {
        self.audio = cfg.audio;
        self.vad = cfg.vad;
        self.appearance = cfg.appearance;
        self.interaction = cfg.interaction;
        self.dictation = cfg.dictation;
        self.system = cfg.system;
        self.stt.active = cfg.stt.active;
        self.stt.transliterate_enabled = cfg.stt.transliterate_enabled;
        self.stt.embedded = cfg.stt.embedded;
        self.llm.active = cfg.llm.active;
        self.llm.threads = cfg.llm.threads;
        self.llm.embedded = cfg.llm.embedded;
        self.tts.active = cfg.tts.active;
        self.tts.voice_index = cfg.tts.voice_index;
        self.tts.speed = cfg.tts.speed;
        self.tts.threads = cfg.tts.threads;
        self.tts.supertonic = cfg.tts.supertonic;
        self.tts.kokoro = cfg.tts.kokoro;
    }

    pub fn merge_providers_config(&mut self, cfg: ProvidersConfigFile) {
        self.llm.server = cfg.llm.server;
        self.llm.cloud = cfg.llm.cloud;
        self.llm.cloud_keys = cfg.llm.cloud_keys;
        self.stt.cloud = cfg.stt.cloud;
        self.tts.edge_tts = cfg.tts.edge_tts;
        self.tts.chatterbox = cfg.tts.chatterbox;
        self.tts.chatterbox_remote = cfg.tts.chatterbox_remote;
        self.tts.zipvoice = cfg.tts.zipvoice;
        self.realtime = cfg.realtime;
    }

    pub fn merge_agent_config(&mut self, cfg: AgentConfigFile) {
        self.persona = cfg.persona;
        self.working_memory = cfg.working_memory;
        self.personal_memory = cfg.personal_memory;
        self.llm.temperature = cfg.cognitive.temperature;
        self.llm.compaction_temperature = cfg.cognitive.compaction_temperature;
        self.llm.max_output_tokens = cfg.cognitive.max_output_tokens;
        self.llm.context_window = cfg.cognitive.context_window;
        self.llm.reasoning_enabled = cfg.cognitive.reasoning_enabled;
    }
}
