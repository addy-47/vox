use std::sync::RwLock;

use once_cell::sync::Lazy;

use super::types::ProviderPresetMeta;

pub const BUNDLED_PROVIDERS_JSON: &str = include_str!("baseline_providers.json");
pub const VENDOR_MODELPARAMS_JSON: &str = include_str!("modelparams_vendor.json");

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::transport::TransportType;

    /// Vendor param paths per modelparams.dev provider slug.
    fn vendor_param_paths() -> std::collections::HashMap<String, std::collections::HashSet<String>> {
        let raw: serde_json::Value =
            serde_json::from_str(VENDOR_MODELPARAMS_JSON).expect("vendor catalog must parse");
        let mut map: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        if let Some(models) = raw.get("models").and_then(|m| m.as_array()) {
            for entry in models {
                let provider = entry
                    .get("provider")
                    .and_then(|p| p.as_str())
                    .unwrap_or_default();
                let paths = map.entry(provider.to_string()).or_default();
                if let Some(params) = entry.get("params").and_then(|p| p.as_array()) {
                    for param in params {
                        if let Some(path) = param.get("path").and_then(|p| p.as_str()) {
                            paths.insert(path.to_string());
                        }
                    }
                }
            }
        }
        map
    }

    /// Validates the bundled manifest parses with every row sourced and unique.
    #[test]
    fn test_bundled_provider_manifest_is_sourced_ssot() {
        let presets = list_presets();
        assert!(
            presets.len() >= 14,
            "manifest must carry all presets, found {}",
            presets.len()
        );
        let mut ids = std::collections::HashSet::new();
        for preset in &presets {
            assert!(ids.insert(preset.id), "duplicate preset id: {}", preset.id);
            assert!(
                !preset.source.trim().is_empty(),
                "preset {} missing source citation",
                preset.id
            );
            assert!(
                !preset.checked.trim().is_empty(),
                "preset {} missing checked date",
                preset.id
            );
        }
        assert!(
            lookup_preset("ollama_openai_compat").is_some(),
            "ollama_openai_compat preset must exist"
        );
    }

    /// Validates transport/path coherence per row (no impossible combinations).
    #[test]
    fn test_provider_wire_path_coherence() {
        for preset in list_presets() {
            if preset.transport == TransportType::OllamaNative {
                assert!(
                    preset.token_limit.starts_with("options."),
                    "preset {}: native token path must nest under options",
                    preset.id
                );
                assert!(
                    preset.tool_choice.is_none(),
                    "preset {}: native transport rejects tool_choice",
                    preset.id
                );
                assert!(
                    !preset.stream_usage,
                    "preset {}: NDJSON has no stream_options",
                    preset.id
                );
            } else {
                assert!(
                    !preset.token_limit.contains('.'),
                    "preset {}: chat token path must be top-level",
                    preset.id
                );
            }
            if preset.id == "nvidia_nim" {
                assert!(
                    !preset.stream_usage,
                    "nvidia_nim: stream_options causes intermittent 503s"
                );
            }
        }
    }

    /// Cross-checks manifest mapping paths against the vendored community catalog.
    #[test]
    fn test_manifest_paths_exist_in_vendor_catalog() {
        let vendor = vendor_param_paths();
        assert!(
            vendor.len() >= 7,
            "vendor file must cover our cloud providers, found {}",
            vendor.len()
        );
        for preset in list_presets() {
            let Some(slug) = preset.catalog else { continue };
            let paths = vendor.get(slug).unwrap_or_else(|| {
                panic!("preset {} cites unknown vendor slug {}", preset.id, slug)
            });
            let token_key = preset.token_limit.rsplit('.').next().unwrap_or_default();
            assert!(
                paths.contains(token_key),
                "preset {}: token path {} missing from vendor {} params",
                preset.id,
                preset.token_limit,
                slug
            );
            if let Some(off) = &preset.reasoning_off {
                assert!(
                    paths.contains(off.path),
                    "preset {}: reasoning path {} missing from vendor {} params",
                    preset.id,
                    off.path,
                    slug
                );
            }
        }
    }
}
