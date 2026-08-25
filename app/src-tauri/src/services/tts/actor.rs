use crate::core::constants::EVENT_MODEL_READY;
use crate::core::events::VoxEvent;
use crate::core::settings::{TtsProviderConfig, VoxSettings};
use crate::services::tts::providers::TtsProvider;
use crate::services::tts::{
    ChatterboxEngine, ChatterboxRemoteProvider, EdgeTtsProvider, TtsEngine as SupertonicEngine,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

/// Commands dispatched to the background speech synthesis worker actor.
pub enum TtsCommand {
    Generate { turn_id: u32, text: String },
    UpdateQualitySteps(u32),
    UpdateSpeed(f32),
    Shutdown,
}

/// Spawns a dedicated OS worker thread processing speech synthesis tasks in a blocking loop.
pub fn spawn_tts_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
) {
    is_loaded.store(true, Ordering::Relaxed);
    if let Err(e) = app.emit(EVENT_MODEL_READY, "TTS") {
        log::warn!("[TTS Worker] Failed to emit model ready event: {:?}", e);
    }

    log::info!(
        "[TTS Worker] Persistent loop started with provider: {:?}",
        provider.kind()
    );

    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCommand::Generate { turn_id, text } => {
                if let Err(e) =
                    provider.synthesize_chunk(&text, turn_id, cancel_flag.clone(), event_tx.clone())
                {
                    log::error!("[TTS Worker] Synthesis error (turn {}): {}", turn_id, e);
                }
            }
            TtsCommand::UpdateQualitySteps(steps) => {
                provider.set_quality_steps(steps);
                log::info!("[TTS Worker] Quality steps updated to {}", steps);
            }
            TtsCommand::UpdateSpeed(speed) => {
                provider.set_speed(speed);
                log::info!("[TTS Worker] Speed updated to {:.2}", speed);
            }
            TtsCommand::Shutdown => {
                log::info!("[TTS Worker] Shutdown command received. Exiting loop.");
                break;
            }
        }
    }

    is_loaded.store(false, Ordering::Relaxed);
    log::info!("[TTS Worker] Loop exited. Provider will be dropped.");
}

/// Resolves a voice UUID to a WAV file path for Chatterbox voice conditioning.
pub fn resolve_reference_audio(voice_id: Option<&str>) -> Option<String> {
    let id = voice_id?;
    let db_path = crate::utils::paths::db_path();
    let rt = crate::persistence::db::get_tokio_handle();

    let conn = rt.block_on(async {
        crate::persistence::db::VoxDb::open_readonly(&db_path)
            .await
            .ok()
    })?;

    let entry =
        rt.block_on(async { crate::persistence::voices::get_voice(&conn, id).await.ok() })??;

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
        TtsProviderConfig::Chatterbox {
            language,
            quality_steps: cb_quality,
            speed: cb_speed,
            voice_id,
        } => {
            log::info!("[TTS Actor] Initializing Chatterbox engine");
            let chatterbox_path = crate::utils::paths::model_dir("tts").join("chatterbox");
            let ref_audio = resolve_reference_audio(voice_id.as_deref());
            ChatterboxEngine::new(
                &chatterbox_path,
                language,
                *cb_quality,
                *cb_speed,
                ref_audio.as_deref(),
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

/// Spawns and initializes a persistent TTS worker actor thread.
#[allow(clippy::too_many_arguments)]
pub fn warm_up_tts(
    app: &tauri::AppHandle,
    tts_tx: &mut Option<std::sync::mpsc::Sender<TtsCommand>>,
    tts_handle: &mut Option<std::thread::JoinHandle<()>>,
    settings: &VoxSettings,
    super_tts_path: &Path,
    cancel_flag: Arc<AtomicBool>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    is_loaded: Arc<AtomicBool>,
    is_sleeping: Arc<AtomicBool>,
) -> Result<(), String> {
    if tts_tx.is_some() {
        return Ok(());
    }

    log::info!("[TTS Actor] Warming up TTS worker");
    let provider = create_tts_provider(settings, super_tts_path)?;

    let (tx, rx) = std::sync::mpsc::channel::<TtsCommand>();
    *tts_tx = Some(tx);

    let app_clone = app.clone();
    let handle = std::thread::Builder::new()
        .name("vox-tts-persistent".to_string())
        .spawn(move || {
            spawn_tts_worker(app_clone, rx, provider, event_tx, cancel_flag, is_loaded);
        })
        .map_err(|e| e.to_string())?;

    *tts_handle = Some(handle);
    is_sleeping.store(false, Ordering::Relaxed);
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

        for i in 0..chars.len() {
            let (pos, c) = chars[i];

            if c == '\n' || c == '?' || c == '!' {
                return Some((pos, c.len_utf8()));
            }

            if c == ',' || c == ';' || c == ':' || c == '—' || c == '–' {
                return Some((pos, c.len_utf8()));
            }

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

    #[test]
    fn test_tts_clause_chunker_abbreviations() {
        let mut chunker = TtsClauseChunker::new();
        let input = "Dr. Smith tested version 0.8.6 at 3.14 PM.";
        let chunks = chunker.push_str(input);

        assert_eq!(chunks.len(), 1, "Expected single chunk, got {:?}", chunks);
        assert_eq!(chunks[0], "Dr. Smith tested version 0.8.6 at 3.14 PM.");

        let mut chunker2 = TtsClauseChunker::new();
        let input2 = "Tested v0.8.6 release.";
        let chunks2 = chunker2.push_str(input2);
        assert_eq!(
            chunks2.len(),
            1,
            "Expected single chunk for version tag, got {:?}",
            chunks2
        );
        assert_eq!(chunks2[0], "Tested v0.8.6 release.");
    }

    #[test]
    fn test_tts_clause_chunker_punctuation() {
        let mut chunker = TtsClauseChunker::new();
        let input = "Hello world, how are you?\nI am doing well, thank you!";
        let chunks = chunker.push_str(input);

        assert_eq!(
            chunks,
            vec![
                "Hello world,",
                "how are you?",
                "I am doing well,",
                "thank you!"
            ]
        );
    }

    #[test]
    fn test_tts_turn_cancel_clears_buffer() {
        let mut chunker = TtsClauseChunker::new();
        let partial_input = "This is incomplete text without punctuation";
        let chunks = chunker.push_str(partial_input);

        assert!(
            chunks.is_empty(),
            "Partial text without punctuation should produce no chunks"
        );
        assert!(
            !chunker.is_empty(),
            "Buffer should contain partial text before cancel"
        );
        assert_eq!(chunker.buffer(), partial_input);

        chunker.clear();

        assert!(
            chunker.is_empty(),
            "Buffer should be empty after cancellation"
        );
        assert_eq!(chunker.buffer(), "");

        let new_turn_chunks = chunker.push_str("New turn sentence.");
        assert_eq!(new_turn_chunks, vec!["New turn sentence."]);
    }
}
