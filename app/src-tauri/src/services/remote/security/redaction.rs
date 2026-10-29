//! Secret redaction for controller-facing settings payloads.
//!
//! Spec: `docs/specs/remote-control-spec.md` §8.4 (Secret Redaction) and `VERIFY-RC-08`.

// ─── Constants & Type Aliases ────────────────────────────────────────────────

pub const REDACTED: &str = "[REDACTED]";

// ─── Data Structures ─────────────────────────────────────────────────────────

// ─── Trait Implementations ───────────────────────────────────────────────────

// ─── Inherent Implementations ────────────────────────────────────────────────

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Redacts all secret fields in a serialized `VoxSettings` JSON value in-place.
/// Enforces Spec §8.4 and VERIFY-RC-08.
pub fn redact_settings_value(root: &mut serde_json::Value) {
    redact_recursive(root, "");
}

fn redact_recursive(val: &mut serde_json::Value, path: &str) {
    match val {
        serde_json::Value::Object(map) => {
            // 1. Secret map containers: redact every value inside the map regardless of provider key name.
            if path == "llm.cloud_keys" || path == "realtime.api_keys" {
                for v in map.values_mut() {
                    *v = serde_json::Value::String(REDACTED.to_string());
                }
                return;
            }

            for (k, v) in map.iter_mut() {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };

                let is_secret_key = k == "api_key"
                    || k == "azure_key"
                    || k.ends_with("_key")
                    || k.ends_with("_token")
                    || k.ends_with("_secret")
                    || child_path == "tts.azure_tts.key";

                if is_secret_key && (v.is_string() || v.is_null()) {
                    *v = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redact_recursive(v, &child_path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_recursive(item, path);
            }
        }
        _ => {}
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_all_spec_secret_shapes() {
        let mut settings = json!({
            "appearance": {
                "theme": "dark",
                "accent_seed": "#6366f1"
            },
            "llm": {
                "active": "gpt-4o",
                "cloud_keys": {
                    "openai": "sk-proj-secret-token",
                    "anthropic": "sk-ant-secret-token"
                },
                "servers": [
                    { "name": "local-gpu", "api_key": "secret-server-key", "host": "10.0.0.5" }
                ]
            },
            "tts": {
                "azure_tts": {
                    "region": "eastus",
                    "key": "azure-live-speech-key"
                },
                "servers": [
                    { "name": "chatterbox", "api_key": "chatter-secret", "port": 8000 }
                ]
            },
            "auth": {
                "access_token": "token-12345",
                "client_secret": "secret-98765"
            }
        });

        redact_settings_value(&mut settings);

        // Safe fields preserved
        assert_eq!(settings["appearance"]["theme"], "dark");
        assert_eq!(settings["appearance"]["accent_seed"], "#6366f1");
        assert_eq!(settings["llm"]["active"], "gpt-4o");
        assert_eq!(settings["tts"]["azure_tts"]["region"], "eastus");
        assert_eq!(settings["llm"]["servers"][0]["host"], "10.0.0.5");
        assert_eq!(settings["tts"]["servers"][0]["port"], 8000);

        // Secret maps redacted
        assert_eq!(settings["llm"]["cloud_keys"]["openai"], REDACTED);
        assert_eq!(settings["llm"]["cloud_keys"]["anthropic"], REDACTED);

        // Nested server array keys redacted
        assert_eq!(settings["llm"]["servers"][0]["api_key"], REDACTED);
        assert_eq!(settings["tts"]["servers"][0]["api_key"], REDACTED);

        // Bare key at tts.azure_tts.key redacted
        assert_eq!(settings["tts"]["azure_tts"]["key"], REDACTED);

        // Suffixes _token and _secret redacted
        assert_eq!(settings["auth"]["access_token"], REDACTED);
        assert_eq!(settings["auth"]["client_secret"], REDACTED);
    }
}
