//! ============================================================================
//! src/services/pipeline/types.rs — Pipeline data types, enums, and audio reference helpers
//! ============================================================================

use crate::core::state::InteractionOwner;

pub enum TranslitTask {
    Token {
        turn_id: u32,
        target: String,
        token: String,
        local_transliterate_enabled: bool,
    },
    Partial {
        turn_id: u32,
        target: String,
        text: String,
        owner: InteractionOwner,
        local_transliterate_enabled: bool,
    },
    Final {
        turn_id: u32,
        target: String,
        text: String,
        owner: InteractionOwner,
        local_transliterate_enabled: bool,
    },
    Cancel {
        turn_id: u32,
    },
    Shutdown,
}

// ─── Voice resolution ────────────────────────────────────────────────────────

/// Resolves a voice UUID to a WAV file path for Chatterbox voice conditioning.
///
/// Opens a short-lived read connection to the DB and looks up the wav_path for
/// the given voice UUID. Returns `None` if the voice is not found, the DB is
/// unavailable, or the file has been deleted from disk — callers should treat
/// `None` as "use built-in voice" and log a warning.
pub(crate) fn resolve_reference_audio(voice_id: Option<&str>) -> Option<String> {
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

    // Prefer pre-baked voice_dir if it exists and contains speaker_emb.npy
    if let Some(ref dir) = entry.voice_dir {
        let path = std::path::Path::new(dir);
        if path.exists() && path.join("speaker_emb.npy").exists() {
            return Some(dir.clone());
        }
    }

    let wav = entry.wav_path?;
    if !std::path::Path::new(&wav).exists() {
        log::warn!(
            "[Pipeline] Voice {} wav_path not found on disk: {}. Using built-in voice.",
            id,
            wav
        );
        return None;
    }
    Some(wav)
}

// ─── Pipeline Orchestrator State ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Cold,
    Warm,
}

impl Default for PipelineState {
    fn default() -> Self {
        PipelineState::Cold
    }
}
