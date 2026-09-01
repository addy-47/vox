use crate::core::events::VoxEvent;
use crate::core::settings::{TtsProviderConfig, VoxSettings};
use crate::services::tts::providers::TtsProvider;
use crate::services::tts::{
    ChatterboxEngine, ChatterboxRemoteProvider, EdgeTtsProvider, KokoroEngine,
    TtsEngine as SupertonicEngine,
};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Commands accepted by the dedicated TTS synthesis worker thread.
#[derive(Debug)]
pub enum TtsCommand {
    Generate { turn_id: u32, text: String },
    SetVoice(i32),
    Shutdown,
}

/// Execution handles and atomics passed to the dedicated TTS worker thread.
pub struct TtsWorkerHandles {
    pub playback: Arc<crate::services::audio::PlaybackEngine>,
    pub event_tx: std::sync::mpsc::Sender<VoxEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub pending_synthesis_jobs: Option<Arc<std::sync::atomic::AtomicU32>>,
    pub telemetry_rtf: Option<Arc<std::sync::atomic::AtomicU32>>,
}

/// Spawns the dedicated TTS worker thread and processes incoming synthesis requests.
pub fn spawn_tts_worker(
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    handles: TtsWorkerHandles,
) {
    log::info!("[TTS Worker] Persistent loop started.");

    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate { turn_id, text } => {
                log::debug!("[TTS Worker] Processing TTS chunk: '{}'", text);
                let res = provider.synthesize_chunk(
                    &text,
                    turn_id,
                    handles.cancel_flag.clone(),
                    &handles.playback,
                    handles.event_tx.clone(),
                    handles.telemetry_rtf.as_ref(),
                );

                if let Some(ref jobs) = handles.pending_synthesis_jobs {
                    jobs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                }

                if let Err(e) = res {
                    log::warn!(
                        "[TTS Worker] Synthesis chunk failed for turn {}: {}",
                        turn_id,
                        e
                    );
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

    match &provider_config {
        TtsProviderConfig::Supertonic => {
            log::info!("[TTS Actor] Initializing Supertonic engine");
            SupertonicEngine::new(super_tts_path, voice, quality_steps, speed)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))
        }
        TtsProviderConfig::Kokoro => {
            log::info!("[TTS Actor] Initializing Kokoro Multi-Lang engine");
            let kokoro_path = crate::utils::paths::model_dir(super::KOKORO_MODEL_DIR);
            KokoroEngine::new(&kokoro_path, voice, speed)
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
    pub tts_tx: &'a mut Option<std::sync::mpsc::Sender<TtsCommand>>,
    pub tts_handle: &'a mut Option<std::thread::JoinHandle<()>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub playback_engine: Arc<crate::services::audio::PlaybackEngine>,
    pub pending_synthesis_jobs: Option<Arc<std::sync::atomic::AtomicU32>>,
    pub telemetry_rtf: Option<Arc<std::sync::atomic::AtomicU32>>,
}

/// Spawns and initializes a persistent TTS worker actor thread.
pub fn warm_up_tts(
    handles: TtsWarmUpHandles<'_>,
    settings: &VoxSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
) -> Result<(), String> {
    if handles.tts_tx.is_some() {
        return Ok(());
    }

    log::info!("[TTS Actor] Warming up TTS worker");
    let provider = create_tts_provider(settings, super_tts_path, reference_audio)?;

    let (tx, rx) = std::sync::mpsc::channel::<TtsCommand>();
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
pub fn cool_down_tts(tts_tx: &mut Option<std::sync::mpsc::Sender<TtsCommand>>) {
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
