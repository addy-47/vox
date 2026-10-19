# Vox Remote Control: Ticket Plan

LAN-only v1, screen-on foreground use, built for you first. Off-LAN, Opus, and background use are scheduled, not designed out.

---

## Rubric

| Tag | Meaning |
| --- | --- |
| **Mandatory** | Remote mode does not work without it |
| **Agnostic enabler** | A contract or seam: no desktop logic changes, keeps parts swappable and off-LAN cheap |
| **Pure UX** | No functional dependency; can slip without breaking the feature |
| **Hardening** | Verification or robustness; needed for trust, not for the first demo |
| **Later** | Out of v1 |

---

## Extension Guard Rails

These cost little now and are what let off-LAN arrive later as one new Connector instead of a refactor.

1. A peer is a device ID plus key. Addresses are replaceable hints held as a candidate list, never the identity.
2. Every caller dials through a Connector. Only LanConnector exists in v1.
3. Authentication is a signed device credential at the app layer, so it rides unchanged over any future transport.
4. The wire protocol is versioned. A Hello message negotiates version, codec and capabilities.
5. One conformance suite (T1.6) must pass for every Link and Connector.

---

## Dependency Waves

Everything inside one wave can be built in parallel once earlier waves land.

**Critical path to M3 (19 size units):** T0.1 → T1.1 → T1.2 → T2.2 → T2.3 → T4.1 → T4.4 → T4.5 → T4.7. Anything off this path has slack.

### Wave 1
- **T0.1 ★** ADR: wire protocol and extension seams
- **T0.2** Working TTS on the laptop (A8)

### Wave 2
- **T0.3** Command and settings routing table
- **T1.1 ★** remote_proto message module

### Wave 3
- **T1.2 ★** RemoteLink + Connector traits, LoopbackLink

### Wave 4
- **T1.3** WsLink over rustls (host and client)
- **T1.4** NetworkAudioCapture
- **T1.5** NetworkAudioSink + real-time pacer
- **T2.2 ★** Composite IpcEmitter + event policy

### Wave 5
- **T1.6** Conformance + loopback e2e suite
- **T2.1** SessionOrigin + capture-source arbitration
- **T2.3 ★** Command gateway + attach snapshot
- **T2.4** Gate signal + playout-aware ducking
- **T4.3** Phone audio playout + jitter buffer

### Wave 6
- **T2.5** Desktop reference client
- **T4.1 ★** Phone RemoteClient (Rust)
- **T4.2** Phone audio uplink
- **T3.1** Host identity + pairing protocol
- **F1** Opus behind the codec byte

### Wave 7
- **T4.4 ★** TS RemoteTransport routing
- **T3.2** Laptop pairing UI
- **T3.3** Phone pairing UI
- **T5.1** Heartbeat, link-loss pause, reconnect
- **T5.2** Address refresh + mDNS fallback
- **T5.4** Security hardening pass
- **F2** Off-LAN Connector (QUIC or WebRTC)

### Wave 8
- **T4.5 ★** remote_control setting + mode switch
- **T5.3** Remote failure messaging
- **F3** Background and screen-off use
- **F4** Multiple devices or hosts

### Wave 9
- **T4.6** On-device Wi-Fi turn + latency baseline
- **T4.7 ★** Mode picker, onboarding branch, environment badge

---

## Tickets

| ID | Ticket | Tag | Milestone | Size | Blocked by | Unblocks |
| --- | --- | --- | --- | --- | --- | --- |
| **T0.1 ★** | ADR: wire protocol and extension seams. Identity is a device key, never an IP. Versioned hello. Auth independent of transport. Connector and Link traits defined. | Agnostic enabler | M1 no phone | S | none | T0.3, T1.1, T2.1, T3.1 |
| **T0.2** | Working TTS on the laptop (A8). Edge TTS returns 403 for every client. Switch the default (supertonic) and decide edge_tts's fate. Host TTS is required in remote mode. | Mandatory | M1 no phone | S | none | none |
| **T0.3** | Command and settings routing table. Classify every IPC command as local, remote, split or denied. Basis for the gateway, TS routing and a static test. | Agnostic enabler | M1 no phone | M | T0.1 | T2.3, T4.4 |
| **T1.1 ★** | remote_proto message module. Req/Res/Err with IPC error categories, Event, AudioFrame{codec, seq, sampleOffset}, PttMarker, Gate, Flush, Heartbeat, Hello. | Agnostic enabler | M1 no phone | M | T0.1 | T1.2, T2.3 |
| **T1.2 ★** | RemoteLink + Connector traits, LoopbackLink. Ordered control channel plus audio channel. Connector.dial(peer) returns a Link. In-process loopback first. | Agnostic enabler | M1 no phone | M | T1.1 | T1.3, T1.4, T1.5, T2.2, T2.3 |
| **T1.3** | WsLink over rustls (host and client). Two sockets so telemetry never stalls audio. Dev cert until T3.1. Per-channel backpressure rules. | Mandatory | M1 no phone | L | T1.2 | T2.5, T4.1, T4.2, T4.3, T3.1 |
| **T1.4** | NetworkAudioCapture. Implements AudioCapture. Resample to 16 kHz mono, handle sequence gaps, inject in-order PTT markers as VoxEvents. | Mandatory | M1 no phone | M | T1.2 | T1.6, T2.1, T2.4, F1 |
| **T1.5** | NetworkAudioSink + real-time pacer. Implements AudioSink. 20 ms paced frames. Flush on pause and barge-in. | Mandatory | M1 no phone | M | T1.2 | T1.6, T2.4 |
| **T1.6** | Conformance + loopback e2e suite. WAV in, full headless turn, audio out. Every future Connector must pass the same suite. | Hardening | M1 no phone | M | T1.4, T1.5 | F2 |
| **T2.1** | SessionOrigin + capture-source arbitration. Engine picks capture by origin. Laptop dictation hotkey gated during a remote session. Never fall back to the laptop mic. Amend ownership-spec. | Mandatory | M1 no phone | M | T0.1, T1.4 | T2.5, T5.1 |
| **T2.2 ★** | Composite IpcEmitter + event policy. Fan out to windows and the remote subscriber. Telemetry about 30 Hz drop-oldest, tokens throttled. | Mandatory | M1 no phone | M | T1.2 | T2.3 |
| **T2.3 ★** | Command gateway + attach snapshot. Allowlisted dispatch into existing services, secret redaction, state snapshot on attach. | Mandatory | M1 no phone | L | T0.3, T1.1, T1.2, T2.2 | T2.5, T4.1, T3.1, T5.4 |
| **T2.4** | Gate signal + playout-aware ducking. Host tells the client when ingestion is closed. Extend the duck window by playout depth plus half the RTT. | Mandatory | M2 phone on Wi-Fi | M | T1.4, T1.5 | T4.2 |
| **T2.5** | Desktop reference client. Headless client runs a full turn against the host. De-risks everything except Android itself. | Agnostic enabler | M1 no phone | M | T1.3, T2.1, T2.3 | none |
| **T3.1** | Host identity + pairing protocol. Cert fingerprint, single-use 2-minute token, device credential, paired_devices table, revoke, opt-in toggle (default off). | Mandatory | M3 usable v1 | L | T0.1, T1.3, T2.3 | T3.2, T3.3, T5.2, T5.4, F2 |
| **T3.2** | Laptop pairing UI. Remote devices settings: enable, QR, device list, revoke, connected indicator. | Pure UX | M3 usable v1 | M | T3.1 | none |
| **T3.3** | Phone pairing UI. QR scan with manual code fallback. Verify a scanner plugin works on Android first. | Pure UX | M3 usable v1 | M | T3.1, T4.1 | T4.7 |
| **T4.1 ★** | Phone RemoteClient (Rust). LanConnector over a candidate address list, remote_call relay, event re-emit as local Tauri events, connection state machine. | Mandatory | M2 phone on Wi-Fi | L | T1.3, T2.3 | T4.4, T4.5, T3.3, T5.1, T5.2 |
| **T4.2** | Phone audio uplink. Existing capture to frames, PTT markers in-band, stops when the gate closes. | Mandatory | M2 phone on Wi-Fi | M | T1.3, T2.4 | T4.6 |
| **T4.3** | Phone audio playout + jitter buffer. Adaptive buffer, drift compensation, Flush handling, playout acks to the host. | Mandatory | M2 phone on Wi-Fi | L | T1.3 | T4.6, F1 |
| **T4.4 ★** | TS RemoteTransport routing. Per-command routing inside transport.ts. Extend Invariant 1b. | Mandatory | M2 phone on Wi-Fi | M | T0.3, T4.1 | T4.5 |
| **T4.5 ★** | remote_control setting + mode switch. Stop the local engine and free the mic. Split the settings store into local UI and remote pipeline. | Mandatory | M2 phone on Wi-Fi | M | T4.1, T4.4 | T4.6, T4.7 |
| **T4.6** | On-device Wi-Fi turn + latency baseline. Exit gate for M2, using a dev token. Record time to first audio over the network, jitter and drift. | Hardening | M2 phone on Wi-Fi | S | T4.2, T4.3, T4.5 | none |
| **T4.7 ★** | Mode picker, onboarding branch, environment badge. Skip the 683 MB model download on "Use laptop". Label sessions by environment. | Pure UX | M3 usable v1 | M | T4.5, T3.3 | none |
| **T5.1** | Heartbeat, link-loss pause, reconnect. Missed beats send PauseSession, aging into Sleeping. Backoff, resync snapshot, resume. | Hardening | M3 usable v1 | M | T4.1, T2.1 | T5.3, F3 |
| **T5.2** | Address refresh + mDNS fallback. Retry stored candidates, then mDNS with a multicast lock. Handles DHCP changes. | Hardening | M3 usable v1 | M | T4.1, T3.1 | none |
| **T5.3** | Remote failure messaging. Toasts for unreachable host, revoked device, version mismatch, host busy. | Pure UX | M3 usable v1 | S | T5.1 | none |
| **T5.4** | Security hardening pass. Pairing rate limit, static test that every command is classified, redaction test. | Hardening | M3 usable v1 | M | T2.3, T3.1 | F4 |
| **F1** | Opus behind the codec byte. Swap PCM16 for Opus with no protocol change. | Later | M4 later | M | T4.3, T1.4 | none |
| **F2** | Off-LAN Connector (QUIC or WebRTC). Spike first. A new Connector only: gateway and audio adapters untouched. Must pass the T1.6 suite. | Later | M4 later | L | T1.6, T3.1 | none |
| **F3** | Background and screen-off use. Foreground service, wake and Wi-Fi locks, HyperOS autostart and battery settings. | Later | M4 later | L | T5.1 | none |
| **F4** | Multiple devices or hosts. Per-device sessions and arbitration. | Later | M4 later | L | T5.4 | none |

---

*Sizes: S ≈ 1 day, M ≈ 2–3 days, L ≈ 4–5 days. Milestones: M1 proof without a phone, M2 phone works on Wi-Fi (dev token), M3 usable v1, M4 later. ★ marks the critical path through M1 to M3.*