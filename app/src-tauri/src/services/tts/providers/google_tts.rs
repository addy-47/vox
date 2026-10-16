use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, LazyLock,
};

use anyhow::{anyhow, Result};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};

use super::{speed_range, SynthesisContext, TtsProvider};
use crate::{
    core::{error::TtsError, events::AudioIntent},
    services::{
        audio::PlaybackEngine,
        tts::{
            ParamRange, ProviderCaps, TtsVoiceSource, GOOGLE_TTS_MAX_TEXT_BYTES,
            GOOGLE_TTS_SAMPLE_RATE, MAX_SPEED, MIN_SPEED, TTS_CHUNK_SIZE,
        },
    },
};

/// Classifies a gax RPC error into a typed, actionable TTS error.
fn classify_rpc_error(e: &google_cloud_gax::error::Error, context: &str) -> anyhow::Error {
    use google_cloud_gax::error::rpc::Code;
    let code = e.status().map(|s| s.code);
    let message = e
        .status()
        .map(|s| s.message.clone())
        .unwrap_or_else(|| e.to_string());
    let detail = format!("Google TTS {context}: {message}");
    match code {
        Some(Code::Unauthenticated) | Some(Code::PermissionDenied) => TtsError::Auth {
            message: format!("{detail} (check ADC, quota project, API enablement, and billing)"),
        }
        .into(),
        Some(Code::ResourceExhausted) => TtsError::RateLimited {
            message: format!("{detail} (200 Chirp-3 req/min, 100 concurrent streams)"),
        }
        .into(),
        Some(Code::NotFound) => TtsError::VoiceNotFound { voice: message }.into(),
        Some(Code::Cancelled) => TtsError::Cancelled.into(),
        _ => TtsError::SynthesisFailed { message: detail }.into(),
    }
}

/// Cloud Google Text-to-Speech provider: one bidi stream per clause (config →
/// text → half-close → PCM chunks), matching the worker's clause granularity.
pub struct GoogleTtsProvider {
    voice: String,
    project_override: Option<String>,
    speed: AtomicU32,
}

impl GoogleTtsProvider {
    pub const SPEED_RANGE: ParamRange = speed_range(MIN_SPEED, MAX_SPEED);

    /// Creates the provider. Network I/O stays on the caller (TTS worker thread).
    pub fn new(voice: &str, project_override: Option<&str>) -> Result<Self> {
        Ok(Self {
            voice: voice.to_string(),
            project_override: project_override
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            speed: AtomicU32::new(1.0f32.to_bits()),
        })
    }
}

static GOOGLE_TTS_RUNTIME: LazyLock<TokioRuntime> = LazyLock::new(|| {
    TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("vox-google-tts")
        .build()
        .expect("Failed to build Google TTS Tokio runtime")
});

/// Builds the client with ADC, pointing at setup docs on failure.
async fn connect_client() -> Result<google_cloud_texttospeech_v1::client::TextToSpeech> {
    google_cloud_texttospeech_v1::client::TextToSpeech::builder()
        .build()
        .await
        .map_err(|e| {
            anyhow!(
                "Google TTS ADC failed (run `gcloud auth application-default login` \
                 with billing + Text-to-Speech API enabled): {}",
                e
            )
        })
}

/// Derives the BCP-47 locale from a voice name (`en-US-Chirp3-HD-Kore` → `en-US`).
fn voice_locale(voice: &str) -> String {
    let locale = voice
        .split('-')
        .take(2)
        .collect::<Vec<_>>()
        .join("-")
        .trim()
        .to_string();
    if locale.is_empty() {
        "en-US".to_string()
    } else {
        locale
    }
}

/// Chunked PCM ingestion state for one streaming response.
struct PcmIngest<'a> {
    stash: Vec<u8>,
    pending: Vec<f32>,
    total_samples: usize,
    turn_id: u32,
    intent: AudioIntent,
    cancel: Arc<AtomicBool>,
    playback: &'a Arc<PlaybackEngine>,
}

impl<'a> PcmIngest<'a> {
    /// Creates ingestion state for one synthesis turn.
    fn new(
        turn_id: u32,
        intent: AudioIntent,
        cancel: Arc<AtomicBool>,
        playback: &'a Arc<PlaybackEngine>,
    ) -> Self {
        Self {
            stash: Vec::new(),
            pending: Vec::new(),
            total_samples: 0,
            turn_id,
            intent,
            cancel,
            playback,
        }
    }

    /// Feeds one PCM s16le payload into playback in worker-sized chunks.
    fn ingest(&mut self, bytes: &[u8]) -> bool {
        self.stash.extend_from_slice(bytes);
        let count = self.stash.len() / 2;
        if count > 0 {
            for chunk in self.stash[..count * 2].chunks_exact(2) {
                let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
                self.pending.push(v);
            }
            self.stash.drain(..count * 2);
        }
        while self.pending.len() >= TTS_CHUNK_SIZE {
            if self.cancel.load(Ordering::Relaxed) {
                log::info!("[GoogleTTS] Synthesis cancelled for turn {}", self.turn_id);
                return false;
            }
            let chunk: Vec<f32> = self.pending.drain(..TTS_CHUNK_SIZE).collect();
            self.total_samples += chunk.len();
            self.playback.ingest_chunk_with_intent(&chunk, self.intent);
        }
        true
    }

    /// Flushes a trailing partial chunk to playback unless cancelled.
    fn flush(&mut self) {
        if self.pending.is_empty() || self.cancel.load(Ordering::Relaxed) {
            return;
        }
        self.total_samples += self.pending.len();
        let chunk = std::mem::take(&mut self.pending);
        self.playback.ingest_chunk_with_intent(&chunk, self.intent);
    }
}

impl TtsProvider for GoogleTtsProvider {
    /// Declares this provider's capabilities.
    fn caps() -> ProviderCaps {
        ProviderCaps {
            voices: TtsVoiceSource::Cloud,
            clone: false,
            speed_range: Self::SPEED_RANGE,
        }
    }

    /// Synthesizes one clause over a fresh bidi stream and feeds PCM to playback.
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        use google_cloud_texttospeech_v1::model;
        let text_clean = text.trim();
        if text_clean.is_empty() {
            return Ok(());
        }
        if text_clean.len() > GOOGLE_TTS_MAX_TEXT_BYTES {
            return Err(TtsError::SynthesisFailed {
                message: format!(
                    "Clause is {} bytes, over the {} byte Google limit; \
                     split the text before synthesis",
                    text_clean.len(),
                    GOOGLE_TTS_MAX_TEXT_BYTES
                ),
            }
            .into());
        }
        let start = std::time::Instant::now();
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let voice_name = self.voice.clone();
        let locale = voice_locale(&voice_name);
        let project = self.project_override.clone();
        let turn_id = ctx.turn_id;
        let intent = ctx.intent;
        let cancel = ctx.cancel.clone();
        let playback = Arc::clone(ctx.playback);
        let telemetry_rtf = ctx.telemetry_rtf.cloned();
        GOOGLE_TTS_RUNTIME.block_on(async move {
            let client = connect_client().await?;
            let mut builder = client.streaming_synthesize();
            if let Some(project) = project {
                let mut options = google_cloud_gax::options::RequestOptions::default();
                options.set_quota_project(project);
                builder = builder.with_options(options);
            }
            let (sender, mut responses) = builder.build();
            let audio_config = model::StreamingAudioConfig::new()
                .set_audio_encoding(model::AudioEncoding::Pcm)
                .set_sample_rate_hertz(GOOGLE_TTS_SAMPLE_RATE as i32)
                .set_speaking_rate(speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max) as f64);
            let voice_params = model::VoiceSelectionParams::new()
                .set_language_code(locale)
                .set_name(voice_name);
            let config = model::StreamingSynthesizeConfig::new()
                .set_voice(voice_params)
                .set_streaming_audio_config(audio_config);
            let config_req = model::StreamingSynthesizeRequest::new().set_streaming_request(
                model::streaming_synthesize_request::StreamingRequest::StreamingConfig(
                    config.into(),
                ),
            );
            sender
                .send(config_req)
                .await
                .map_err(|e| anyhow!("Google TTS config send failed: {}", e))?;
            let input = model::StreamingSynthesisInput::new().set_input_source(
                model::streaming_synthesis_input::InputSource::Text(text_clean.to_string()),
            );
            let input_req = model::StreamingSynthesizeRequest::new().set_streaming_request(
                model::streaming_synthesize_request::StreamingRequest::Input(input.into()),
            );
            sender
                .send(input_req)
                .await
                .map_err(|e| anyhow!("Google TTS input send failed: {}", e))?;
            drop(sender);
            let mut ingest = PcmIngest::new(turn_id, intent, cancel.clone(), &playback);
            while let Some(response) = responses.next().await {
                if cancel.load(Ordering::Relaxed) {
                    log::info!("[GoogleTTS] Synthesis cancelled for turn {}", turn_id);
                    break;
                }
                let response =
                    response.map_err(|e| classify_rpc_error(&e, "stream failed"))?;
                if !ingest.ingest(&response.audio_content) {
                    break;
                }
            }
            ingest.flush();
            let total_samples = ingest.total_samples;
            let audio_duration = total_samples as f32 / GOOGLE_TTS_SAMPLE_RATE as f32;
            let rtf = if audio_duration > 0.0 {
                start.elapsed().as_secs_f32() / audio_duration
            } else {
                0.0
            };
            log::info!(
                "[GoogleTTS] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
                turn_id,
                audio_duration,
                rtf
            );
            if let Some(rtf_handle) = telemetry_rtf {
                rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
            }
            Ok(())
        })
    }

    /// Hot-updates the speech playback speed multiplier.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Checks ADC + API reachability with a lightweight voices list call.
    fn health_check(&self) -> bool {
        GOOGLE_TTS_RUNTIME.block_on(async {
            let client = match connect_client().await {
                Ok(client) => client,
                Err(_) => return false,
            };
            client
                .list_voices()
                .set_language_code("en-US")
                .send()
                .await
                .is_ok()
        })
    }
}
