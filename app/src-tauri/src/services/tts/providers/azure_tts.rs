use std::{
    io::{ErrorKind, Read},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use azure_core::credentials::TokenCredential;
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};

use super::{speed_range, SynthesisContext, TtsProvider};
use crate::{
    core::{error::TtsError, events::AudioIntent},
    services::{
        audio::PlaybackEngine,
        tts::{
            auth, ParamRange, ProviderCaps, TtsVoiceSource, AZURE_TTS_API_PATH,
            AZURE_TTS_OUTPUT_FORMAT, AZURE_TTS_SAMPLE_RATE, AZURE_TTS_TIMEOUT_SECS,
            AZURE_TTS_VOICES_PATH, MAX_SPEED, MIN_SPEED, TTS_CHUNK_SIZE,
        },
    },
};

/// Entra scope for Azure Cognitive Services tokens.
const AZURE_COGNITIVE_SCOPE: &str = "https://cognitiveservices.azure.com/.default";
/// Env fallback for the Speech resource key when `providers.jsonc` has none.
const AZURE_SPEECH_KEY_ENV: &str = "AZURE_SPEECH_KEY";
/// Cached Entra token lifetime. Entra tokens live ~10 min; refresh early.
const ENTRA_TOKEN_TTL_SECS: u64 = 480;
/// Upper bound for a single 429 backoff sleep.
const MAX_RETRY_AFTER_SECS: u64 = 10;

/// Resolved credential path. Decided once at construction, never chained at call time.
enum AuthMode {
    Key(String),
    Entra { resource_id: String },
}

/// Cloud Azure AI Speech provider: one REST SSML request per clause, raw 24 kHz PCM.
pub struct AzureTtsProvider {
    client: reqwest::blocking::Client,
    base_url: String,
    voices_url: String,
    voice: String,
    auth: AuthMode,
    entra_token: parking_lot::Mutex<Option<auth::CachedToken>>,
    speed: AtomicU32,
}

impl AzureTtsProvider {
    pub const SPEED_RANGE: ParamRange = speed_range(MIN_SPEED, MAX_SPEED);

    /// Creates the provider and resolves credentials deterministically.
    pub fn new(
        region_or_endpoint: &str,
        voice: &str,
        key: &str,
        resource_id: Option<&str>,
    ) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Some(Duration::from_secs(AZURE_TTS_TIMEOUT_SECS)))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| anyhow!("Failed to build reqwest client: {}", e))?;
        let base = auth::normalize_azure_endpoint(region_or_endpoint)
            .trim_end_matches('/')
            .to_string();
        let voices_url = format!("{base}{AZURE_TTS_VOICES_PATH}");
        let auth = if !key.trim().is_empty() {
            AuthMode::Key(key.trim().to_string())
        } else if let Ok(env_key) = std::env::var(AZURE_SPEECH_KEY_ENV) {
            if env_key.trim().is_empty() {
                return Err(auth_error());
            }
            AuthMode::Key(env_key.trim().to_string())
        } else if let Some(rid) = resource_id.map(str::trim).filter(|s| !s.is_empty()) {
            AuthMode::Entra {
                resource_id: rid.to_string(),
            }
        } else {
            return Err(auth_error());
        };
        if voice.contains(['\'', '"', '<', '>']) {
            return Err(anyhow!(
                "Azure TTS voice name contains SSML-unsafe characters"
            ));
        }
        Ok(Self {
            client,
            base_url: base,
            voices_url,
            voice: voice.to_string(),
            auth,
            entra_token: parking_lot::Mutex::new(None),
            speed: AtomicU32::new(1.0f32.to_bits()),
        })
    }

    /// Safe one-line summary for logs. Never emits secret content.
    pub fn describe(&self) -> String {
        let mode = match self.auth {
            AuthMode::Key(_) => "key",
            AuthMode::Entra { .. } => "entra",
        };
        format!(
            "AzureTts(base={}, voice={}, auth={})",
            self.base_url, self.voice, mode
        )
    }
}

/// Actionable credential error. Points at setup, never echoes secrets.
fn auth_error() -> anyhow::Error {
    anyhow!(
        "Azure TTS needs a Speech resource key in providers.jsonc tts.azure_tts.key \
         (or {AZURE_SPEECH_KEY_ENV} env), or Entra via tts.azure_tts.resource_id plus \
         `az login` with the Cognitive Services Speech User role"
    )
}

/// Builds the SSML document for one clause request. Speed rides server-side
/// in `prosody rate` (Edge pattern), keeping chunk boundaries click-free.
fn build_ssml(voice: &str, text: &str, speed: f32) -> String {
    let lang = voice
        .split('-')
        .take(2)
        .collect::<Vec<_>>()
        .join("-")
        .trim()
        .to_string();
    let lang = if lang.is_empty() {
        "en-US"
    } else {
        lang.as_str()
    };
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let rate_pct = format!("{:+}%", ((speed - 1.0) * 100.0) as i32);
    format!(
        "<speak version='1.0' xml:lang='{lang}'><voice name='{voice}'><prosody rate='{rate_pct}'>{escaped}</prosody></voice></speak>"
    )
}

static AZURE_TTS_RUNTIME: LazyLock<TokioRuntime> = LazyLock::new(|| {
    TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("vox-azure-tts-auth")
        .build()
        .expect("Failed to build Azure TTS auth Tokio runtime")
});

/// Acquires a cached Entra token, refreshing past its TTL.
fn entra_token(provider: &AzureTtsProvider) -> Result<String> {
    if let Some(cached) = provider.entra_token.lock().as_ref() {
        if let Some(token) = cached.get() {
            return Ok(token.to_string());
        }
    }
    if !matches!(provider.auth, AuthMode::Entra { .. }) {
        return Err(anyhow!(
            "Entra token requested on key-authenticated provider"
        ));
    }
    let token = AZURE_TTS_RUNTIME.block_on(async {
        let credential = azure_identity::DeveloperToolsCredential::new(None)
            .map_err(|e| anyhow!("Entra credential failed (run `az login` first): {}", e))?;
        let access = credential
            .get_token(&[AZURE_COGNITIVE_SCOPE], None)
            .await
            .map_err(|e| anyhow!("Entra token acquisition failed: {}", e))?;
        Ok::<String, anyhow::Error>(access.token.secret().to_string())
    })?;
    *provider.entra_token.lock() = Some(auth::CachedToken::new(
        token.clone(),
        Duration::from_secs(ENTRA_TOKEN_TTL_SECS),
    ));
    Ok(token)
}

/// Formats the auth header for the resolved auth mode. Entra uses the
/// `aad#resource#token` construction inside a Bearer header, per
/// https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech
/// ("Use Microsoft Entra authentication": include the `aad#` prefix and the
/// `#` separator between resource ID and access token).
fn auth_header(provider: &AzureTtsProvider) -> Result<(String, String)> {
    match provider.auth {
        AuthMode::Key(ref key) => Ok(("Ocp-Apim-Subscription-Key".to_string(), key.clone())),
        AuthMode::Entra { ref resource_id } => {
            let token = entra_token(provider)?;
            Ok((
                "Authorization".to_string(),
                format!("Bearer aad#{resource_id}#{token}"),
            ))
        }
    }
}

/// Reads headerless s16le PCM from the HTTP response and streams f32 chunks to playback.
fn stream_pcm_response(
    mut response: reqwest::blocking::Response,
    turn_id: u32,
    intent: AudioIntent,
    cancel: &Arc<AtomicBool>,
    playback: &Arc<PlaybackEngine>,
) -> Result<usize> {
    let mut byte_buf = Vec::new();
    let mut pending = Vec::new();
    let mut total_samples = 0usize;
    let mut buf = [0u8; 8192];
    loop {
        if cancel.load(Ordering::Relaxed) {
            log::info!("[AzureTTS] Synthesis cancelled for turn {}", turn_id);
            return Ok(total_samples);
        }
        match response.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                byte_buf.extend_from_slice(&buf[..n]);
                let count = byte_buf.len() / 2;
                if count > 0 {
                    for chunk in byte_buf[..count * 2].chunks_exact(2) {
                        let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
                        pending.push(v);
                    }
                    byte_buf.drain(..count * 2);
                }
                while pending.len() >= TTS_CHUNK_SIZE {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(total_samples);
                    }
                    let chunk: Vec<f32> = pending.drain(..TTS_CHUNK_SIZE).collect();
                    total_samples += chunk.len();
                    playback.ingest_chunk_with_intent(&chunk, intent);
                }
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow!("Error reading Azure stream: {}", e)),
        }
    }
    if !pending.is_empty() {
        total_samples += pending.len();
        playback.ingest_chunk_with_intent(&pending, intent);
    }
    Ok(total_samples)
}

/// Sleeps cancellably in small slices so mid-synth cancel stays responsive.
fn cancellable_sleep(dur: Duration, cancel: &Arc<AtomicBool>) {
    let start = Instant::now();
    while start.elapsed() < dur {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

impl TtsProvider for AzureTtsProvider {
    /// Declares this provider's capabilities.
    fn caps() -> ProviderCaps {
        ProviderCaps {
            voices: TtsVoiceSource::Cloud,
            clone: false,
            speed_range: Self::SPEED_RANGE,
        }
    }

    /// Synthesizes one clause via Azure REST SSML and streams PCM to playback.
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        let text_clean = text.trim();
        if text_clean.is_empty() {
            return Ok(());
        }
        let start = Instant::now();
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let ssml = build_ssml(&self.voice, text_clean, speed);
        let url = format!(
            "{}{AZURE_TTS_API_PATH}",
            self.base_url.trim_end_matches('/')
        );
        let mut attempted_refresh = false;
        let mut attempted_backoff = false;
        let response = loop {
            let (name, value) = auth_header(self)?;
            let resp = self
                .client
                .post(&url)
                .header("Content-Type", "application/ssml+xml")
                .header("X-Microsoft-OutputFormat", AZURE_TTS_OUTPUT_FORMAT)
                .header("User-Agent", "Vox")
                .header(name, value)
                .body(ssml.clone())
                .send()
                .map_err(|e| anyhow!("Azure TTS request to {} failed: {}", self.describe(), e))?;
            let status = resp.status();
            if status.is_success() {
                break resp;
            }
            if status.as_u16() == 401
                && matches!(self.auth, AuthMode::Entra { .. })
                && !attempted_refresh
            {
                *self.entra_token.lock() = None;
                attempted_refresh = true;
                continue;
            }
            if status.as_u16() == 429 && !attempted_backoff {
                let wait = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(2)
                    .min(MAX_RETRY_AFTER_SECS);
                log::warn!(
                    "[AzureTTS] Throttled (turn {}), backing off {}s",
                    ctx.turn_id,
                    wait
                );
                cancellable_sleep(Duration::from_secs(wait), &ctx.cancel);
                attempted_backoff = true;
                continue;
            }
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(TtsError::Auth {
                    message: format!(
                        "Azure Speech rejected credentials for {} (HTTP {})",
                        self.describe(),
                        status
                    ),
                }
                .into());
            }
            if status.as_u16() == 429 {
                return Err(TtsError::RateLimited {
                    message: format!("Azure Speech throttled turn {} (HTTP 429)", ctx.turn_id),
                }
                .into());
            }
            return Err(auth::classify_http_status(status.as_u16()).into());
        };
        let total_samples =
            stream_pcm_response(response, ctx.turn_id, ctx.intent, &ctx.cancel, ctx.playback)?;
        let audio_duration = total_samples as f32 / AZURE_TTS_SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 {
            start.elapsed().as_secs_f32() / audio_duration
        } else {
            0.0
        };
        log::info!(
            "[AzureTTS] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
            ctx.turn_id,
            audio_duration,
            rtf
        );
        if let Some(rtf_handle) = ctx.telemetry_rtf {
            rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
        }
        Ok(())
    }

    /// Hot-updates the speech playback speed multiplier.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Checks reachability of the Speech endpoint. Any non-5xx proves routing.
    fn health_check(&self) -> bool {
        self.client
            .get(&self.voices_url)
            .send()
            .map(|r| r.status().is_success() || (r.status().as_u16() < 500))
            .unwrap_or(false)
    }
}

/// Synthesizes text with the specified Azure voice directly to a WAV file for previews.
pub async fn synthesize_to_wav(
    region_or_endpoint: &str,
    voice: &str,
    key: &str,
    resource_id: Option<&str>,
    text: &str,
    speed: f32,
    output_path: &std::path::Path,
) -> Result<()> {
    let provider = AzureTtsProvider::new(region_or_endpoint, voice, key, resource_id)?;
    let text_clean = text.trim();
    if text_clean.is_empty() {
        return Ok(());
    }

    let ssml = build_ssml(&provider.voice, text_clean, speed);
    let url = format!("{}{AZURE_TTS_API_PATH}", provider.base_url.trim_end_matches('/'));

    let (name, value) = auth_header(&provider)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(AZURE_TTS_TIMEOUT_SECS))
        .build()
        .map_err(|e| anyhow!("Failed to build client for Azure preview: {}", e))?;

    let resp = client
        .post(&url)
        .header("Content-Type", "application/ssml+xml")
        .header("X-Microsoft-OutputFormat", AZURE_TTS_OUTPUT_FORMAT)
        .header("User-Agent", "Vox")
        .header(name, value)
        .body(ssml)
        .send()
        .await
        .map_err(|e| anyhow!("Azure preview request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(anyhow!("Azure preview rejected with HTTP {}", status));
    }

    let pcm_bytes = resp.bytes().await.map_err(|e| anyhow!("Failed to read Azure preview audio: {}", e))?;
    let mut samples: Vec<f32> = Vec::with_capacity(pcm_bytes.len() / 2);
    for chunk in pcm_bytes.chunks_exact(2) {
        let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
        samples.push(v);
    }

    if samples.is_empty() {
        anyhow::bail!("Azure TTS returned 0 audio samples");
    }

    crate::services::tts::voice::write_f32_wav(output_path, &samples, AZURE_TTS_SAMPLE_RATE)
        .map_err(|e| anyhow!("Failed to write preview WAV: {}", e))?;

    Ok(())
}
