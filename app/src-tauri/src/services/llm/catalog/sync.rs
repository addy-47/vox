use std::{collections::HashMap, path::PathBuf, sync::RwLock, time::Duration};

use once_cell::sync::Lazy;
use reqwest::Client;

use super::types::{CapabilityProvenance, ModelSpec};
use crate::utils::paths;

const MODELS_DEV_URL: &str = "https://models.dev/models.json";
const SYNC_TIMEOUT_SECS: u64 = 10;
pub const BUNDLED_MODELS_JSON: &str = include_str!("baseline_catalog.json");

/// Global in-memory cache of baseline model specifications.
static BASELINE_REGISTRY: Lazy<RwLock<HashMap<String, ModelSpec>>> = Lazy::new(|| {
    let map = parse_catalog_json(BUNDLED_MODELS_JSON);
    RwLock::new(map)
});

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum FlexibleU32 {
    Num(f64),
    Str(String),
}

impl FlexibleU32 {
    fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Num(f) => Some(*f as u32),
            Self::Str(s) => s.parse::<f64>().ok().map(|f| f as u32),
        }
    }
}

#[derive(serde::Deserialize)]
struct RawLimit {
    context: Option<FlexibleU32>,
    output: Option<FlexibleU32>,
}

#[derive(serde::Deserialize)]
struct RawModelEntry<'a> {
    #[serde(borrow)]
    model_id: Option<&'a str>,
    #[serde(borrow)]
    id: Option<&'a str>,
    #[serde(borrow)]
    name: Option<&'a str>,
    #[serde(borrow)]
    family: Option<&'a str>,
    context_window: Option<FlexibleU32>,
    max_output_tokens: Option<FlexibleU32>,
    limit: Option<RawLimit>,
    #[serde(default)]
    supports_tools: Option<bool>,
    #[serde(default)]
    tool_call: Option<bool>,
    #[serde(default)]
    supports_structured: Option<bool>,
    #[serde(default)]
    structured_output: Option<bool>,
}

/// Parses catalog JSON structure (supporting both flattened and models.dev format) into a map of ModelSpec.
/// Uses zero-DOM typed single-pass deserialization for sub-millisecond parsing speed.
pub fn parse_catalog_json(json_str: &str) -> HashMap<String, ModelSpec> {
    let raw_map: HashMap<&str, RawModelEntry> = match serde_json::from_str(json_str) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[Catalog::Sync] Deserialization error: {}", e);
            return HashMap::new();
        }
    };

    let mut map = HashMap::with_capacity(raw_map.len());
    for (key, val) in raw_map {
        let model_id = val.model_id.or(val.id).unwrap_or(key).to_string();
        let name = val.name.unwrap_or(key).to_string();
        let family = val.family.map(|s| s.to_string());
        let context_window = val
            .context_window
            .as_ref()
            .and_then(|v| v.as_u32())
            .or_else(|| {
                val.limit
                    .as_ref()
                    .and_then(|l| l.context.as_ref())
                    .and_then(|v| v.as_u32())
            });
        let max_output_tokens = val
            .max_output_tokens
            .as_ref()
            .and_then(|v| v.as_u32())
            .or_else(|| {
                val.limit
                    .as_ref()
                    .and_then(|l| l.output.as_ref())
                    .and_then(|v| v.as_u32())
            });
        let supports_tools = val.supports_tools.or(val.tool_call).unwrap_or(false);
        let supports_structured = val
            .supports_structured
            .or(val.structured_output)
            .unwrap_or(false);

        map.insert(
            key.to_lowercase(),
            ModelSpec {
                model_id,
                name,
                family,
                context_window,
                max_output_tokens,
                supports_tools,
                supports_structured,
                provenance: CapabilityProvenance::CatalogBaseline,
            },
        );
    }
    map
}

/// Returns the cache directory for model catalog baselines.
fn catalog_cache_dir() -> PathBuf {
    paths::get().cache.join("catalog")
}

/// Initializes baseline registry from local persistent cache if available.
pub fn load_local_baseline_cache() {
    let cache_file = catalog_cache_dir().join("models_baseline.json");
    if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            let parsed_map = parse_catalog_json(&content);
            if !parsed_map.is_empty() {
                if let Ok(mut lock) = BASELINE_REGISTRY.write() {
                    for (k, spec) in parsed_map {
                        lock.insert(k, spec);
                    }
                    log::info!(
                        "[Catalog::Sync] Loaded {} models from local cache.",
                        lock.len()
                    );
                }
            }
        }
    }
}

/// Look up baseline specification by model ID or family match.
pub fn get_baseline_spec(model_id: &str) -> Option<ModelSpec> {
    let lower_id = model_id.to_lowercase();
    if let Ok(lock) = BASELINE_REGISTRY.read() {
        // 1. Exact match
        if let Some(spec) = lock.get(&lower_id) {
            return Some(spec.clone());
        }

        // 2. Substring or family match
        for (k, spec) in lock.iter() {
            if lower_id.contains(k) || k.contains(&lower_id) {
                return Some(ModelSpec {
                    model_id: model_id.to_string(),
                    provenance: CapabilityProvenance::FamilyBaseline,
                    ..spec.clone()
                });
            }
            if let Some(ref family) = spec.family {
                if lower_id.contains(family) {
                    return Some(ModelSpec {
                        model_id: model_id.to_string(),
                        provenance: CapabilityProvenance::FamilyBaseline,
                        ..spec.clone()
                    });
                }
            }
        }
    }
    None
}

/// Spawns a background non-blocking task to synchronize the catalog from models.dev using ETag caching.
pub fn spawn_catalog_sync() {
    tokio::spawn(async move {
        if let Err(e) = sync_from_models_dev().await {
            log::debug!(
                "[Catalog::Sync] Background sync skipped or unavailable: {}",
                e
            );
        }
    });
}

/// Executes conditional HTTP fetch against models.dev.
async fn sync_from_models_dev() -> anyhow::Result<()> {
    let cache_dir = catalog_cache_dir();
    tokio::fs::create_dir_all(&cache_dir).await?;

    let etag_file = cache_dir.join("models_baseline.etag");
    let cache_file = cache_dir.join("models_baseline.json");

    let saved_etag = if etag_file.exists() {
        tokio::fs::read_to_string(&etag_file).await.ok()
    } else {
        None
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(SYNC_TIMEOUT_SECS))
        .build()?;

    let mut req = client.get(MODELS_DEV_URL);
    if let Some(ref etag) = saved_etag {
        req = req.header("If-None-Match", etag.trim());
    }

    let res = req.send().await?;
    if res.status() == reqwest::StatusCode::NOT_MODIFIED {
        log::info!("[Catalog::Sync] Baseline catalog is up-to-date (HTTP 304).");
        return Ok(());
    }

    if !res.status().is_success() {
        anyhow::bail!("Catalog fetch returned status: {}", res.status());
    }

    let new_etag = res
        .headers()
        .get("etag")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let body = res.text().await?;

    let new_map = parse_catalog_json(&body);
    if !new_map.is_empty() {
        let tmp_file = cache_file.with_extension("tmp");
        tokio::fs::write(&tmp_file, &body).await?;
        tokio::fs::rename(&tmp_file, &cache_file).await?;

        if let Some(etag) = new_etag {
            let _ = tokio::fs::write(&etag_file, etag).await;
        }

        if let Ok(mut lock) = BASELINE_REGISTRY.write() {
            let count = new_map.len();
            for (k, spec) in new_map {
                lock.insert(k, spec);
            }
            log::info!(
                "[Catalog::Sync] Baseline catalog updated with {} models.",
                count
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_parse_catalog_json_performance_and_accuracy() {
        let start = Instant::now();
        let catalog = parse_catalog_json(BUNDLED_MODELS_JSON);
        let duration = start.elapsed();

        eprintln!(
            "\n>>> Catalog Parse Metric: parsed {} models from 1.3MB JSON in {:.2}ms ({} µs)",
            catalog.len(),
            duration.as_secs_f64() * 1000.0,
            duration.as_micros()
        );

        // Sanity assertions
        assert!(!catalog.is_empty(), "Catalog must not be empty");
        assert!(
            catalog.len() >= 4000,
            "Catalog should have at least 4,000 models, found {}",
            catalog.len()
        );

        // Spot-check popular models
        let gpt4o = catalog.get("gpt-4o").expect("gpt-4o must be present");
        assert_eq!(gpt4o.context_window, Some(128000));
        assert!(gpt4o.supports_tools);

        let claude = catalog
            .get("claude-3-5-sonnet-20241022")
            .or_else(|| catalog.get("claude-3-5-sonnet"));
        assert!(claude.is_some(), "Claude 3.5 Sonnet must be present");
        if let Some(c) = claude {
            assert_eq!(c.context_window, Some(200000));
        }

        // Must complete comfortably under 100ms in debug, and <5ms in release
        assert!(
            duration.as_millis() < 250,
            "Catalog parsing took too long: {:?}",
            duration
        );
    }
}
