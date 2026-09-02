mod handshake;
mod protocol;
mod session;

use std::sync::Arc;

use anyhow::{bail, Result};
use parking_lot::Mutex;

use crate::core::settings::{DeepgramVoiceAgentConfig, InteractionMode};
use crate::services::realtime::transport::connection::spawn_harness;
use crate::services::realtime::transport::{HarnessConfig, HarnessInit};
use crate::services::realtime::{
    RealtimeAudioConfig, RealtimeProviderEvent, RealtimeProviderKind, RealtimeSession,
    RealtimeVoiceProvider, DEEPGRAM_DEFAULT_WS_URL, DEEPGRAM_HEALTH_CHECK_ADDR,
    DEFAULT_INPUT_SAMPLE_RATE, DEFAULT_OUTPUT_SAMPLE_RATE, MAX_RECONNECT_ATTEMPTS,
    RECONNECT_BASE_DELAY_SECS, RECONNECT_FACTOR_SECS, WS_HEALTH_CHECK_TIMEOUT,
};

use session::{DeepgramDriver, DeepgramSessionState, DeepgramVoiceAgentSession};

pub struct DeepgramVoiceAgentProvider {
    config: DeepgramVoiceAgentConfig,
    system_prompt: String,
    state_rx: tokio::sync::watch::Receiver<crate::core::state::InteractionState>,
    turn_id: Arc<std::sync::atomic::AtomicU32>,
}

impl DeepgramVoiceAgentProvider {
    pub fn new(
        config: DeepgramVoiceAgentConfig,
        system_prompt: String,
        state_rx: tokio::sync::watch::Receiver<crate::core::state::InteractionState>,
        turn_id: Arc<std::sync::atomic::AtomicU32>,
    ) -> Self {
        Self {
            config,
            system_prompt,
            state_rx,
            turn_id,
        }
    }
}

impl RealtimeVoiceProvider for DeepgramVoiceAgentProvider {
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::DeepgramVoiceAgent
    }

    fn audio_config(&self) -> RealtimeAudioConfig {
        RealtimeAudioConfig {
            input_sample_rate: DEFAULT_INPUT_SAMPLE_RATE,
            output_sample_rate: DEFAULT_OUTPUT_SAMPLE_RATE,
            requires_input_resampling: false,
            requires_output_resampling: false,
        }
    }

    fn connect(
        &self,
        interaction_mode: InteractionMode,
    ) -> Result<(
        Box<dyn RealtimeSession>,
        tokio::sync::mpsc::Receiver<RealtimeProviderEvent>,
    )> {
        log::debug!(
            "[DeepgramVoiceAgent] Connecting with interaction_mode: {:?}",
            interaction_mode
        );
        let handle = tokio::runtime::Handle::current();

        if self.config.api_key.is_empty() {
            bail!("No API key configured for Deepgram Voice Agent. Please check settings.");
        }

        let api_key = self.config.api_key.clone();
        let url = std::env::var("DEEPGRAM_AGENT_ENDPOINT_OVERRIDE")
            .unwrap_or_else(|_| DEEPGRAM_DEFAULT_WS_URL.to_string());

        let (ws_write, ws_read) = tokio::task::block_in_place(|| {
            handle.block_on(handshake::perform_handshake(
                &url,
                &api_key,
                &self.config,
                &self.system_prompt,
            ))
        })?;

        let (provider_event_tx, provider_event_rx) =
            tokio::sync::mpsc::channel::<RealtimeProviderEvent>(
                crate::services::realtime::BRIDGE_CHANNEL_CAPACITY,
            );

        let session_state = Arc::new(Mutex::new(DeepgramSessionState {
            last_assistant_text: String::new(),
            turn_id: self.turn_id.clone(),
            server_turn_cursor: None,
        }));

        let driver = Arc::new(DeepgramDriver {
            state: session_state,
        });

        let config_clone = self.config.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let url_clone = url.clone();
        let api_key_clone = api_key.clone();

        let reconnect_fn = Box::new(move || {
            let url = url_clone.clone();
            let api_key = api_key_clone.clone();
            let config = config_clone.clone();
            let system_prompt = system_prompt_clone.clone();
            Box::pin(async move {
                handshake::perform_handshake(&url, &api_key, &config, &system_prompt).await
            }) as futures_util::future::BoxFuture<'static, anyhow::Result<_>>
        });

        let harness = spawn_harness(
            driver,
            HarnessConfig {
                max_reconnect_attempts: MAX_RECONNECT_ATTEMPTS,
                reconnect_base_delay_secs: RECONNECT_BASE_DELAY_SECS,
                reconnect_factor_secs: RECONNECT_FACTOR_SECS,
            },
            HarnessInit {
                ws_write,
                ws_read,
                reconnect_fn,
                provider_event_tx,
                state_rx: self.state_rx.clone(),
                turn_id_ref: self.turn_id.clone(),
                tokio_handle: handle,
            },
        );

        Ok((
            Box::new(DeepgramVoiceAgentSession {
                outbound_tx: harness.outbound_tx,
                shutdown_tx: harness.shutdown_tx,
                terminated: harness.terminated,
            }),
            provider_event_rx,
        ))
    }

    fn health_check(&self) -> bool {
        use std::net::ToSocketAddrs;
        if let Ok(mut addrs) = DEEPGRAM_HEALTH_CHECK_ADDR.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                return crate::services::realtime::transport::health::tcp_health_check(
                    addr,
                    WS_HEALTH_CHECK_TIMEOUT,
                );
            }
        }
        false
    }
}
