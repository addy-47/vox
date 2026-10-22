# ADR: Vox Remote Control — Architecture Decisions

**Status:** Accepted · **Date:** 2026-10-19 · **Scope:** Vox Remote Control v1 (LAN)

Governed by [`docs/specs/remote-control-spec.md`](../../specs/remote-control-spec.md). This ADR records *why*, and the module layout. The spec records *what*.

---

## 1. Context

A phone must act as the microphone, the speaker and the UI surface for a laptop running the full Vox pipeline. The laptop is always the pipeline host; the phone carries zero model weights and runs zero local inference.

The feature introduces the first inbound network listener in the codebase, a second identity model, a second settings surface, and a second source of audio. Each of those is a place where a wrong decision is expensive to undo.

---

## 2. Decisions

### ADR-RC-01 — Key material: a single X25519 keypair

`DeviceKey` **is** the Noise_XX static key. There is no Ed25519 key.

**Why.** Noise_XX authenticates via X25519 static keys. Using the same key for identity and transport means the identity and the transport credential are the same object by construction: there is nothing to keep in sync, and a `DeviceId` cannot be forged without also forging the Noise handshake. `DeviceId = SHA-256("vox/device-id/v1" ‖ x25519_public_key)`.

**Cost.** No signing key for future non-transport security. Accepted — v1 has no such need.

### ADR-RC-02 — Transport layering: TCP → WebSocket → Noise_XX, no TLS

**Why.** Noise already provides mutual authentication, confidentiality, forward secrecy and replay protection. Adding TLS would introduce a second trust anchor and a certificate lifecycle that a LAN peer has no way to provision — the phone would have to trust a self-signed certificate it received over an unauthenticated channel, which buys nothing.

**Cost.** The LAN traffic is not readable by a passive observer (Noise), but the WebSocket itself is `ws://`, so a passive observer can see *that* two devices are talking. Accepted.

### ADR-RC-03 — The audio socket runs its own Noise handshake

**Why.** The first draft of the spec authenticated the audio socket with a session token only. That left the voice stream unencrypted and unauthenticated against an off-path attacker. The audio socket now completes a full Noise handshake against the same static key as the control channel, and *additionally* sends the session token, which binds the audio socket to the same logical session as the control channel — something a standalone handshake cannot express.

**Cost.** One extra handshake per link. Negligible.

### ADR-RC-04 — One canonical `RemoteLinkState` enum, no booleans

**Why.** The spec originally specified a `controller_attached: bool` on the host. That violates `.agents/rules/backend-style-guide.md` §5 ("never introduce boolean flags that shadow state transitions"), and it was the direct cause of the scattered, contradictory checks the first design produced across the engine, dictation, lifecycle, router and gateway.

The enum is shared by both roles. The controller drives the full dial-driven machine; the host occupies a subset (`Disabled`, `Disconnected`, `Connected`) because the host never dials. One type, one implementation, one query site.

### ADR-RC-05 — The guard is one function, driven by one value, with an explicit origin

**Why.** A disabled button is an affordance, not an enforcement boundary. But the backend guard and the UI disable must never disagree, or the UI will offer a button the backend refuses. So both read the same `RemoteLinkState`.

The call origin is an explicit `ControlOrigin` parameter on the affected service functions — not a thread-local, not a global flag, not a boolean. The desktop path passes `Desktop`; the gateway passes `Controller`. The rule applies only to `Desktop`.

### ADR-RC-06 — Guard scope: all local writes are blocked, appearance excepted

**Why.** During a remote session the phone is the surface the user is looking at, and the desktop is a passive observer. If the desktop can mutate state, it changes what the phone is showing — without the phone's view ever refreshing to match. A user reading a notification on the phone must be able to trust that nothing on the laptop changed underneath them.

So the rule is a single statement: **all local writes are blocked while a remote session is active, except the appearance domain.** Reads stay available so the desktop can still browse history, settings, memory and notifications.

Two carve-outs exist because the situation that needs them is precisely the situation in which a phone is attached: `remote_disconnect_device` (end this phone's link, keep the pairing) and `remote_revoke_device` (permanently remove a lost device) stay available, along with pairing a further device and turning remote control off entirely. Blocking "revoke my lost phone" while another phone is connected would be absurd.

**Cost.** The desktop loses nearly all of its mutating power during a remote session. That is the intent, not a side effect.

**Default-deny.** The implementation is an allowlist: a command is permitted while a remote session is active only if it is explicitly classified as a read, as window chrome, as the appearance domain, or as remote-control administration. A command added later is blocked until someone deliberately classifies it. A forgotten classification costs a disabled button, not a corrupted session.

### ADR-RC-07 — The host refuses a controller it cannot serve, with a distinct reason

**Why.** Three cases were undefined and each produced a bad user experience: the host was not accepting, the host was mid-session, and another controller was attached. In all three the controller needs to say something specific, not "connection failed". The host also keeps advertising mDNS with an `accepts` flag even when it refuses, so "not accepting" is distinguishable from "offline".

### ADR-RC-08 — The host can end a controller's participation without destroying the pairing

**Why.** Revocation (delete the row) and "stop this phone now" (close the link, keep the row) are different user intents. A phone left on the sofa is not a lost phone. The `LinkLoss` reason `host_disconnected` carries it.

### ADR-RC-09 — The snapshot carries the recent transcript

**Why.** A controller pairing mid-conversation would otherwise open onto a blank screen while the host is mid-turn, contradicting US-RC-03.

### ADR-RC-10 — The backend owns remote state and pushes it

**Why.** The frontend must never poll for state. One `remote_state_changed` event carries the canonical enum on both roles. A read command exists **only on the controller**, and only to hydrate the initial render — the controller auto-reconnects, so its state can already be `Reconnecting` before the UI mounts. The host has no read command: a freshly booted host cannot have a controller attached, so the desktop starts at `Disconnected` and follows the event.

### ADR-RC-11 — Module layout: `services/remote/` as a tree, not a flat directory

**Why.** A flat `services/remote/` with 21 files would mix a pure protocol definition with a TCP listener, a session state machine and a command gateway. The tree makes the layering explicit and gives the extension seams their own homes:

| Module | Owns | Must never contain |
| :--- | :--- | :--- |
| `protocol/` | wire messages, framing, capabilities, wire errors | I/O, state, Tauri types |
| `link/` | `RemoteLink` trait, `LoopbackLink`, `WsLink`, link lifecycle | protocol semantics, app state |
| `security/` | identity, key storage, Noise, pairing, trust store | session state, IPC |
| `session/` | `RemoteLinkState`, `SessionRole`, session manager, heartbeat, snapshot | protocol encoding, Tauri commands |
| `control/` | routing table, arg specs, gateway, redaction, event policy | transport, audio |
| `discovery/` | address hints, mDNS, dial candidates | session state |
| `audio/` | network capture, network sink, pacing | session state, protocol encoding |
| `testing/` | headless reference client | production logic |

Two rules do the heavy lifting:
- `protocol/` is **pure**. No `tokio`, no `tauri`, no filesystem. Both roles and the test suite depend on it with no I/O, so the wire contract cannot drift.
- Host and Controller are **roles, not branches**. `session/manager.rs` implements the session once, parameterised by `SessionRole`. A separate `host.rs` and `client.rs` would drift.

### ADR-RC-12 — The gateway calls existing command functions; no bulk extraction

**Why.** The commands are `pub async fn` taking `AppHandle` + `State`, and the codebase already uses the `let state = app.state(); ...(&state)` pattern that derefs into the service layer. A gateway holding an `AppHandle` can therefore call the existing command functions directly, with per-command argument structs deserialising the JSON `args`. Only the seven guarded commands need the `ControlOrigin` split.

**Cost.** The gateway depends on the command signatures staying stable, and needs ~330 lines of argument-struct boilerplate.

### ADR-RC-13 — One agent implements and self-tests a wave; the gate is independent

**Why.** Splitting implementation and testing across two agents inside a wave doubles the iteration cost, because the tester finds defects the implementer could have fixed in the same pass. The independence that matters is at the **gate**, not inside the wave.

Per wave: one implementer writes the code *and* its tests; then a test engineer produces independent evidence; then a QA engineer reviews adversarially and issues the verdict. A failed gate produces scoped fix tickets, which re-enter the same path.

**Cost.** The implementer grades its own work until the gate. The gate is the control, and it is a real one: the QA engineer reads raw evidence, not a summary.

### ADR-RC-14 — The remote-control dependency set is viable on the Android target

**Finding (wave W0, ticket 01).** The three crates the remote-control feature depends on were added to the dependency set and the dual-target compile gate was run with them present. **All three compile for `aarch64-linux-android` with 0 warnings and 0 errors.** No crate had to be dropped, and no fallback is required.

| Crate | Pinned version | Android verdict | Basis |
| :--- | :--- | :--- | :--- |
| `snow` | `0.10` | **Viable** | Pure Rust. The default resolver backend is `ring`, which the crate already depends on through `rustls`; the lockfile gained no new native toolchain, and `ring` already cross-compiles for `aarch64-linux-android`. |
| `mdns-sd` | `0.21` | **Viable** | Pure Rust, no async-runtime dependency. Its transitive tree is `if-addrs` (libc `getifaddrs`), `socket2`, `socket-pktinfo`, `flume` and `log`, all of which build for the Android target. |
| `qrcode` | `0.14` | **Viable** | Pure Rust. The `image` feature is disabled (`default-features = false, features = ["svg", "pic"]`) so the `image` crate's transitive tree stays out of the Android build; the SVG renderer and the raw module matrix cover the pairing surface. |

**Lockfile side effect.** `libc` moved `0.2.186` → `0.2.191`. This is forced by `socket-pktinfo 0.4.1`, which `mdns-sd 0.21.6` requires. No other existing crate changed version, and no existing crate was removed.

**Functional evidence, not only a compile.** A crate that compiles is not yet a crate that works. `tests/remote_dependency_smoke_test.rs` exercises all three at runtime, in memory, with no socket and no device: a full Noise_XX handshake (`Noise_XX_25519_ChaChaPoly_BLAKE2s`) between two peers, after which the peers read each other's transport payload — which is only possible when both sides derived the same session key; an mDNS `_vox._tcp.local` service record carrying the host's device id in the TXT record; and a pairing payload encoded to a QR matrix and rendered to SVG.

**Contingency.** If `mdns-sd` ever fails to build for the Android target, the named fallback is the one already agreed in the remote-control spec: **discovery by QR address hint plus manual entry only, with mDNS deferred** (§6.1, the "Manual fallback" row). The QR carries `hint_addr:hint_port`, so the primary discovery path is unaffected by the loss of mDNS. The contingency is carried by the discovery ticket in wave W8, not by this wave.

**Cost.** Three new dependencies and one transitive `libc` bump. No platform feature was enabled globally, and no target-scoped dependency was added — none of the three crates needs one (`.agents/rules/android-style-guide.md` Pitfall 5).

---

### ADR-RC-15 — The byte-injection seam is a test trait, not part of the transport contract

**Decision.** The `RemoteLink` trait carries only the six operations a transport
actually performs. The ability to put an arbitrary byte string on a link —
the "send uncapped, unencoded bytes" capability the conformance suite needs
in order to place a malformed frame on the wire — lives on a separate
`RawLink: RemoteLink` trait that the test tree uses and production code does
not.

**Why.** The first version put both methods on `RemoteLink`, marked
`#[doc(hidden)]`. A hidden method on a public trait is still a method every
transport must implement, so wave W2b's WebSocket link would have been handed
"write arbitrary bytes straight onto the socket, bypassing the encoder and the
size cap". That is a dangerous capability to put in the hands of a network
transport, and it would have been implemented because the trait required it,
not because anything needed it.

The conformance suite needs the capability because a suite that can only send
well-formed messages cannot prove that a transport rejects a malformed one.
That is a property of the *test*, not of the link contract.

**Cost.** Two traits instead of one, and the conformance suite's bound becomes
`L: RawLink + 'static`. The production trait is four methods smaller, and the
next transport implements four methods instead of six.

---

## 3. Rejected alternatives

| Alternative | Why rejected |
| :--- | :--- |
| TLS with self-signed certificates and a manual fingerprint | A second trust anchor for a capability Noise already provides, plus a certificate lifecycle a LAN peer cannot provision. |
| A separate Ed25519 identity key | Two key types, two derivations, two places to get wrong, for no capability v1 uses. |
| `controller_attached: bool` | Violates the repo's own style guide; caused the scattered checks. |
| A thread-local caller marker | Invisible coupling; a missed call site silently changes behaviour. An explicit parameter is checked by the compiler. |
| Polling for remote state | The backend already knows; polling adds latency and a second source of truth. |
| A separate `remote_device_connected` / `remote_device_disconnected` event pair | Forces the frontend to rebuild a state machine from two edges. One event carrying the enum is strictly better. |
| Silently no-oping blocked local writes | The UI would show state that was never applied. An explicit error is honest. |
| A flat `services/remote/` directory | Mixes a pure protocol definition with a TCP listener and a command gateway. |
| Separate `host.rs` and `client.rs` session implementations | Two implementations of one state machine; they drift. |
| Two agents (implement + test) inside each wave | Doubles the iteration cost. Independence belongs at the gate. |
| Byte injection on the production `RemoteLink` trait | A hidden method is still a method every transport must implement, so the WebSocket link would inherit "write arbitrary uncapped bytes". |
| A bespoke `Capabilities` bitmask type | The spec writes the field as `1` and never enumerates a capability list. A `u64` mask with one named bit is the whole requirement. |
| An injected `Clock` trait for the pairing token lifetime | One implementation, boxed behind `dyn`, with two injection constructors, to supply one timestamp. Passing `now: Instant` is the same thing without the machinery. |

---

## 4. Consequences

**Negative.**
- The LAN shows WebSocket traffic between two devices (encrypted payloads, visible endpoints).
- The gateway depends on the IPC command signatures.
- The `protocol/` module is deliberately I/O-free, which makes the wire contract testable but means the codec and framing logic cannot be shared with the audio path.

**Positive.**
- A second transport (relay, QUIC) is one new `Connector` plus one new `Link`; `protocol/`, `session/`, `control/` and `audio/` are untouched.
- The whole state model is one enum with one query site, so "is a remote session active?" has exactly one answer.
- The wire contract is testable without a socket, a device, or a model.
