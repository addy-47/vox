use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use crate::core::events::{Actionability, PipelineError, PipelineImpact, VoxEvent};
use crate::core::settings::{TtsProviderConfig, VoxSettings};
use crate::services::audio::PlaybackEngine;
use crate::services::tts::providers::TtsProvider;
use crate::services::tts::{
    ChatterboxEngine, ChatterboxRemoteProvider, EdgeTtsProvider, KokoroEngine,
    TtsEngine as SupertonicEngine,
};

/// Commands accepted by the dedicated TTS synthesis worker thread.
#[derive(Debug)]
pub enum TtsCommand {
    Generate { turn_id: u32, text: String },
    SetVoice(i32),
    Shutdown,
}

/// Execution handles and atomics passed to the dedicated TTS worker thread.
pub struct TtsWorkerHandles {
    pub playback: Arc<PlaybackEngine>,
    pub event_tx: mpsc::Sender<VoxEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub pending_synthesis_jobs: Option<Arc<AtomicU32>>,
    pub telemetry_rtf: Option<Arc<AtomicU32>>,
}

/// Spawns the dedicated TTS worker thread and processes incoming synthesis requests.
pub fn spawn_tts_worker(
    rx: mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    handles: TtsWorkerHandles,
) {
    log::info!("[TTS Worker] Persistent loop started.");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate { turn_id, text } => {
                log::debug!("[TTS Worker] Processing TTS chunk: '{}'", text);
                let text_clone = text.clone();
                let cancel_flag = handles.cancel_flag.clone();
                let playback = handles.playback.clone();
                let event_tx = handles.event_tx.clone();
                let telemetry_rtf = handles.telemetry_rtf.clone();

                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    provider.synthesize_chunk(
                        &text_clone,
                        turn_id,
                        cancel_flag,
                        &playback,
                        event_tx,
                        telemetry_rtf.as_ref(),
                    )
                }));

                if let Some(ref jobs) = handles.pending_synthesis_jobs {
                    let remaining = jobs.fetch_sub(1, Ordering::Relaxed);
                    if remaining <= 1 {
                        handles.playback.flush_pre_roll();
                    }
                }

                match res {
                    Ok(Err(e)) => {
                        log::warn!(
                            "[TTS Worker] Synthesis chunk failed for turn {}: {}",
                            turn_id,
                            e
                        );
                    }
                    Err(panic_payload) => {
                        let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "TTS synthesize_chunk panicked".to_string()
                        };
                        log::error!(
                            "[TTS Worker] Panic caught during synthesis of turn {}: {}",
                            turn_id,
                            panic_msg
                        );
                        let _ = handles.event_tx.send(VoxEvent::Error(PipelineError {
                            turn_id,
                            message: format!("TTS synthesis panic: {}", panic_msg),
                            source: "TtsActor".to_string(),
                            impact: PipelineImpact::Degraded,
                            actionability: Actionability::None,
                        }));
                    }
                    Ok(Ok(())) => {}
                }
            }
            TtsCommand::SetVoice(voice) => {
                log::info!("[TTS Worker] Setting active speaker voice to: {}", voice);
                provider.set_voice(voice);
            }
            TtsCommand::Shutdown => {
                log::info!("[TTS Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    log::info!("[TTS Worker] Loop exited. Provider will be dropped.");
}

/// Resolves a voice UUID to a WAV file path for Chatterbox voice conditioning.
pub async fn resolve_reference_audio(voice_id: Option<&str>) -> Option<String> {
    let id = voice_id?;
    let db_path = crate::utils::paths::db_path();

    let conn = crate::persistence::db::VoxDb::open_readonly(&db_path)
        .await
        .ok()?;

    let entry = crate::persistence::voices::get_voice(&conn, id)
        .await
        .ok()??;

    if let Some(ref dir) = entry.voice_dir {
        let path = std::path::Path::new(dir);
        if path.exists() && path.join("speaker_emb.npy").exists() {
            return Some(dir.clone());
        }
    }

    let wav = entry.wav_path?;
    if !std::path::Path::new(&wav).exists() {
        log::warn!(
            "[TTS Actor] Voice {} wav_path not found on disk: {}. Using built-in voice.",
            id,
            wav
        );
        return None;
    }
    Some(wav)
}

/// Creates a boxed TTS provider based on settings configuration.
pub fn create_tts_provider(
    settings: &VoxSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
) -> Result<Box<dyn TtsProvider>, String> {
    let provider_config = settings.tts.to_provider_config();
    let voice = settings.tts.voice_index;
    let quality_steps = settings.tts.quality_steps;
    let speed = settings.tts.speed;
    let num_threads = settings.tts.threads;

    match &provider_config {
        TtsProviderConfig::Supertonic => {
            log::info!("[TTS Actor] Initializing Supertonic engine");
            SupertonicEngine::new(super_tts_path, voice, quality_steps, speed, num_threads)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))
        }
        TtsProviderConfig::Kokoro => {
            log::info!("[TTS Actor] Initializing Kokoro Multi-Lang engine");
            let kokoro_path = crate::utils::paths::model_dir(super::KOKORO_MODEL_DIR);
            KokoroEngine::new(&kokoro_path, voice, speed, num_threads)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Kokoro engine: {}", e))
        }
        TtsProviderConfig::Chatterbox {
            language,
            quality_steps: cb_quality,
            speed: cb_speed,
            voice_id: _,
        } => {
            log::info!("[TTS Actor] Initializing Chatterbox engine");
            let chatterbox_path = crate::utils::paths::model_dir(super::CHATTERBOX_MODEL_DIR);
            ChatterboxEngine::new(
                &chatterbox_path,
                language,
                *cb_quality,
                *cb_speed,
                reference_audio,
            )
            .map(|e| Box::new(e) as Box<dyn TtsProvider>)
            .map_err(|e| format!("Failed to create Chatterbox engine: {}", e))
        }
        TtsProviderConfig::ChatterboxRemote {
            endpoint,
            language,
            quality_steps: remote_quality,
            speed: remote_speed,
            remote_path,
            voice_id: _,
        } => {
            log::info!("[TTS Actor] Initializing ChatterboxRemote provider");
            ChatterboxRemoteProvider::new(
                endpoint,
                language,
                *remote_quality,
                *remote_speed,
                remote_path,
            )
            .map(|p| Box::new(p) as Box<dyn TtsProvider>)
            .map_err(|e| format!("Failed to create ChatterboxRemote provider: {}", e))
        }
        TtsProviderConfig::EdgeTts { voice: edge_voice } => {
            log::info!("[TTS Actor] Initializing EdgeTTS provider");
            Ok(Box::new(EdgeTtsProvider::new(edge_voice.as_deref())))
        }
    }
}

/// Handles and flags passed when warming up the TTS actor.
pub struct TtsWarmUpHandles<'a> {
    pub tts_tx: &'a mut Option<mpsc::Sender<TtsCommand>>,
    pub tts_handle: &'a mut Option<std::thread::JoinHandle<()>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub playback_engine: Arc<PlaybackEngine>,
    pub pending_synthesis_jobs: Option<Arc<AtomicU32>>,
    pub telemetry_rtf: Option<Arc<AtomicU32>>,
}

/// Spawns and initializes a persistent TTS worker actor thread.
pub fn warm_up_tts(
    handles: TtsWarmUpHandles<'_>,
    settings: &VoxSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
    event_tx: mpsc::Sender<VoxEvent>,
) -> Result<(), String> {
    if handles.tts_tx.is_some() {
        return Ok(());
    }

    log::info!("[TTS Actor] Warming up TTS worker");
    let provider = create_tts_provider(settings, super_tts_path, reference_audio)?;

    let (tx, rx) = mpsc::channel::<TtsCommand>();
    *handles.tts_tx = Some(tx);

    let worker_handles = TtsWorkerHandles {
        playback: handles.playback_engine,
        event_tx,
        cancel_flag: handles.cancel_flag,
        pending_synthesis_jobs: handles.pending_synthesis_jobs,
        telemetry_rtf: handles.telemetry_rtf,
    };

    let handle = std::thread::Builder::new()
        .name("vox-tts-persistent".to_string())
        .spawn(move || {
            if let Err(e) =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max)
            {
                log::warn!("[TTS Actor] Thread priority elevation failed: {:?}", e);
            }
            spawn_tts_worker(rx, provider, worker_handles);
        })
        .map_err(|e| e.to_string())?;

    *handles.tts_handle = Some(handle);
    Ok(())
}

/// Signals the running TTS worker thread to shutdown and drop its model instance.
pub fn cool_down_tts(tts_tx: &mut Option<mpsc::Sender<TtsCommand>>) {
    if let Some(tx) = tts_tx.take() {
        if let Err(e) = tx.send(TtsCommand::Shutdown) {
            log::warn!("[TTS Actor] Failed to send Shutdown command: {}", e);
        }
        log::info!("[TTS Actor] Shutdown command sent (offloaded)");
    }
}

/// Accumulates streaming token fragments and splits them into speakable clause/sentence chunks.
#[derive(Debug, Default, Clone)]
pub struct TtsClauseChunker {
    buffer: String,
}

impl TtsClauseChunker {
    /// Creates an empty TtsClauseChunker instance.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Appends incoming text slice into the accumulator and returns any completed clauses.
    pub fn push_str(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        self.extract_chunks()
    }

    /// Flushes any remaining unpunctuated text in the buffer as a final speakable chunk.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Clears the internal chunker buffer unconditionally on cancellation or interruption.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Returns a slice view of the current unconsumed buffer text.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Returns true if the chunker accumulator contains no text.
    pub fn is_empty(&self) -> bool {
        self.buffer.trim().is_empty()
    }

    /// Scans buffer text and locates valid clause or sentence split byte positions.
    fn find_split_point(&self) -> Option<(usize, usize)> {
        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

        // Check for 25-word emergency boundary to prevent buffer bloat
        let words: Vec<&str> = self.buffer.split_whitespace().collect();
        if words.len() >= 25 {
            let target_word_count = 20;
            let mut count = 0;
            for (pos, c) in &chars {
                if c.is_whitespace() {
                    count += 1;
                    if count >= target_word_count {
                        return Some((*pos, c.len_utf8()));
                    }
                }
            }
        }

        for i in 0..chars.len() {
            let (pos, c) = chars[i];

            // Primary sentence boundaries: newline, question mark, exclamation mark
            if c == '\n' || c == '?' || c == '!' {
                return Some((pos, c.len_utf8()));
            }

            // Sub-clause boundaries: comma, semicolon, colon, em-dash
            if c == ',' || c == ';' || c == ':' || c == '—' || c == '–' {
                // Natural flow & prosody pacing: only split if sub-clause has >= 5 words
                let text_before = &self.buffer[..pos];
                let word_count = text_before.split_whitespace().count();
                if word_count >= 5 {
                    return Some((pos, c.len_utf8()));
                }
                continue;
            }

            // Period sentence boundary
            if c == '.' {
                let prev_is_digit = if i > 0 {
                    chars[i - 1].1.is_ascii_digit()
                } else {
                    false
                };
                let next_is_digit = if i + 1 < chars.len() {
                    chars[i + 1].1.is_ascii_digit()
                } else {
                    false
                };

                if prev_is_digit && next_is_digit {
                    continue;
                }

                let text_before = &self.buffer[..pos];
                let last_word = text_before
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|p: char| !p.is_alphanumeric());

                if is_abbreviation(last_word) {
                    continue;
                }

                return Some((pos, c.len_utf8()));
            }
        }

        None
    }

    /// Extracts all completed speakable clause strings from the buffer.
    fn extract_chunks(&mut self) -> Vec<String> {
        let mut chunks = Vec::new();

        while !self.buffer.is_empty() {
            if let Some((pos, len)) = self.find_split_point() {
                let end = pos + len;
                let chunk = self.buffer[..end].trim().to_string();
                self.buffer = self.buffer[end..].to_string();

                if !chunk.is_empty() {
                    chunks.push(chunk);
                }
            } else {
                break;
            }
        }

        chunks
    }
}

/// Identifies standard honorifics, abbreviations, and version prefixes that suppress period splits.
fn is_abbreviation(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }

    let lower = word.to_lowercase();

    const ABBREVS: &[&str] = &[
        "dr", "mr", "mrs", "ms", "prof", "sr", "jr", "st", "vs", "e.g", "i.e", "etc", "approx",
        "dept", "fig", "ver", "vol", "inc", "ltd", "co", "no", "p", "pg", "pp",
    ];

    if ABBREVS.contains(&lower.as_str()) {
        return true;
    }

    if lower.starts_with('v') && lower.len() > 1 && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    if word.len() == 1 && word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests strong terminators (? ! newline) always split regardless of word count.
    #[test]
    fn test_chunker_strong_terminators_split() {
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str("Hello world? Next clause here.");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "Hello world?");
        assert_eq!(chunks[1], "Next clause here.");
        assert!(c.is_empty());
    }

    /// Tests newline is treated as strong terminator.
    #[test]
    fn test_chunker_newline_splits() {
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str("Line one\nLine two");
        assert_eq!(chunks, vec!["Line one"]);
        assert_eq!(c.buffer(), "Line two");
    }

    /// Tests comma requires >=5 words before split for prosody pacing.
    #[test]
    fn test_chunker_comma_gated_by_word_count() {
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str("Hello, world here");
        assert!(chunks.is_empty(), "short comma must not split");

        let mut c2 = TtsClauseChunker::new();
        let chunks2 = c2.push_str("This is a longer sentence, and it continues");
        assert_eq!(chunks2, vec!["This is a longer sentence,"]);
    }

    /// Tests period does not split on decimal like 3.14
    #[test]
    fn test_chunker_period_decimal_guard() {
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str("Value is 3.14 and continues");
        assert!(chunks.is_empty(), "decimal period must not split");
        let mut c2 = TtsClauseChunker::new();
        let chunks2 = c2.push_str("Value is 3.14. Next sentence");
        assert_eq!(chunks2, vec!["Value is 3.14."]);
    }

    /// Tests period does not split after known abbreviation.
    #[test]
    fn test_chunker_period_abbreviation_guard() {
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str("Hello Dr. Smith is here");
        assert!(chunks.is_empty(), "abbreviation period must not split");
        let mut c2 = TtsClauseChunker::new();
        let chunks2 = c2.push_str("Hello Dr. Smith is here. Next one");
        assert_eq!(chunks2, vec!["Hello Dr. Smith is here."]);
    }

    /// Tests emergency 25-word cap forces split at 20 words.
    #[test]
    fn test_chunker_emergency_word_cap() {
        let long = (0..30)
            .map(|i| format!("w{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = TtsClauseChunker::new();
        let chunks = c.push_str(&long);
        assert!(!chunks.is_empty(), "bloat guard must emit chunk");
        assert_eq!(chunks[0].split_whitespace().count(), 20);
    }

    /// Tests flush returns trimmed remainder and clears buffer.
    #[test]
    fn test_chunker_flush_and_clear() {
        let mut c = TtsClauseChunker::new();
        c.push_str("Hello world");
        assert_eq!(c.flush(), Some("Hello world".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.flush(), None);
        c.push_str("  trailing  ");
        assert_eq!(c.flush(), Some("trailing".to_string()));
        c.push_str("keep");
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.buffer(), "");
    }

    /// Tests is_abbreviation covers honorifics, version and single-letter cases.
    #[test]
    fn test_is_abbreviation_variants() {
        assert!(is_abbreviation("Dr"));
        assert!(is_abbreviation("dr"));
        assert!(is_abbreviation("e.g"));
        assert!(is_abbreviation("Mrs"));
        assert!(is_abbreviation("v2"));
        assert!(is_abbreviation("v10"));
        assert!(is_abbreviation("J"));
        assert!(!is_abbreviation("Hello"));
        assert!(!is_abbreviation(""));
        assert!(!is_abbreviation("world"));
    }

    /// Tests extract_chunks returns multiple clauses when multiple split points present.
    #[test]
    fn test_chunker_multiple_clauses() {
        let mut c = TtsClauseChunker::new();
        let chunks =
            c.push_str("First sentence! Second? Third, with many words before comma, and tail.");
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0], "First sentence!");
        assert_eq!(chunks[1], "Second?");
    }
}
