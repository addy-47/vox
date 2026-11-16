use std::sync::{
    atomic::{AtomicU32, Ordering::Relaxed},
    mpsc,
};

use crate::{core::events::AudioIntent, services::tts::actor::TtsCommand};

pub type TtsSender = mpsc::Sender<TtsCommand>;

/// Dispatches interim filler speech to the TTS actor channel.
pub fn dispatch_tts_filler(
    tts_tx: &mpsc::Sender<TtsCommand>,
    turn_id: u32,
    text: String,
    pending_jobs: &AtomicU32,
) -> Result<(), String> {
    pending_jobs.fetch_add(1, Relaxed);
    if let Err(e) = tts_tx.send(TtsCommand::Generate {
        turn_id,
        text,
        intent: AudioIntent::InterimFiller,
    }) {
        pending_jobs.fetch_sub(1, Relaxed);
        return Err(format!("Failed to dispatch interim filler TTS: {}", e));
    }
    Ok(())
}

/// Dispatches a terminal or suspended turn response to the TTS actor channel.
pub fn dispatch_tts_response(
    tts_tx: &mpsc::Sender<TtsCommand>,
    turn_id: u32,
    text: String,
    pending_jobs: &AtomicU32,
) -> Result<(), String> {
    pending_jobs.fetch_add(1, Relaxed);
    if let Err(e) = tts_tx.send(TtsCommand::Generate {
        turn_id,
        text,
        intent: AudioIntent::TurnResponse,
    }) {
        pending_jobs.fetch_sub(1, Relaxed);
        return Err(format!("Failed to dispatch turn response TTS: {}", e));
    }
    Ok(())
}
