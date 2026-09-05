mod handshake;
mod protocol;
mod session;

use std::sync::Arc;

use anyhow::{bail, Result};
use parking_lot::Mutex;

use crate::core::settings::{GeminiRealtimeConfig, InteractionMode};
use crate::services::realtime::transport::{connection::spawn_harness, HarnessConfig, HarnessInit};
use crate::services::realtime::{
    RealtimeAudioConfig, RealtimeProviderEvent, RealtimeProviderKind, RealtimeSession,
    RealtimeVoiceProvider, DEFAULT_INPUT_SAMPLE_RATE, DEFAULT_OUTPUT_SAMPLE_RATE,
    GEMINI_HEALTH_CHECK_ADDR, GEMINI_HEALTH_CHECK_FALLBACK_SOCKET_ADDR, MAX_RECONNECT_ATTEMPTS,
    RECONNECT_BASE_DELAY_SECS, RECONNECT_FACTOR_SECS, WS_HEALTH_CHECK_TIMEOUT,
};

use crate::core::state::InteractionState;
use session::{GeminiDriver, GeminiLiveSession, GeminiSessionState};
use std::sync::atomic::AtomicU32;

pub struct GeminiLiveProvider {
    config: GeminiRealtimeConfig,
    system_prompt: String,
    state_rx: tokio::sync::watch::Receiver<InteractionState>,
    turn_id: Arc<AtomicU32>,
}

impl GeminiLiveProvider {
    pub fn new(
        config: GeminiRealtimeConfig,
        system_prompt: String,
        state_rx: tokio::sync::watch::Receiver<InteractionState>,
        turn_id: Arc<AtomicU32>,
    ) -> Self {
        Self {
            config,
            system_prompt,
            state_rx,
            turn_id,
        }
    }
}

impl RealtimeVoiceProvider for GeminiLiveProvider {
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::GeminiLive
    }

    fn audio_config(&self) -> RealtimeAudioConfig {
        RealtimeAudioConfig {
            input_sample_rate: DEFAULT_INPUT_SAMPLE_RATE,
            output_sample_rate: DEFAULT_OUTPUT_SAMPLE_RATE,
            requires_input_resampling: false,
            requires_output_resampling: true,
        }
    }

    fn connect(
        &self,
        interaction_mode: InteractionMode,
    ) -> Result<(
        Box<dyn RealtimeSession>,
        tokio::sync::mpsc::Receiver<RealtimeProviderEvent>,
    )> {
        let handle = tokio::runtime::Handle::current();

        if self.config.api_key.is_empty() {
            bail!("No API key configured for Gemini Live.");
        }

        let model = if self.config.model.starts_with("models/") {
            self.config.model.clone()
        } else {
            format!("models/{}", self.config.model)
        };
        let url = handshake::build_url(&self.config.api_key);
        let is_ptt = interaction_mode == InteractionMode::PTT;
        let resume_handle = self.config.resume_handle.clone();

        let (ws_write, ws_read) = tokio::task::block_in_place(|| {
            handle.block_on(handshake::perform_handshake(
                &url,
                &model,
                &self.config,
                &self.system_prompt,
                is_ptt,
                resume_handle.as_deref(),
            ))
        })?;

        let (provider_event_tx, provider_event_rx) =
            tokio::sync::mpsc::channel::<RealtimeProviderEvent>(
                crate::services::realtime::BRIDGE_CHANNEL_CAPACITY,
            );

        let session_state = Arc::new(Mutex::new(GeminiSessionState {
            interrupt_active: false,
            resume_handle: self.config.resume_handle.clone(),
            model: model.clone(),
            turn_id: self.turn_id.clone(),
            server_turn_cursor: None,
        }));

        let driver = Arc::new(GeminiDriver {
            state: session_state,
        });

        let config_clone = self.config.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let url_clone = url.clone();
        let model_clone = model.clone();
        let driver_clone = driver.clone();

        let reconnect_fn = Box::new(move || {
            let url = url_clone.clone();
            let model = model_clone.clone();
            let config = config_clone.clone();
            let system_prompt = system_prompt_clone.clone();
            let driver = driver_clone.clone();
            Box::pin(async move {
                let resume_handle = driver.state.lock().resume_handle.clone();
                handshake::perform_handshake(
                    &url,
                    &model,
                    &config,
                    &system_prompt,
                    is_ptt,
                    resume_handle.as_deref(),
                )
                .await
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
            Box::new(GeminiLiveSession {
                outbound_tx: harness.outbound_tx,
                shutdown_tx: harness.shutdown_tx,
                terminated: harness.terminated,
            }),
            provider_event_rx,
        ))
    }

    fn health_check(&self) -> bool {
        use crate::services::realtime::transport::health::{resolve_or_fallback, tcp_health_check};
        let addr = resolve_or_fallback(
            GEMINI_HEALTH_CHECK_ADDR,
            GEMINI_HEALTH_CHECK_FALLBACK_SOCKET_ADDR,
        );
        tcp_health_check(addr, WS_HEALTH_CHECK_TIMEOUT)
    }
}
