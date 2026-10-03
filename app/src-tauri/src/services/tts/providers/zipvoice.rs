use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering},
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

use super::{
    edge_tts::EdgeTtsProvider, speed_range, SynthesisContext, TtsProvider,
};
use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::VoxEvent,
    },
    services::{
        translit::is_devanagari,
        tts::{
            ParamRange, ProviderCaps, TtsVoiceSource, EDGE_TTS_HINDI_VOICE, MAX_SPEED, MIN_SPEED,
        },
    },
};

pub const MODEL_FILE_TTS_ZIPVOICE_ENCODER: &str = "encoder.int8.onnx";
pub const MODEL_FILE_TTS_ZIPVOICE_DECODER: &str = "decoder.int8.onnx";
pub const MODEL_FILE_TTS_ZIPVOICE_VOCODER: &str = "vocos_24khz.onnx";
pub const MODEL_FILE_TTS_ZIPVOICE_TOKENS: &str = "tokens.txt";
pub const MODEL_FILE_TTS_ZIPVOICE_LEXICON: &str = "lexicon.txt";
pub const MODEL_DIRNAME_TTS_ZIPVOICE_ESPEAK: &str = "espeak-ng-data";

/// Silence scale for ZipVoice.
pub const ZIPVOICE_SILENCE_SCALE: f32 = 1.0;
/// Internal guidance scale for flow-distilled ZipVoice (1 forward pass per step).
///
/// Raised from the upstream default of 1.0 to 3.0 by measurement, not taste. A 14-config
/// grid over 6 prompts x 3 disjoint synthesis texts (438 syntheses, medians over repeats,
/// `threads=4`, int8) put mean output speaker-similarity on a clear monotone climb:
/// 1.0 -> 0.547, 1.5 -> 0.556, 2.0 -> 0.595, 2.5 -> 0.594, 3.0 -> 0.624, 4.0 -> 0.609.
/// So 3.0 is the measured peak; 4.0 declines. Guidance stays off the RTF critical path
/// (max RTF 1.08 at 3.0 vs 1.21 at guidance 1.5), so the quality gain is nearly free.
pub const ZIPVOICE_GUIDANCE_SCALE: f32 = 3.0;

const DEFAULT_ZIPVOICE_FEAT_SCALE: f32 = 0.1;
/// Decoder timestep shift. Raised from 0.5 to 0.7 by the same grid: mean output
/// speaker-similarity 0.553 at 0.5 vs 0.599 at 0.7, with no RTF cost.
///
/// Public so `tts_bench` can default its `--zv-t-shift` flag to the engine's real
/// default. Previously the bench hardcoded 0.5 and then called `set_tuning`, so a
/// "default" bench run silently measured a config production never used.
pub const DEFAULT_ZIPVOICE_T_SHIFT: f32 = 0.7;
const DEFAULT_ZIPVOICE_TARGET_RMS: f32 = 0.1;

/// Fixed flow-matching step count for ZipVoice. Not user-configurable.
///
/// ZipVoice-Distill is a *flow-distilled* model (arXiv 2506.13053): distillation
/// exists specifically to cut sampling steps, and the released checkpoint is
/// trained for a small fixed count. Overshooting it blurs the ODE trajectory
/// rather than improving quality — every reference implementation ships
/// `numSteps = 4`, and sherpa-onnx's own `GenerationConfig` default is 5.
pub const ZIPVOICE_STEPS: i32 = 4;

/// Default minimum sentence-chunk length merged by sherpa's chunker.
const DEFAULT_ZIPVOICE_MIN_CHAR: i32 = 10;

#[derive(Debug, Clone)]
pub struct ZipvoiceTuning {
    pub steps: i32,
    pub guidance_scale: f32,
    pub feat_scale: f32,
    pub t_shift: f32,
    pub target_rms: f32,
    pub min_char_in_sentence: i32,
}

impl Default for ZipvoiceTuning {
    fn default() -> Self {
        Self {
            steps: ZIPVOICE_STEPS,
            // Use the shared const rather than a second hardcoded 1.0, which previously
            // let this default drift away from the engine's real default.
            guidance_scale: ZIPVOICE_GUIDANCE_SCALE,
            feat_scale: DEFAULT_ZIPVOICE_FEAT_SCALE,
            t_shift: DEFAULT_ZIPVOICE_T_SHIFT,
            target_rms: DEFAULT_ZIPVOICE_TARGET_RMS,
            min_char_in_sentence: DEFAULT_ZIPVOICE_MIN_CHAR,
        }
    }
}

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
    steps: AtomicI32,
    feat_scale: AtomicF32,
    t_shift: AtomicF32,
    target_rms: AtomicF32,
    min_char_in_sentence: AtomicI32,
    reference: RwLock<Option<Arc<ZipvoiceReference>>>,
    /// Root of the packaged voice profiles. Retained so `set_voice` can resolve
    /// an index into a reference clip at runtime; without it, voice selection
    /// would be unreachable without a full engine rebuild.
    voices_dir: PathBuf,
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

    /// Resolves a voice index into a packaged reference profile and hot-swaps
    /// it in.
    ///
    /// ZipVoice is zero-shot: identity comes from the reference clip, so there
    /// is no in-engine voice table to index. The index is therefore a position
    /// into the voice pack, matching the order `load_voice_pack` returns.
    ///
    /// Previously unimplemented, so `TtsCommand::SetVoice` silently did nothing
    /// for ZipVoice while `dispatch_worker_command` still logged a dispatch.
    /// Loading the clip is blocking I/O plus a decode, so a failure leaves the
    /// previous voice in place rather than dropping to silence.
    fn set_voice(&self, voice: i32) {
        if voice < 0 {
            log::warn!("[Tts::Zipvoice] Ignoring negative voice index {}", voice);
            return;
        }
        let idx = voice as usize;

        let pack = match load_voice_pack(&self.voices_dir) {
            Ok(p) => p,
            Err(e) => {
                log::error!(
                    "[Tts::Zipvoice] Cannot load voice pack from {:?}: {}",
                    self.voices_dir,
                    e
                );
                return;
            }
        };
        let Some(entry) = pack.get(idx) else {
            log::error!(
                "[Tts::Zipvoice] Voice index {} out of range ({} packaged voices available)",
                idx,
                pack.len()
            );
            return;
        };

        match resolve_zipvoice_reference(&self.voices_dir, Some(entry.slug.as_str())) {
            Ok(reference) => self.set_reference(reference),
            Err(e) => log::error!(
                "[Tts::Zipvoice] Failed to load reference for voice '{}': {}",
                entry.slug,
                e
            ),
        }
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

        let clamped_guidance = if guidance_scale > 0.0 {
            guidance_scale
        } else {
            ZIPVOICE_GUIDANCE_SCALE
        };
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
            steps: AtomicI32::new(ZIPVOICE_STEPS),
            feat_scale: AtomicF32::new(DEFAULT_ZIPVOICE_FEAT_SCALE),
            t_shift: AtomicF32::new(DEFAULT_ZIPVOICE_T_SHIFT),
            target_rms: AtomicF32::new(DEFAULT_ZIPVOICE_TARGET_RMS),
            min_char_in_sentence: AtomicI32::new(DEFAULT_ZIPVOICE_MIN_CHAR),
            reference: RwLock::new(initial_reference),
            voices_dir: model_path.join("voices"),
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

    fn set_guidance_scale(&self, scale: f32) {
        let val = if scale > 0.0 {
            scale
        } else {
            ZIPVOICE_GUIDANCE_SCALE
        };
        self.guidance_scale.store(val, Ordering::Relaxed);
        log::debug!("[Tts::Zipvoice] Guidance scale updated to {:.2}", val);
    }

    /// Hot-swaps the full inference tuning set without restarting the engine.
    pub fn set_tuning(&self, tuning: &ZipvoiceTuning) {
        self.steps
            .store(tuning.steps.clamp(1, 16), Ordering::Relaxed);
        self.set_guidance_scale(tuning.guidance_scale);
        if tuning.feat_scale > 0.0 {
            self.feat_scale.store(tuning.feat_scale, Ordering::Relaxed);
        }
        if tuning.t_shift >= 0.0 {
            self.t_shift.store(tuning.t_shift, Ordering::Relaxed);
        }
        if tuning.target_rms > 0.0 {
            self.target_rms.store(tuning.target_rms, Ordering::Relaxed);
        }
        if tuning.min_char_in_sentence > 0 {
            self.min_char_in_sentence
                .store(tuning.min_char_in_sentence, Ordering::Relaxed);
        }
        log::info!(
            "[Tts::Zipvoice] Tuning updated: steps={} guidance={:.2} feat={:.3} tshift={:.2} rms={:.3} minchar={}",
            self.steps.load(Ordering::Relaxed),
            self.guidance_scale.load(Ordering::Relaxed),
            self.feat_scale.load(Ordering::Relaxed),
            self.t_shift.load(Ordering::Relaxed),
            self.target_rms.load(Ordering::Relaxed),
            self.min_char_in_sentence.load(Ordering::Relaxed),
        );
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
        extra.insert(
            "min_char_in_sentence".to_string(),
            serde_json::json!(self.min_char_in_sentence.load(Ordering::Relaxed)),
        );
        extra.insert(
            "feat_scale".to_string(),
            serde_json::json!(self.feat_scale.load(Ordering::Relaxed)),
        );
        extra.insert(
            "t_shift".to_string(),
            serde_json::json!(self.t_shift.load(Ordering::Relaxed)),
        );
        extra.insert(
            "target_rms".to_string(),
            serde_json::json!(self.target_rms.load(Ordering::Relaxed)),
        );
        extra.insert(
            "guidance_scale".to_string(),
            serde_json::json!(self.guidance_scale.load(Ordering::Relaxed)),
        );

        let gen_config = GenerationConfig {
            silence_scale: ZIPVOICE_SILENCE_SCALE,
            speed,
            sid: 0,
            reference_audio: Some(reference.samples.clone()),
            reference_sample_rate: reference.sample_rate as i32,
            reference_text: Some(reference.text.clone()),
            num_steps: self.steps.load(Ordering::Relaxed),
            extra: Some(extra),
        };

        let intent = ctx.intent;
        let cancel_cb = ctx.cancel.clone();
        let playback_cb = Arc::clone(ctx.playback);
        let streamed_samples_count = Arc::new(AtomicUsize::new(0));
        let streamed_count_cb = Arc::clone(&streamed_samples_count);

        let tts_guard = self.tts.lock();
        let sample_rate = tts_guard.sample_rate() as usize;

        let clean_text = normalize_zipvoice_text(text);
        if clean_text.trim().is_empty() {
            return Ok(());
        }

        log::info!(
            "[Tts::Zipvoice] Synthesizing turn {} clause ({} chars) voice='{}' ({} samples, {} chars ref)",
            ctx.turn_id,
            clean_text.len(),
            reference.slug,
            reference.samples.len(),
            reference.text.len(),
        );

        let audio = tts_guard.generate_with_config(
            &clean_text,
            &gen_config,
            Some(move |raw_samples: &[f32], _progress: f32| -> bool {
                if cancel_cb.load(Ordering::Relaxed) {
                    return false;
                }
                if raw_samples.is_empty() {
                    return true;
                }
                streamed_count_cb.fetch_add(raw_samples.len(), Ordering::Relaxed);
                if !cancel_cb.load(Ordering::Relaxed) {
                    playback_cb.ingest_chunk_with_intent(raw_samples, intent);
                }
                true
            }),
        );
        drop(tts_guard);

        if ctx.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let streamed_total = streamed_samples_count.load(Ordering::Relaxed);
        let mut total_samples = streamed_total;
        if streamed_total == 0 {
            let audio_ref = audio.ok_or_else(|| {
                anyhow!("[Tts::Zipvoice] Generation failed and yielded zero samples")
            })?;
            let samples = audio_ref.samples();
            if samples.is_empty() {
                return Err(anyhow!(
                    "[Tts::Zipvoice] Generated audio contained 0 samples"
                ));
            }
            check_peak_clipping(samples);
            if !samples.is_empty() && !ctx.cancel.load(Ordering::Relaxed) {
                ctx.playback.ingest_chunk_with_intent(samples, intent);
            }
            total_samples = samples.len();
        }
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

/// Resolves an optional pack slug into a loaded and shared ZipvoiceReference.
/// Slugs travel verbatim from settings and the voices table; no id decoding.
pub fn resolve_zipvoice_reference(
    voices_dir: &Path,
    voice_slug: Option<&str>,
) -> Result<Arc<ZipvoiceReference>> {
    let pack = load_voice_pack(voices_dir)?;
    if pack.is_empty() {
        return Err(anyhow!(
            "No valid ZipVoice reference voice profiles found in {:?}",
            voices_dir
        ));
    }

    let selected_entry = match voice_slug {
        Some(slug) => pack.iter().find(|e| e.slug == slug).ok_or_else(|| {
            anyhow!(
                "Unknown ZipVoice voice slug '{}' ({} packaged voices available)",
                slug,
                pack.len()
            )
        })?,
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

/// Normalizes Unicode typographic characters to ASCII equivalents for sherpa-onnx lexicon.
fn normalize_zipvoice_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '’' | '‘' | '`' | '´' => out.push('\''),
            '“' | '”' | '«' | '»' => out.push('"'),
            '—' | '–' | '―' => out.push_str(" - "),
            '…' => out.push_str("..."),
            _ => out.push(ch),
        }
    }
    out
}
