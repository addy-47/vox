use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{
    services::{
        llm::{QWEN_MODEL_DIR, QWEN_MODEL_FILE},
        memory::{PRIMARY_EMBEDDING_MODEL_DIR, PRIMARY_EMBEDDING_MODEL_FILENAME},
        stt::{MODEL_FILE_ASR_ENCODER, NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR},
        tts::{CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR, SUPERTONIC_MODEL_DIR, ZIPVOICE_MODEL_DIR},
        vad::{MODEL_DIR_VAD, MODEL_FILE_VAD},
    },
    utils::paths,
};

/// Desktop default + ceiling for the wizard window; the minimum is the mobile floor.
const WIZARD_DEFAULT_WIDTH: f64 = 900.0;
const WIZARD_DEFAULT_HEIGHT: f64 = 650.0;
const WIZARD_MIN_WIDTH: f64 = 390.0;
const WIZARD_MIN_HEIGHT: f64 = 700.0;

/// Lazily constructs the wizard setup window on-demand.
pub fn ensure_wizard_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window("wizard") {
        return Ok(existing);
    }

    log::info!("[Wizard] Lazily constructing 'wizard' setup webview window...");
    let builder = WebviewWindowBuilder::new(app, "wizard", WebviewUrl::App("/wizard".into()))
        .title("Vox Setup Wizard")
        .inner_size(WIZARD_DEFAULT_WIDTH, WIZARD_DEFAULT_HEIGHT)
        .min_inner_size(WIZARD_MIN_WIDTH, WIZARD_MIN_HEIGHT)
        .max_inner_size(WIZARD_DEFAULT_WIDTH, WIZARD_DEFAULT_HEIGHT)
        .transparent(false)
        .decorations(false)
        .always_on_top(false)
        .resizable(true)
        .visible(false);
    #[cfg(target_os = "android")]
    let builder = builder.fullscreen(true);
    #[cfg(not(target_os = "android"))]
    let builder = builder.center();
    let window = builder
        .build()
        .map_err(|e| format!("Failed to create wizard window: {}", e))?;

    Ok(window)
}

/// Checks if all required models for a functional Vox experience are present on disk.
pub fn check_setup_health() -> bool {
    let p = paths::get();

    let vad_ok = p.models.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD).exists();
    if !vad_ok {
        return false;
    }

    let stt_ok = p
        .models
        .join(NEMOTRON_MODEL_DIR)
        .join(MODEL_FILE_ASR_ENCODER)
        .exists()
        || p.models
            .join(QWEN_ASR_MODEL_DIR)
            .join(MODEL_FILE_ASR_ENCODER)
            .exists();
    if !stt_ok {
        return false;
    }

    if !p.models.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE).exists() {
        return false;
    }

    let tts_ok = [
        SUPERTONIC_MODEL_DIR,
        KOKORO_MODEL_DIR,
        CHATTERBOX_MODEL_DIR,
        ZIPVOICE_MODEL_DIR,
    ]
    .iter()
    .any(|dir| dir_has_files(&p.models.join(dir)));
    if !tts_ok {
        return false;
    }

    let embedder_ok = p
        .models
        .join(PRIMARY_EMBEDDING_MODEL_DIR)
        .join(PRIMARY_EMBEDDING_MODEL_FILENAME)
        .exists();
    if !embedder_ok {
        log::warn!("[Health] Memory embedder model missing on disk");
    }

    log::info!("[Health] All core models verified in {:?}", p.models);
    true
}

/// Reports whether a model directory exists and contains at least one file.
/// Per-entry errors are logged, never silently dropped.
fn dir_has_files(dir: &std::path::Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(item) => {
                        if item.path().is_file() {
                            return true;
                        }
                    }
                    Err(err) => {
                        log::warn!("[Health] Model dir entry unreadable in {:?}: {}", dir, err);
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}
