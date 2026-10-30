use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;

const AUDIO_TOKEN_TTL: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct TokenEntry {
    device_id: String,
    peer_static_pub: [u8; 32],
    expires_at: Instant,
}

/// Thread-safe in-memory store for single-use audio session tokens.
///
/// Spec §7.4 & VERIFY-RC-13: 32-byte cryptographically secure random token,
/// 10-second TTL, strictly bound to the authenticated control session static key,
/// single-use with anti-replay guarantees.
#[derive(Clone, Default)]
pub struct AudioSessionTokenStore {
    inner: Arc<Mutex<HashMap<String, TokenEntry>>>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AudioTokenError {
    #[error("Audio session token is invalid, expired, or already used")]
    InvalidOrExpired,
    #[error("Audio socket static key does not match control session static key")]
    KeyMismatch,
}

impl AudioSessionTokenStore {
    /// Creates a new empty `AudioSessionTokenStore`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Mints a random 32-byte hex token bound to the authenticated controller's device_id and Noise static key.
    pub fn mint(&self, device_id: &str, peer_static_pub: &[u8; 32]) -> String {
        let mut raw = [0u8; 32];
        getrandom::fill(&mut raw).expect("getrandom failed");
        let token = hex::encode(raw);

        let mut lock = self.inner.lock();
        let now = Instant::now();
        lock.retain(|_, entry| entry.expires_at > now);

        lock.insert(
            token.clone(),
            TokenEntry {
                device_id: device_id.to_string(),
                peer_static_pub: *peer_static_pub,
                expires_at: now + AUDIO_TOKEN_TTL,
            },
        );

        token
    }

    /// Single-use redemption: verifies token exists, is not expired, and matches the Noise static key.
    /// Anti-replay: Removes the token on attempt (VERIFY-RC-13).
    pub fn redeem(
        &self,
        token: &str,
        peer_static_pub: &[u8; 32],
    ) -> Result<String, AudioTokenError> {
        let mut lock = self.inner.lock();
        let entry = lock
            .remove(token)
            .ok_or(AudioTokenError::InvalidOrExpired)?;

        if Instant::now() > entry.expires_at {
            return Err(AudioTokenError::InvalidOrExpired);
        }

        if entry.peer_static_pub != *peer_static_pub {
            return Err(AudioTokenError::KeyMismatch);
        }

        Ok(entry.device_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_token_mint_and_redeem_success() {
        let store = AudioSessionTokenStore::new();
        let device_id = "test-device-id";
        let pubkey = [42u8; 32];

        let token = store.mint(device_id, &pubkey);
        assert_eq!(token.len(), 64);

        let redeemed = store.redeem(&token, &pubkey);
        assert_eq!(redeemed.expect("redeem failed"), device_id);
    }

    #[test]
    fn audio_token_anti_replay_single_use() {
        let store = AudioSessionTokenStore::new();
        let device_id = "test-device-id";
        let pubkey = [42u8; 32];

        let token = store.mint(device_id, &pubkey);
        let first = store.redeem(&token, &pubkey);
        assert!(first.is_ok());

        let second = store.redeem(&token, &pubkey);
        assert_eq!(second.unwrap_err(), AudioTokenError::InvalidOrExpired);
    }

    #[test]
    fn audio_token_key_mismatch_rejection() {
        let store = AudioSessionTokenStore::new();
        let device_id = "test-device-id";
        let pubkey = [42u8; 32];
        let wrong_pubkey = [99u8; 32];

        let token = store.mint(device_id, &pubkey);
        let result = store.redeem(&token, &wrong_pubkey);
        assert_eq!(result.unwrap_err(), AudioTokenError::KeyMismatch);
    }
}
