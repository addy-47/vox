use std::{
    collections::{HashMap, HashSet},
    fmt,
    net::{IpAddr, UdpSocket},
    sync::Arc,
    time::Instant,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use parking_lot::Mutex;
use qrcode::{render::svg, QrCode};
use tokio::sync::oneshot;
use zeroize::Zeroize;

use super::{
    errors::{PairingRefusal, SecurityError},
    identity::{derive_device_id, DeviceId, PeerIdentity, X25519_KEY_BYTES},
    key_store::KeyStore,
    MAX_PREAUTH_MESSAGE_BYTES, PAIRING_APPROVAL_TIMEOUT, PAIRING_ATTEMPTS_PER_WINDOW,
    PAIRING_MAX_FAILED_ATTEMPTS, PAIRING_RATE_LIMIT_WINDOW, PAIRING_TOKEN_MIN_ENTROPY_BITS,
    PAIRING_TOKEN_TTL,
};

/// The number of bytes in a pairing token.
///
/// The token carries at least the 128 bits of entropy the spec demands.
pub const PAIRING_TOKEN_BYTES: usize = PAIRING_TOKEN_MIN_ENTROPY_BITS / 8;

/// The key a token is stored under.
type TokenKey = [u8; PAIRING_TOKEN_BYTES];

/// A single-use pairing token.
///
/// The token is memory-only. There is no database table for it, and there
/// never will be: a token that outlived the process would outlive the user's
/// intent to pair.
pub struct PairingToken {
    bytes: [u8; PAIRING_TOKEN_BYTES],
}

impl PairingToken {
    /// Generates a token with fresh cryptographic randomness.
    pub fn generate() -> Result<Self, SecurityError> {
        let mut bytes = [0u8; PAIRING_TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|error| SecurityError::Random(error.to_string()))?;
        Ok(Self { bytes })
    }

    /// The token as lowercase hex.
    pub fn to_hex(&self) -> String {
        hex::encode(self.bytes)
    }

    /// Parses a token from its hex form.
    pub fn from_hex(text: &str) -> Result<Self, SecurityError> {
        let bytes = hex::decode(text)
            .map_err(|_| SecurityError::PairingRefused(PairingRefusal::UnknownToken))?;
        if bytes.len() != PAIRING_TOKEN_BYTES {
            return Err(SecurityError::PairingRefused(PairingRefusal::UnknownToken));
        }
        let mut key = [0u8; PAIRING_TOKEN_BYTES];
        key.copy_from_slice(&bytes);
        Ok(Self { bytes: key })
    }
}

impl fmt::Debug for PairingToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingToken(redacted)")
    }
}

impl Drop for PairingToken {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// One issued token.
struct TokenEntry {
    /// When the token was issued.
    issued_at: Instant,
    /// Whether the token has already authorised a pairing.
    consumed: bool,
    /// The source addresses for which the token has been invalidated.
    invalidated: HashSet<IpAddr>,
}

/// What one source address has done.
#[derive(Default)]
struct SourceState {
    /// The timestamps of the recent attempts, for the rate limit.
    attempts: Vec<Instant>,
    /// The number of failed validations, per token.
    failures: HashMap<TokenKey, u32>,
}

/// The in-memory, process-local store of pairing tokens.
pub struct PairingTokenStore {
    state: Mutex<StoreState>,
}

/// The contents of the token store.
#[derive(Default)]
struct StoreState {
    tokens: HashMap<TokenKey, TokenEntry>,
    sources: HashMap<IpAddr, SourceState>,
}

impl Default for PairingTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingTokenStore {
    /// A store that reads the system clock.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(StoreState::default()),
        }
    }

    /// Issues a token and holds it in memory.
    ///
    /// `now` is the instant the token's lifetime is measured from.
    pub fn issue(&self, now: Instant) -> Result<PairingToken, SecurityError> {
        let token = PairingToken::generate()?;
        let mut state = self.state.lock();
        state.tokens.insert(
            token.bytes,
            TokenEntry {
                issued_at: now,
                consumed: false,
                invalidated: HashSet::new(),
            },
        );
        Ok(token)
    }

    /// Records a pairing attempt from a source address.
    ///
    /// The attempt is counted against the per-source limit, whether the
    /// attempt is ultimately refused or not.
    pub fn record_attempt(&self, source: IpAddr, now: Instant) -> Result<(), SecurityError> {
        let mut state = self.state.lock();
        prune(&mut state, now);
        let source_state = state.sources.entry(source).or_default();
        source_state
            .attempts
            .retain(|attempt| now.saturating_duration_since(*attempt) < PAIRING_RATE_LIMIT_WINDOW);
        if source_state.attempts.len() as u32 >= PAIRING_ATTEMPTS_PER_WINDOW {
            return Err(SecurityError::PairingRefused(PairingRefusal::RateLimited));
        }
        source_state.attempts.push(now);
        Ok(())
    }

    /// Validates a token and consumes it.
    ///
    /// A pairing token is single-use: the validation consumes it, so the
    /// token can never authorise two pairings. A failed validation — a wrong
    /// token, an expired token, or a token that was already consumed — counts
    /// against the source, and a source that fails
    /// `PAIRING_MAX_FAILED_ATTEMPTS` times invalidates the token for itself.
    ///
    /// The caller records the attempt itself, because it has to check the
    /// claimed identity against the handshake before the token is examined.
    pub fn validate_token(
        &self,
        token: &PairingToken,
        source: IpAddr,
        now: Instant,
    ) -> Result<(), SecurityError> {
        let mut state = self.state.lock();

        // The token itself.
        let refusal = match state.tokens.get_mut(&token.bytes) {
            None => Some(PairingRefusal::UnknownToken),
            Some(entry) => {
                if entry.invalidated.contains(&source) {
                    Some(PairingRefusal::Invalidated)
                } else if entry.consumed {
                    Some(PairingRefusal::Consumed)
                } else if now.saturating_duration_since(entry.issued_at) > PAIRING_TOKEN_TTL {
                    Some(PairingRefusal::Expired)
                } else {
                    entry.consumed = true;
                    None
                }
            }
        };

        match refusal {
            None => {
                if let Some(source_state) = state.sources.get_mut(&source) {
                    source_state.failures.remove(&token.bytes);
                }
                Ok(())
            }
            Some(refusal) => {
                record_failure(&mut state, &token.bytes, source);
                Err(SecurityError::PairingRefused(refusal))
            }
        }
    }

    /// Records a failed attempt against a token from a source address.
    ///
    /// A source that fails `PAIRING_MAX_FAILED_ATTEMPTS` times against a
    /// token loses that token: the token is invalidated for that source
    /// address, and only for it.
    pub fn record_failure(&self, token: &PairingToken, source: IpAddr) {
        let mut state = self.state.lock();
        record_failure(&mut state, &token.bytes, source);
    }
}

/// Drops the state that can no longer affect a decision.
///
/// A host on a LAN is reachable by every device on the network, so the
/// token map and the source map would otherwise grow without bound.
fn prune(state: &mut StoreState, now: Instant) {
    // A token is kept for one further lifetime after it expires, because an
    // expired token still affects one thing: it must be reported as *expired*
    // rather than as *unknown*. Dropping it at the expiry instant would make
    // the `Expired` refusal unreachable, and the user would be told their QR
    // code was never issued when it was simply stale.
    let retention = PAIRING_TOKEN_TTL + PAIRING_TOKEN_TTL;
    state
        .tokens
        .retain(|_, entry| now.saturating_duration_since(entry.issued_at) <= retention);
    state.sources.retain(|_, source_state| {
        source_state
            .attempts
            .iter()
            .any(|attempt| now.saturating_duration_since(*attempt) < PAIRING_RATE_LIMIT_WINDOW)
    });
}

/// Records a failed validation and invalidates the token for the source when
/// the source has failed too many times.
fn record_failure(state: &mut StoreState, key: &TokenKey, source: IpAddr) {
    let failures = {
        let source_state = state.sources.entry(source).or_default();
        let counter = source_state.failures.entry(*key).or_insert(0);
        *counter += 1;
        *counter
    };
    if failures >= PAIRING_MAX_FAILED_ATTEMPTS {
        if let Some(entry) = state.tokens.get_mut(key) {
            entry.invalidated.insert(source);
        }
    }
}

/// A pairing request: the token and the identity the peer claims.
///
/// The claimed identity is a **claim**, not a credential: it arrives before
/// any key is established, so anybody on the network can assert any device
/// id. It becomes trustworthy only when it is bound to the Noise handshake
/// that follows.
pub struct PairingRequest {
    token: PairingToken,
    claimed_device_id: DeviceId,
    claimed_public_key: Vec<u8>,
    display_name: String,
}

impl PairingRequest {
    /// Builds a request from the wire form: a hex token, a hex device id and
    /// a hex public key.
    ///
    /// The request is the one message that travels before the Noise handshake
    /// completes, so this is where the pre-authentication cap is enforced
    /// (remote-control spec §7.1). Without it, an unauthenticated peer on
    /// the LAN could make the host allocate an arbitrarily large request
    /// before any key is established.
    pub fn from_wire(
        token: &str,
        device_id: &str,
        public_key: &str,
        display_name: &str,
    ) -> Result<Self, SecurityError> {
        let size = token.len() + device_id.len() + public_key.len() + display_name.len();
        if size > MAX_PREAUTH_MESSAGE_BYTES {
            return Err(SecurityError::PreauthMessageTooLarge {
                size,
                limit: MAX_PREAUTH_MESSAGE_BYTES,
            });
        }
        let token = PairingToken::from_hex(token)?;
        let claimed_device_id = DeviceId::parse(device_id)?;
        let claimed_public_key =
            hex::decode(public_key).map_err(|_| SecurityError::IdentityMismatch)?;
        if claimed_public_key.len() != X25519_KEY_BYTES {
            return Err(SecurityError::IdentityMismatch);
        }
        Ok(Self {
            token,
            claimed_device_id,
            claimed_public_key,
            display_name: display_name.to_string(),
        })
    }

    /// The token the peer presented.
    pub fn token(&self) -> &PairingToken {
        &self.token
    }

    /// The device id the peer claims.
    pub fn claimed_device_id(&self) -> &DeviceId {
        &self.claimed_device_id
    }

    /// The public key the peer claims.
    pub fn claimed_public_key(&self) -> &[u8] {
        &self.claimed_public_key
    }

    /// The display name the peer claims.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Verifies that a claimed identity is the same object as the peer's static
/// key, read from the completed handshake.
///
/// Remote-control spec §5.3: the peer's static key, the `public_key` it
/// claimed and the `device_id` it claimed must all be the same object. If
/// they are not, the pairing is refused.
pub fn verify_claimed_identity(
    claimed_device_id: &DeviceId,
    claimed_public_key: &[u8],
    handshake_static: &[u8],
) -> Result<(), SecurityError> {
    if handshake_static != claimed_public_key {
        return Err(SecurityError::IdentityMismatch);
    }
    if derive_device_id(handshake_static) != *claimed_device_id {
        return Err(SecurityError::IdentityMismatch);
    }
    Ok(())
}

/// The user's answer to a pairing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingDecision {
    /// The user approved the pairing.
    Approve,
    /// The user rejected the pairing.
    Reject,
}

/// The result of a pairing request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingOutcome {
    /// The user approved the pairing. The token has been consumed.
    Approved {
        /// The peer the pairing is recorded under.
        peer: PeerIdentity,
    },
    /// The user rejected the pairing.
    Rejected,
    /// No decision arrived before the deadline.
    TimedOut,
}

/// The handle the approval surface uses to answer a pairing request.
///
/// The handle answers at most once: the second answer finds nothing to send
/// to.
#[derive(Clone)]
pub struct PairingPrompt {
    responder: Arc<Mutex<Option<oneshot::Sender<PairingDecision>>>>,
}

impl PairingPrompt {
    /// Approves the pairing.
    pub fn approve(&self) -> bool {
        self.answer(PairingDecision::Approve)
    }

    /// Rejects the pairing.
    pub fn reject(&self) -> bool {
        self.answer(PairingDecision::Reject)
    }

    /// Sends the decision, once.
    fn answer(&self, decision: PairingDecision) -> bool {
        let sender = self.responder.lock().take();
        match sender {
            Some(sender) => sender.send(decision).is_ok(),
            None => false,
        }
    }
}

/// A pairing request awaiting the user's decision.
pub struct PendingPairing {
    identity: PeerIdentity,
    prompt: PairingPrompt,
    decision: oneshot::Receiver<PairingDecision>,
}

impl fmt::Debug for PendingPairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingPairing")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl PendingPairing {
    /// The handle that answers this request.
    pub fn prompt(&self) -> PairingPrompt {
        self.prompt.clone()
    }

    /// The identity the pairing will be recorded under.
    pub fn identity(&self) -> &PeerIdentity {
        &self.identity
    }

    /// Waits for the decision, bounded by the approval timeout.
    pub async fn settle(self) -> PairingOutcome {
        match tokio::time::timeout(PAIRING_APPROVAL_TIMEOUT, self.decision).await {
            Ok(Ok(PairingDecision::Approve)) => PairingOutcome::Approved {
                peer: self.identity,
            },
            Ok(Ok(PairingDecision::Reject)) => PairingOutcome::Rejected,
            Ok(Err(_)) => PairingOutcome::Rejected,
            Err(_) => PairingOutcome::TimedOut,
        }
    }
}

/// The pairing flow: it issues tokens, validates requests, and binds a
/// claimed identity to a completed handshake.
pub struct PairingCoordinator {
    tokens: Arc<PairingTokenStore>,
}

impl Default for PairingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingCoordinator {
    /// A coordinator with its own token store.
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(PairingTokenStore::new()),
        }
    }

    /// Issues a pairing token.
    ///
    /// `now` is the instant the token's lifetime is measured from.
    pub fn issue_token(&self, now: Instant) -> Result<PairingToken, SecurityError> {
        self.tokens.issue(now)
    }

    /// The token store behind this coordinator.
    pub fn tokens(&self) -> &Arc<PairingTokenStore> {
        &self.tokens
    }

    /// Validates a pairing request against a completed handshake.
    ///
    /// The order is the spec's order (remote-control spec §5.3): the attempt
    /// is counted, the claimed identity is checked against the handshake, and
    /// only then is the token examined and consumed.
    ///
    /// The attempt is counted first, so an attempt is always counted against
    /// the source's rate limit — including an attempt whose claimed identity
    /// turns out to be a lie. A failed identity check counts against the
    /// token as well: a source that keeps presenting a token it cannot back
    /// with a handshake loses that token.
    pub fn begin(
        &self,
        request: PairingRequest,
        handshake_static: &[u8],
        source: IpAddr,
        now: Instant,
    ) -> Result<PendingPairing, SecurityError> {
        self.tokens.record_attempt(source, now)?;

        // The binding: the handshake's static key, the claimed public key and
        // the claimed device id must all be the same object.
        if let Err(error) = verify_claimed_identity(
            request.claimed_device_id(),
            request.claimed_public_key(),
            handshake_static,
        ) {
            self.tokens.record_failure(request.token(), source);
            return Err(error);
        }

        // The token: not expired, not used, not invalidated.
        self.tokens.validate_token(request.token(), source, now)?;

        // The identity the pairing is recorded under is the handshake's key,
        // not the claim: the claim has just been proved to be that key.
        let identity = PeerIdentity::new(
            derive_device_id(handshake_static),
            handshake_static.to_vec(),
            request.display_name().to_string(),
        )?;

        let (responder, decision) = oneshot::channel();
        Ok(PendingPairing {
            identity,
            prompt: PairingPrompt {
                responder: Arc::new(Mutex::new(Some(responder))),
            },
            decision,
        })
    }
}

/// Discovers the primary outgoing LAN IPv4 address.
pub fn get_local_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    match local_addr.ip() {
        IpAddr::V4(ipv4) if !ipv4.is_loopback() => Some(ipv4.to_string()),
        _ => None,
    }
}

/// Mints an ephemeral pairing payload JSON and renders it into an SVG QR data URL.
///
/// Returns `(payload_json, qr_svg_data_url)`.
pub fn mint_pairing_qr_svg(
    host_id: &DeviceId,
    public_key: &[u8],
    hint_addr: &str,
    hint_port: u16,
    token: &PairingToken,
    display_name: Option<&str>,
) -> Result<(String, String), SecurityError> {
    let name = display_name
        .filter(|s| !s.is_empty())
        .unwrap_or("Vox Host");
    let payload_val = serde_json::json!({
        "device_id": host_id.as_str(),
        "public_key": hex::encode(public_key),
        "display_name": name,
        "token": token.to_hex(),
        "hint_addr": hint_addr,
        "hint_port": hint_port,
    });
    let payload = serde_json::to_string(&payload_val)
        .map_err(|e| SecurityError::Noise(format!("Failed to serialize pairing payload: {e}")))?;

    let code = QrCode::new(payload.as_bytes())
        .map_err(|e| SecurityError::Noise(format!("Failed to generate QR code: {e}")))?;
    let svg = code.render::<svg::Color>().min_dimensions(240, 240).build();

    let data_url = format!(
        "data:image/svg+xml;base64,{}",
        BASE64.encode(svg.as_bytes())
    );
    Ok((payload, data_url))
}

/// Mints an ephemeral pairing invitation payload, QR code, and expiration for desktop host.
///
/// Returns `(payload_json, qr_svg_data_url, expires_at_secs, expires_at_ms)`.
pub fn mint_pairing_invitation(
    coordinator: &PairingCoordinator,
    port: u16,
) -> Result<(String, String, i64, i64), SecurityError> {
    let key_store = KeyStore::default_path()?;
    let device_key = key_store.load_or_create()?;
    let host_id = derive_device_id(device_key.public_key());
    let token = coordinator.issue_token(Instant::now())?;
    let hint_addr = get_local_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    let host_name = sysinfo::System::host_name().unwrap_or_else(|| "Vox Host".to_string());
    let (payload, qr_data_url) =
        mint_pairing_qr_svg(&host_id, device_key.public_key(), &hint_addr, port, &token, Some(&host_name))?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let expires_at_ms = now_ms + PAIRING_TOKEN_TTL.as_millis() as i64;
    let expires_at_secs = expires_at_ms / 1000;
    Ok((payload, qr_data_url, expires_at_secs, expires_at_ms))
}
