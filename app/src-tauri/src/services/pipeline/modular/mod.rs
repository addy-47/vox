pub mod context;
pub mod passive;
pub mod ptt;

use crate::core::events::VoxEvent;
use crate::core::settings::InteractionMode;
use crate::core::state::AppState;
use crate::services::audio::PlaybackEngine;
use std::sync::Arc;
use tauri::AppHandle;

/// Dispatches a VoxEvent to the active modular interaction mode handler.
pub fn handle_event<R: tauri::Runtime>(
    mode: InteractionMode,
    app: &AppHandle<R>,
    state: &AppState,
    playback: &Arc<PlaybackEngine>,
    event: VoxEvent,
) {
    match mode {
        InteractionMode::Passive => passive::handle_event(app, state, playback, event),
        InteractionMode::PTT => ptt::handle_event(app, state, playback, event),
    }
}
