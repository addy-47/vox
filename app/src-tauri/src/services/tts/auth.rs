use std::time::{Duration, Instant};

/// Remaining freshness margin before a cached token is treated as expired.
const TOKEN_REFRESH_MARGIN_SECS: u64 = 60;

/// Expiry-aware bearer token cache shared by cloud TTS providers.
pub struct CachedToken {
    value: String,
    expires_at: Instant,
}

impl CachedToken {
    /// Caches a token valid for the given TTL from now.
    pub fn new(value: String, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    /// Returns the token only while it stays fresh beyond the refresh margin.
    pub fn get(&self) -> Option<&str> {
        if Instant::now() + Duration::from_secs(TOKEN_REFRESH_MARGIN_SECS) < self.expires_at {
            Some(&self.value)
        } else {
            None
        }
    }
}

/// Normalizes an Azure region shorthand or custom endpoint into a synthesis base URL.
pub fn normalize_azure_endpoint(region_or_endpoint: &str) -> String {
    let trimmed = region_or_endpoint.trim().trim_end_matches('/');
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}.tts.speech.microsoft.com")
    }
}

/// Redaction guard for secrets in logs. Never logs the input.
pub fn redact_secret() -> &'static str {
    "[redacted]"
}
