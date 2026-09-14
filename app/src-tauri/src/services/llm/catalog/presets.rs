use std::sync::RwLock;

use once_cell::sync::Lazy;

use super::types::ProviderPresetMeta;

pub const BUNDLED_PROVIDERS_JSON: &str = include_str!("baseline_providers.json");

static PROVIDERS_CACHE: Lazy<RwLock<Vec<ProviderPresetMeta>>> = Lazy::new(|| {
    let list: Vec<ProviderPresetMeta> =
        serde_json::from_str(BUNDLED_PROVIDERS_JSON).unwrap_or_default();
    RwLock::new(list)
});

/// Returns the complete list of available provider presets.
pub fn list_presets() -> Vec<ProviderPresetMeta> {
    if let Ok(lock) = PROVIDERS_CACHE.read() {
        lock.clone()
    } else {
        serde_json::from_str(BUNDLED_PROVIDERS_JSON).unwrap_or_default()
    }
}

/// Alias for list_presets for backward compatibility.
pub fn provider_catalog() -> Vec<ProviderPresetMeta> {
    list_presets()
}

/// Looks up a provider preset by identifier.
pub fn lookup_preset(id: &str) -> Option<ProviderPresetMeta> {
    let lower = id.to_lowercase();
    let presets = list_presets();
    presets
        .into_iter()
        .find(|p| p.id == lower || p.name.to_lowercase() == lower)
}
