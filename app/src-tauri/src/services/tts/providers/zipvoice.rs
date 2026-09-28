use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsZipvoiceModelConfig,
};

use super::{edge_tts::EdgeTtsProvider, speed_range, SynthesisContext, TtsProvider};
use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::VoxEvent,
        settings::{ParamRange, ProviderCaps, TtsVoiceSource},
    },
    services::{
        translit::is_devanagari,
        tts::{
            EDGE_TTS_HINDI_VOICE, MAX_SPEED, MAX_ZIPVOICE_GUIDANCE_SCALE, MIN_SPEED,
            MIN_ZIPVOICE_GUIDANCE_SCALE, MODEL_DIRNAME_TTS_ZIPVOICE_ESPEAK,
            MODEL_FILE_TTS_ZIPVOICE_DECODER, MODEL_FILE_TTS_ZIPVOICE_ENCODER,
            MODEL_FILE_TTS_ZIPVOICE_LEXICON, MODEL_FILE_TTS_ZIPVOICE_TOKENS,
            MODEL_FILE_TTS_ZIPVOICE_VOCODER, ZIPVOICE_SILENCE_SCALE,
        },
    },
};

const DEFAULT_ZIPVOICE_FEAT_SCALE: f32 = 0.1;
const DEFAULT_ZIPVOICE_T_SHIFT: f32 = 0.5;
const DEFAULT_ZIPVOICE_TARGET_RMS: f32 = 0.1;

/// Fixed flow-matching step count for ZipVoice. Not user-configurable.
///
/// ZipVoice-Distill is a *flow-distilled* model (arXiv 2506.13053): distillation
/// exists specifically to cut sampling steps, and the released checkpoint is
/// trained for a small fixed count. Overshooting it blurs the ODE trajectory
/// rather than improving quality — every reference implementation ships
/// `numSteps = 4`, and sherpa-onnx's own `GenerationConfig` default is 5.
pub const ZIPVOICE_STEPS: i32 = 4;

struct AtomicF32 {
    inner: AtomicU32,
}

impl AtomicF32 {
    const fn new(val: f32) -> Self {
        Self {
            inner: AtomicU32::new(val.to_bits()),
        }
    }

    fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.inner.load(order))
    }

    fn store(&self, val: f32, order: Ordering) {
        self.inner.store(val.to_bits(), order);
    }
}

/// Metadata record for a single packaged ZipVoice reference voice profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipvoiceVoiceEntry {
    pub slug: String,
    pub label: String,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub attribution: Option<String>,
}

/// In-memory reference audio and exact transcript for zero-shot ZipVoice cloning.
#[derive(Debug, Clone)]
pub struct ZipvoiceReference {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub text: String,
    pub slug: String,
}

/// Zero-shot conversational speech synthesis engine wrapping ZipVoice ONNX via Sherpa-ONNX.
pub struct ZipvoiceEngine {
    tts: Mutex<OfflineTts>,
    speed: AtomicF32,
    guidance_scale: AtomicF32,
    reference: RwLock<Option<Arc<ZipvoiceReference>>>,
}

impl TtsProvider for ZipvoiceEngine {
    /// Declares this provider's capabilities.
    fn caps() -> ProviderCaps {
        ProviderCaps {
            voices: TtsVoiceSource::Custom,
            clone: false,
            speed_range: Self::SPEED_RANGE,
        }
    }

    /// Hot-updates the playback speed factor.
    fn set_speed(&self, speed: f32) {
        self.speed.store(
            speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max),
            Ordering::Relaxed,
        );
    }

    /// Returns true confirming the engine is loaded in memory.
    fn health_check(&self) -> bool {
        true
    }

    /// Synthesizes text chunk into 24kHz audio and feeds to PlaybackEngine with cross-fade.
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        if ctx.cancel.load(Ordering::Relaxed) || text.trim().is_empty() {
            return Ok(());
        }

        if is_devanagari(text) {
            return self.handle_unsupported_script(text, ctx);
        }

        let ref_opt = self.reference.read().clone();
        let reference = match ref_opt {
            Some(r) => r,
            None => {
                log::error!(
                    "[Tts::Zipvoice] Cannot synthesize turn {}: reference voice not loaded",
                    ctx.turn_id
                );
                return Err(anyhow!("[Tts::Zipvoice] Reference voice not loaded"));
            }
        };

        self.execute_synthesis(text, &reference, ctx)
    }
}

impl ZipvoiceEngine {
    pub const SPEED_RANGE: ParamRange = speed_range(MIN_SPEED, MAX_SPEED);

    /// Initializes ZipVoice offline TTS components from the specified model directory.
    pub fn new(
        model_path: &Path,
        speed: f32,
        guidance_scale: f32,
        num_threads: u32,
        initial_reference: Option<Arc<ZipvoiceReference>>,
    ) -> Result<Self> {
        let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };

        let clamped_guidance =
            guidance_scale.clamp(MIN_ZIPVOICE_GUIDANCE_SCALE, MAX_ZIPVOICE_GUIDANCE_SCALE);
        let clamped_speed = speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max);

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                zipvoice: OfflineTtsZipvoiceModelConfig {
                    tokens: Some(mp(MODEL_FILE_TTS_ZIPVOICE_TOKENS)),
                    encoder: Some(mp(MODEL_FILE_TTS_ZIPVOICE_ENCODER)),
                    decoder: Some(mp(MODEL_FILE_TTS_ZIPVOICE_DECODER)),
                    vocoder: Some(mp(MODEL_FILE_TTS_ZIPVOICE_VOCODER)),
                    data_dir: Some(mp(MODEL_DIRNAME_TTS_ZIPVOICE_ESPEAK)),
                    lexicon: Some(mp(MODEL_FILE_TTS_ZIPVOICE_LEXICON)),
                    feat_scale: DEFAULT_ZIPVOICE_FEAT_SCALE,
                    t_shift: DEFAULT_ZIPVOICE_T_SHIFT,
                    target_rms: DEFAULT_ZIPVOICE_TARGET_RMS,
                    guidance_scale: clamped_guidance,
                },
                num_threads: num_threads as i32,
                debug: false,
                provider: Some("cpu".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("[Tts::Zipvoice] Failed to create OfflineTts instance"))?;

        log::info!(
            "[Tts::Zipvoice] Initialized ZipVoice TTS (speed={:.2}, guidance={:.2}, steps={}, threads={})",
            clamped_speed,
            clamped_guidance,
            ZIPVOICE_STEPS,
            num_threads
        );

        Ok(Self {
            tts: Mutex::new(tts),
            speed: AtomicF32::new(clamped_speed),
            guidance_scale: AtomicF32::new(clamped_guidance),
            reference: RwLock::new(initial_reference),
        })
    }

    /// Hot-swaps the active reference voice profile without restarting worker threads.
    pub fn set_reference(&self, reference: Arc<ZipvoiceReference>) {
        let slug = reference.slug.clone();
        *self.reference.write() = Some(reference);
        log::info!(
            "[Tts::Zipvoice] Active reference voice updated to '{}'",
            slug
        );
    }

    /// Hot-updates the flow-matching classifier-free guidance scale.
    pub fn set_guidance_scale(&self, scale: f32) {
        let clamped = scale.clamp(MIN_ZIPVOICE_GUIDANCE_SCALE, MAX_ZIPVOICE_GUIDANCE_SCALE);
        self.guidance_scale.store(clamped, Ordering::Relaxed);
        log::debug!("[Tts::Zipvoice] Guidance scale updated to {:.2}", clamped);
    }

    fn handle_unsupported_script(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        log::warn!(
            "[Tts::Zipvoice] Devanagari script detected in turn {}: routing to Edge TTS fallback",
            ctx.turn_id
        );
        if let Err(e) = ctx.event_tx.send(VoxEvent::Error(PipelineError {
            turn_id: ctx.turn_id,
            message: "Configured TTS does not support Hindi (Devanagari). Routing to Edge TTS."
                .to_string(),
            source: "ZipVoice".to_string(),
            impact: PipelineImpact::Degraded,
        })) {
            log::warn!("[Tts::Zipvoice] Failed to emit error event: {}", e);
        }

        let fallback_provider = EdgeTtsProvider::new(Some(EDGE_TTS_HINDI_VOICE));
        fallback_provider.synthesize_chunk(text, ctx)
    }

    fn execute_synthesis(
        &self,
        text: &str,
        reference: &ZipvoiceReference,
        ctx: &SynthesisContext<'_>,
    ) -> Result<()> {
        let start = Instant::now();
        let speed = self.speed.load(Ordering::Relaxed);

        let mut extra = std::collections::HashMap::new();
        extra.insert("min_char_in_sentence".to_string(), serde_json::json!(10));

        let gen_config = GenerationConfig {
            silence_scale: ZIPVOICE_SILENCE_SCALE,
            speed,
            sid: 0,
            reference_audio: Some(reference.samples.clone()),
            reference_sample_rate: reference.sample_rate as i32,
            reference_text: Some(reference.text.clone()),
            num_steps: ZIPVOICE_STEPS,
            extra: Some(extra),
        };

        let intent = ctx.intent;
        let cancel_cb = ctx.cancel.clone();

        let tts_guard = self.tts.lock();
        let sample_rate = tts_guard.sample_rate() as usize;

        let audio = tts_guard.generate_with_config(
            text,
            &gen_config,
            Some(move |_samples: &[f32], _progress: f32| -> bool {
                !cancel_cb.load(Ordering::Relaxed)
            }),
        );
        drop(tts_guard);

        if ctx.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let audio_ref = audio
            .ok_or_else(|| anyhow!("[Tts::Zipvoice] Generation failed and yielded zero samples"))?;
        let samples = audio_ref.samples();
        if samples.is_empty() {
            return Err(anyhow!(
                "[Tts::Zipvoice] Generated audio contained 0 samples"
            ));
        }

        check_peak_clipping(samples);
        if !ctx.cancel.load(Ordering::Relaxed) {
            ctx.playback.ingest_chunk_with_intent(samples, intent);
        }

        let total_samples = samples.len();
        let elapsed = start.elapsed().as_secs_f32();
        let audio_dur = if sample_rate > 0 {
            total_samples as f32 / sample_rate as f32
        } else {
            0.0
        };
        let rtf = if audio_dur > 0.0 {
            elapsed / audio_dur
        } else {
            0.0
        };

        log::info!(
            "[Tts::Zipvoice] Synthesis complete turn={} dur={:.2}s rtf={:.3}",
            ctx.turn_id,
            audio_dur,
            rtf
        );

        if let Some(telemetry) = ctx.telemetry_rtf {
            telemetry.store(rtf.to_bits(), Ordering::Relaxed);
        }

        Ok(())
    }
}

/// Enumerates and validates all reference voice profiles in the packaged voices directory.
pub fn load_voice_pack(voices_dir: &Path) -> Result<Vec<ZipvoiceVoiceEntry>> {
    let manifest_path = voices_dir.join("voices.json");
    if manifest_path.exists() {
        let file = File::open(&manifest_path)
            .with_context(|| format!("Failed to open voice manifest at {:?}", manifest_path))?;
        let reader = BufReader::new(file);
        let entries: Vec<ZipvoiceVoiceEntry> =
            serde_json::from_reader(reader).with_context(|| {
                format!("Failed to parse JSON voice manifest at {:?}", manifest_path)
            })?;

        let mut validated = Vec::with_capacity(entries.len());
        for entry in entries {
            let slug_dir = voices_dir.join(&entry.slug);
            if slug_dir.join("clip.wav").exists() && slug_dir.join("reference.txt").exists() {
                validated.push(entry);
            } else {
                log::warn!(
                    "[Tts::Zipvoice] Voice entry '{}' missing clip.wav or reference.txt in {:?}",
                    entry.slug,
                    slug_dir
                );
            }
        }
        if !validated.is_empty() {
            return Ok(validated);
        }
    }

    scan_voice_directories(voices_dir)
}

/// Scans the directory for valid voice subdirectories containing clip.wav and reference.txt.
fn scan_voice_directories(voices_dir: &Path) -> Result<Vec<ZipvoiceVoiceEntry>> {
    let mut entries = Vec::new();
    if !voices_dir.exists() {
        return Ok(entries);
    }

    for dir_entry in fs::read_dir(voices_dir)? {
        let entry = dir_entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(slug) = path.file_name().and_then(|n| n.to_str()) {
                if path.join("clip.wav").exists() && path.join("reference.txt").exists() {
                    let label = title_case(slug);
                    entries.push(ZipvoiceVoiceEntry {
                        slug: slug.to_string(),
                        label,
                        gender: None,
                        text: None,
                        license: None,
                        attribution: None,
                    });
                }
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

/// Loads the audio waveform and transcript for a specific voice entry into memory.
pub fn load_reference(entry: &ZipvoiceVoiceEntry, voices_dir: &Path) -> Result<ZipvoiceReference> {
    let voice_dir = voices_dir.join(&entry.slug);
    let wav_path = voice_dir.join("clip.wav");
    let txt_path = voice_dir.join("reference.txt");

    let wav_str = wav_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid WAV path {:?}", wav_path))?;
    let wave = sherpa_onnx::Wave::read(wav_str)
        .ok_or_else(|| anyhow!("Failed to read WAV via sherpa_onnx::Wave at {:?}", wav_path))?;

    let mut text = String::new();
    let mut file = File::open(&txt_path)
        .with_context(|| format!("Missing reference transcript at {:?}", txt_path))?;
    file.read_to_string(&mut text)?;
    let trimmed_text = text.trim().to_string();

    if trimmed_text.is_empty() {
        return Err(anyhow!(
            "Reference transcript is empty for voice '{}' at {:?}",
            entry.slug,
            txt_path
        ));
    }

    if wave.samples().is_empty() {
        return Err(anyhow!(
            "Audio clip contains zero samples for voice '{}' at {:?}",
            entry.slug,
            wav_path
        ));
    }

    Ok(ZipvoiceReference {
        samples: wave.samples().to_vec(),
        sample_rate: wave.sample_rate() as u32,
        text: trimmed_text,
        slug: entry.slug.clone(),
    })
}

/// Resolves an optional voice ID/slug into a loaded and shared ZipvoiceReference.
pub fn resolve_zipvoice_reference(
    voices_dir: &Path,
    voice_id: Option<&str>,
) -> Result<Arc<ZipvoiceReference>> {
    let pack = load_voice_pack(voices_dir)?;
    if pack.is_empty() {
        return Err(anyhow!(
            "No valid ZipVoice reference voice profiles found in {:?}",
            voices_dir
        ));
    }

    let target_slug =
        voice_id.map(|id| id.strip_prefix("zipvoice_voice_").unwrap_or(id).to_string());

    let selected_entry = match target_slug {
        Some(ref slug) => pack
            .iter()
            .find(|e| e.slug == *slug)
            .or_else(|| pack.first())
            .ok_or_else(|| anyhow!("Failed to select voice entry"))?,
        None => pack
            .first()
            .ok_or_else(|| anyhow!("Voice pack is unexpectedly empty"))?,
    };

    let reference = load_reference(selected_entry, voices_dir)?;
    Ok(Arc::new(reference))
}

/// Converts a lowercase identifier slug into Title Case for UI display.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Checks if any sample exceeds full scale (1.0) and logs a warning.
fn check_peak_clipping(samples: &[f32]) {
    let mut max_abs = 0.0f32;
    for &s in samples {
        let a = s.abs();
        if a > max_abs {
            max_abs = a;
        }
    }
    if max_abs > 1.0 {
        log::warn!("[Tts::Zipvoice] Audio peak clipped: {:.3} > 1.0", max_abs);
    }
}
