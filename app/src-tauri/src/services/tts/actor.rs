use crate::core::events::VoxEvent;
use crate::services::tts::providers::TtsProvider;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum TtsCommand {
    Generate {
        turn_id: u32,
        text: String,
    },
    /// Hot-update the number of diffusion steps (2-12).
    UpdateQualitySteps(u32),
    /// Hot-update the speed factor (0.7-2.0).
    UpdateSpeed(f32),
    Shutdown,
}

/// Spawn a persistent TTS worker thread.
///
/// The worker takes ownership of the provider and processes `TtsCommand`s
/// from the pipeline in a blocking loop. The provider must be fully initialized
/// before calling this function.
///
/// # Parameters
/// - `app` — Tauri app handle for emitting model lifecycle events.
/// - `rx` — Receiver for `TtsCommand` from the pipeline.
/// - `provider` — The TTS provider to use (e.g. Supertonic, Pocket, etc.).
/// - `event_tx` — Channel to emit `VoxEvent`s (TtsChunk, TtsFinished) back to the pipeline.
/// - `cancel_flag` — Shared atomic flag for barge-in cancellation.
/// - `is_loaded` — Set to true after successful init, false on shutdown.
pub fn spawn_tts_worker(
    app: tauri::AppHandle,
    rx: std::sync::mpsc::Receiver<TtsCommand>,
    provider: Box<dyn TtsProvider>,
    event_tx: std::sync::mpsc::Sender<VoxEvent>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
) {
    use tauri::Emitter;

    // Provider is pre-initialized — signal ready immediately.
    is_loaded.store(true, Ordering::Relaxed);
    let _ = app.emit(crate::core::constants::EVENT_MODEL_READY, "TTS");

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

/// Clause and sentence chunker for streaming TTS input.
///
/// Accumulates text fragments and splits them into speakable clause/sentence
/// chunks at natural boundaries (`,`, `;`, `:`, `—`, `.`, `!`, `?`, `\n`).
/// Protects abbreviations (Dr., Mr., vs., e.g., etc.), version tags (v0.8.6),
/// and decimal numbers (3.14) from invalid splitting.
#[derive(Debug, Default, Clone)]
pub struct TtsClauseChunker {
    buffer: String,
}

impl TtsClauseChunker {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Feed incoming text into the chunker and return any complete chunks.
    pub fn push_str(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        self.extract_chunks()
    }

    /// Flush any remaining text in the buffer as a final chunk.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Clear the buffer unconditionally (e.g. on turn cancellation or barge-in).
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// View current buffer contents.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.trim().is_empty()
    }

    /// Scan internal buffer for completed clauses and sentences.
    fn extract_chunks(&mut self) -> Vec<String> {
        let mut chunks = Vec::new();

        loop {
            if self.buffer.is_empty() {
                break;
            }

            let mut split_pos: Option<(usize, usize)> = None; // (char_byte_pos, char_len)
            let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

            for i in 0..chars.len() {
                let (pos, c) = chars[i];

                // Hard boundaries: newline, question mark, exclamation point
                if c == '\n' || c == '?' || c == '!' {
                    split_pos = Some((pos, c.len_utf8()));
                    break;
                }

                // Clause boundaries: comma, semicolon, colon, dash
                if c == ',' || c == ';' || c == ':' || c == '—' || c == '–' {
                    split_pos = Some((pos, c.len_utf8()));
                    break;
                }

                // Period boundary: check if abbreviation, decimal, or version string
                if c == '.' {
                    // Check 1: Decimal number (e.g., 3.14 or 0.8.6)
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
                        // Part of decimal/version string like 3.14 or 0.8.6 - skip
                        continue;
                    }

                    // Check 2: Preceding word abbreviation (e.g. Dr., Mr., v0., vs., e.g., etc.)
                    let text_before = &self.buffer[..pos];
                    let last_word = text_before
                        .split_whitespace()
                        .last()
                        .unwrap_or("")
                        .trim_matches(|p: char| !p.is_alphanumeric());

                    if is_abbreviation(last_word) {
                        // Skip abbreviation split
                        continue;
                    }

                    // Valid period boundary
                    split_pos = Some((pos, c.len_utf8()));
                    break;
                }
            }

            if let Some((pos, len)) = split_pos {
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

/// Helper function to detect standard abbreviations and version prefixes.
fn is_abbreviation(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }

    let lower = word.to_lowercase();

    // Standard title & common abbreviations
    const ABBREVS: &[&str] = &[
        "dr", "mr", "mrs", "ms", "prof", "sr", "jr", "st", "vs", "e.g", "i.e", "etc", "approx",
        "dept", "fig", "ver", "vol", "inc", "ltd", "co", "no", "p", "pg", "pp",
    ];

    if ABBREVS.contains(&lower.as_str()) {
        return true;
    }

    // Version prefixes like "v0", "v1", "v2"
    if lower.starts_with('v') && lower.len() > 1 && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // Single capital letter initial (e.g. "A." in "A. Smith")
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

        // Verify the chunker did NOT split on "Dr.", "v0." / "0.8.6", or "3." / "3.14"
        assert_eq!(chunks.len(), 1, "Expected single chunk, got {:?}", chunks);
        assert_eq!(chunks[0], "Dr. Smith tested version 0.8.6 at 3.14 PM.");

        // Additional abbreviation check with v0.8.6 explicitly
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

        // Verify chunks split cleanly at clause boundaries
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

        // Trigger turn cancellation / clear buffer
        chunker.clear();

        assert!(
            chunker.is_empty(),
            "Buffer should be empty after cancellation"
        );
        assert_eq!(chunker.buffer(), "");

        // Next turn should proceed cleanly without stale buffer text
        let new_turn_chunks = chunker.push_str("New turn sentence.");
        assert_eq!(new_turn_chunks, vec!["New turn sentence."]);
    }
}
