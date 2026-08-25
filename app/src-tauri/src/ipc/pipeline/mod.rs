pub mod assistant;
pub mod dictation;
pub mod test_clip;

pub use assistant::*;
pub use dictation::*;
pub use test_clip::*;

#[derive(serde::Serialize)]
pub struct RealtimeSessionCache {
    pub has_session: bool,
    pub provider: String,
    pub expires_in_seconds: i64,
    pub model: String,
}

/// Returns cached real-time session resumption information.
#[tauri::command]
pub async fn get_realtime_session_cache() -> Result<RealtimeSessionCache, String> {
    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<serde_json::Value>(&data) {
                let expires_at = cached["expires_at"].as_u64().unwrap_or(0);
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let provider = cached["provider"].as_str().unwrap_or("").to_string();
                let model = cached["model"].as_str().unwrap_or("").to_string();
                let expires_in_seconds = (expires_at as i64 - now_ms as i64) / 1000;

                return Ok(RealtimeSessionCache {
                    has_session: expires_in_seconds > 0,
                    provider,
                    expires_in_seconds,
                    model,
                });
            }
        }
    }

    Ok(RealtimeSessionCache {
        has_session: false,
        provider: String::new(),
        expires_in_seconds: 0,
        model: String::new(),
    })
}
