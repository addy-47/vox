//! ============================================================================
//! evals/common/keys.rs — API key resolution (flag -> env -> temp/.env)
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};

const ENV_KEYS: &[&str] = &["OPENROUTER_API_KEY", "NVIDIA_API_KEY"];

const DOTENV_CANDIDATES: &[&str] = &[
    "temp/.env",
    "../../temp/.env",
    "../../../temp/.env",
    "../../../../temp/.env",
];

fn read_dotenv_value(path: &PathBuf, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|x| x.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
            .unwrap_or(v);
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}

pub fn resolve_api_key(explicit: Option<&str>) -> Result<String> {
    if let Some(k) = explicit {
        let t = k.trim();
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    for name in ENV_KEYS {
        if let Ok(v) = std::env::var(name) {
            let t = v.trim();
            if !t.is_empty() {
                return Ok(t.to_string());
            }
        }
    }
    for c in DOTENV_CANDIDATES {
        let p = PathBuf::from(c);
        if p.exists() {
            for name in ENV_KEYS {
                if let Some(v) = read_dotenv_value(&p, name) {
                    return Ok(v);
                }
            }
        }
    }
    Err(anyhow!(
        "API key not found. Pass explicitly, set one of {:?}, or populate temp/.env",
        ENV_KEYS
    ))
}

pub fn resolve_api_key_optional(explicit: Option<&str>) -> Option<String> {
    resolve_api_key(explicit).ok()
}
