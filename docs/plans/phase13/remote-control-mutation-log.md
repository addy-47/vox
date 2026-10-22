# Remote Control — Mutation Log

Every mutant seeded against the remote-control code, and what happened to it.

**Why this file exists.** A test that has never been shown to go red is not
evidence. This file is the record of that proof. A log that records only the
mutants that were caught is not a record of the coverage — it is a record of
the successes — so the mutants that **survived** are recorded here too, with
the hole they exposed and the test that closed it.

**Scope.** `app/src-tauri/src/services/remote/**` — `security/noise.rs`,
`security/pairing.rs`, `security/trust.rs`, `security/identity.rs`,
`security/key_store.rs`, `protocol/errors.rs`, `protocol/framing.rs`,
`protocol/messages.rs`, `link/loopback.rs`, `link/remote_link.rs`,
`services/remote/mod.rs`.

**Method.** Each mutant is one exact-string production replacement. After
each, the scoped test binaries run (`cargo nextest run --release
--test-threads=1 --no-fail-fast`) and the production file is restored
byte-exactly from a snapshot taken before the pass. The pass is run **once**
per wave, at the end, by the independent Verifier — not by the implementer.

**Score: 25 attempted, 25 killed, 0 survived.**

---

## 1. The mutation table

| # | Mutant (the one-line production change) | File | Killed by | Verdict |
| --- | --- | --- | --- | --- |
| 01 | `remote_static()` returns the **pin** instead of the revealed key | `security/noise.rs` | `test_a_handshake_against_an_impostor_fails_before_the_static_key_is_sent`, `test_the_pin_is_never_reported_as_the_remote_static_key` | KILLED |
| 02 | `verify_expected_peer` never returns an error | `security/noise.rs` | `test_a_handshake_against_an_impostor_fails_before_the_static_key_is_sent`, `test_the_pin_is_never_reported_as_the_remote_static_key` | KILLED |
| 03 | The pin is enforced *before* the peer has revealed its key | `security/noise.rs` | 8 tests in `remote_security_noise_test.rs`, incl. `test_the_pin_is_never_reported_as_the_remote_static_key` | KILLED |
| 04 | **The handshake message size cap is deleted** | `security/noise.rs` | **SURVIVED the pre-existing suite**; killed by `test_a_valid_handshake_message_over_the_cap_is_still_refused` | KILLED (after the new test) |
| 05 | The handshake cap compares `>=` instead of `>` | `security/noise.rs` | `test_an_oversized_handshake_message_is_refused` | KILLED |
| 06 | The Noise payload cap compares `>=` instead of `>` | `security/noise.rs` | `test_a_payload_near_the_maximum_size_is_an_error`, `test_the_largest_control_message_fits_the_encrypted_transport` | KILLED |
| 07 | `MAX_CONTROL_MESSAGE_BYTES = 64 * 1024` (drifts from the Noise cap) | `services/remote/mod.rs` | `test_the_largest_control_message_fits_the_encrypted_transport` | KILLED |
| 08 | The control size cap compares `>=` instead of `>` | `protocol/framing.rs` | `test_a_control_message_at_the_cap_survives_the_link`, `test_the_wire_vocabulary_is_stable` | KILLED |
| 09 | `MissingTypeTag` is classified `Fatal` | `protocol/errors.rs` | `test_every_protocol_error_variant_is_pinned_to_its_spec_value`, `test_the_link_skips_an_unknown_message_type_and_keeps_delivering`, `test_a_survivable_violation_on_the_audio_channel_does_not_end_the_receive`, `test_the_loopback_agrees_with_the_classification_for_every_variant`, `test_the_loopback_link_passes_every_conformance_case` | KILLED |
| 10 | `MalformedJson` is classified `Skippable` | `protocol/errors.rs` | `test_a_message_mutated_in_transit_is_rejected`, `test_every_protocol_error_variant_is_pinned_to_its_spec_value`, `test_the_loopback_agrees_with_the_classification_for_every_variant` | KILLED |
| 11 | `UnknownPayloadTag` is classified `Skippable` | `protocol/errors.rs` | `test_a_protocol_violation_closes_the_link_with_its_cause`, `test_every_protocol_error_variant_is_pinned_to_its_spec_value`, `test_the_loopback_agrees_with_the_classification_for_every_variant`, `test_the_loopback_link_passes_every_conformance_case` | KILLED |
| 12 | The frame's sequence number is written little-endian | `protocol/framing.rs` | `test_the_audio_frame_layout_matches_the_spec_byte_for_byte`, `test_the_audio_payload_tag_discriminates_the_two_audio_shapes`, `test_a_control_message_and_an_audio_frame_arrive_intact`, `test_the_loopback_link_passes_every_conformance_case` | KILLED |
| 13 | The PCM16 samples are read big-endian | `protocol/framing.rs` | the same four as #12 | KILLED |
| 14 | The two audio payload tags are swapped on encode | `protocol/framing.rs` | `test_the_audio_payload_tag_discriminates_the_two_audio_shapes`, `test_a_control_message_and_an_audio_frame_arrive_intact`, `test_the_loopback_link_passes_every_conformance_case` | KILLED |
| 15 | The transport treats a skippable violation as fatal | `link/loopback.rs` | `test_the_link_skips_an_unknown_message_type_and_keeps_delivering`, `test_the_loopback_agrees_with_the_classification_for_every_variant`, `test_the_loopback_link_passes_every_conformance_case` | KILLED |
| 16 | **An observed peer protocol close is flattened into `Local`** | `link/loopback.rs` | **SURVIVED the pre-existing suite**; killed by `test_a_peer_protocol_violation_is_observed_with_its_cause` | KILLED (after the new test) |
| 17 | The receive-side control size cap is not applied | `link/loopback.rs` | `test_a_protocol_violation_closes_the_link_with_its_cause` | KILLED |
| 18 | The pairing token is not marked consumed | `security/pairing.rs` | `test_a_token_is_single_use_and_expires` | KILLED |
| 19 | The token lifetime boundary compares `>=` instead of `>` | `security/pairing.rs` | `test_a_token_at_its_lifetime_boundary_is_still_accepted` | KILLED |
| 20 | The per-source attempt limit compares `>` instead of `>=` | `security/pairing.rs` | `test_the_per_source_attempt_limit_and_the_per_source_invalidation` | KILLED |
| 21 | The per-source invalidation threshold compares `>` instead of `>=` | `security/pairing.rs` | `test_the_per_source_attempt_limit_and_the_per_source_invalidation` | KILLED |
| 22 | The claimed device id is not bound to the handshake key | `security/pairing.rs` | `test_a_claimed_identity_must_be_the_handshake_key` | KILLED |
| 23 | The device-id domain string changes | `security/identity.rs` | `test_the_device_id_matches_the_golden_vector` | KILLED |
| 24 | The trust store ignores the role contradiction | `security/trust.rs` | `test_the_trust_store_refuses_every_untrustworthy_peer` | KILLED |
| 25 | The trust store ignores the key/id binding | `security/trust.rs` | `test_the_trust_store_refuses_every_untrustworthy_peer` | KILLED |

---

## 2. The two survivors — the holes this pass exposed

These are the holes the suite had *before* this pass, and they were exactly
the two "landed fixes" that were not really covered.

### Survivor 1 — mutant 04: the Noise handshake message size cap can be deleted and the suite stays green

The pre-existing test handed the driver 257 zero bytes and asserted the
result was an error. Without the cap, `snow` refuses the nonsense bytes
anyway, so the assertion still held. **The 256-byte cap — the entire fix for
the pre-authentication allocation hole — was untested.**

*Remediation:* `test_a_valid_handshake_message_over_the_cap_is_still_refused`
hands the driver a message the **pattern produced** and that is only too
large, so only the cap can refuse it, and then asserts the responder's
handshake state is untouched.

### Survivor 2 — mutant 16: an observed peer protocol violation can be reported as a plain `Local` close and the suite stays green

`observed_peer_reason` has three arms; only the `Local`/`Peer` mapping was
pinned. A peer that broke the wire contract was therefore reported to the
observing side as a routine close — precisely the confusion the close-reason
fix set out to end.

*Remediation:* `test_a_peer_protocol_violation_is_observed_with_its_cause`.

---

## 3. Production defects this pass found

The mutation pass is a review technique, and it found three real defects that
the behavioural suite did not. All three are fixed; the regression test for
each is named.

| # | Defect | Fix | Regression test |
| --- | --- | --- | --- |
| **D-1** | `prune` dropped the expired token before `validate_token` could classify it, so `PairingRefusal::Expired` was **unreachable** and the user was told their QR code was never issued rather than stale. | An expired token is kept for one further lifetime, so the expiry classification stays reachable and the store still prunes. | `test_a_token_is_single_use_and_expires` (the refusal is now pinned to `Expired`) |
| **D-2** | `NoiseHandshake::write_message` had no size cap, so the driver could emit a handshake message a conforming peer refuses. | The handshake payload is capped at `NOISE_MAX_HANDSHAKE_PAYLOAD_BYTES`, so the driver cannot produce a message over the cap a peer enforces. | `test_an_oversized_handshake_message_is_refused` |
| **D-3** | The 4 KB pre-authentication cap (remote-control spec §7.1) had **zero production callers**: the `ControlMessagePhase::PreAuthentication` branch existed only for a test that called the checker directly. | The phase enum was deleted and the cap is now enforced at the point where the pre-authentication message enters — `PairingRequest::from_wire`, the one message that travels before the Noise handshake completes. | `test_an_oversized_pre_authentication_request_is_refused` |

---

## 4. What is still uncovered, and why

The remote-control suite covers the code that exists. It does **not** cover
the feature, because most of the feature is not written: `session/`,
`control/`, `discovery/`, `audio/` and `testing/` are still empty module
stubs for the later waves. **14 of the spec's 22 `VERIFY-RC-*` behaviours
have no test**, because the production code they test does not exist.

Each of those behaviours is listed below with the exact production seam it
needs. They are not written as fakes: a test that reimplements the session
manager inside the test file passes whether or not the real session manager
works. **This table is the blocker list for the next wave — it is a seam
list, not a test list.**

| # | Behaviour | Missing production seam |
| --- | --- | --- |
| M-2 | The 4 KB pre-authentication cap enforced by the transport | The link has no lifecycle phase. The seam: a phase on `RemoteLink`/`LoopbackLink` that flips when the Noise handshake completes, and the transport checking the message against the cap that applies to that phase. *(The application-message side of this cap is now enforced at the pairing entry point — see D-3.)* |
| M-3 | The host refuses a controller while a local session is active | `session/` has no session manager and no local-session state; nothing emits `ControlMessage::LinkRefused`. Seam: `session/manager.rs` plus a `LinkRefused` emission point. |
| M-4 | The host refuses when `remote.accepts_controllers` is false | The setting and its mutation router exist. The connect half does not: no admission point reads it. Seam: the connector that consults `settings.remote.accepts_controllers` and emits `LinkRefused { reason: NotAccepting }`. |
| M-5 | A role contradiction produces `RoleNotPermitted` and a close | `TrustStore::lookup` already returns `Refused(RoleMismatch)` and is tested. No production code ever *emits* `ControlMessage::RoleNotPermitted`. Seam: the session layer that consumes `TrustDecision` and sends the refusal before closing. |
| **M-6** | **Every local write is blocked while a remote session is connected** | **No `ControlOrigin` parameter and no `RemoteLinkState` exist. Seam: both concepts, plus the registry-driven block in the command gateway. This is the highest-impact gap in the audit — two writers into one pipeline is currently possible.** |
| M-7 | The audio socket requires a valid session token | **There is no session token anywhere in the codebase.** The concept must be specified before it can be tested. |
| M-8 | The heartbeat declares a dead controller | No heartbeat implementation; the four `HEARTBEAT_*` constants have zero production callers. Seam: `session/manager.rs`'s heartbeat loop. |
| M-9 | The snapshot carries the recent turns | No session, no snapshot builder. The `AttachSnapshot` *wire type* is tested; the *content* has no producer. |
| M-10 | The routing table is complete against the command registry | `control/routing.rs` does not exist. The enumeration must be derived from the production registry, so it cannot be written first. |
| M-11 | A `get_settings` response to a controller is redacted | No remote redaction path exists (the desktop path is covered and is explicitly *not* VERIFY-RC-08). Seam: a redacting settings response selected by `ControlOrigin`. |
| M-12 | A stale-epoch audio frame is discarded | The `Flush`/`epoch` wire fields exist and round-trip; the *filter* does not. Seam: an epoch tracker in the audio path that advances on `Flush` and drops frames below the current epoch. |
| M-13 | The state is push-based; exactly one `remote_state_changed` event | No `RemoteLinkState` and no event exist. |
| M-14 | A host-initiated disconnect keeps the pairing | The revocation half is covered. The "keeps the pairing" half needs a disconnect path — no session, no disconnect operation. |
| M-15 | The audio channel is encrypted and bound to the control channel's static key | `NoiseTransport` encrypt/decrypt is tested. The *binding* needs two channels that share a handshake — no such thing exists. |
| M-16 | A controller's pipeline events reach the controller and not the desktop | No event policy. The required assertion is a **negative** one (the desktop channel stays silent) and must wait deterministically, not on a timeout. |
| M-17 | Telemetry is rate-limited for the controller and not for the desktop | No rate limiter, and no deterministic clock in the remote path. |
| M-18 | The session is preserved across a controller disconnect | No session state machine. |
| M-19 | A peer on another protocol version is rejected before any other message | The prologue-level rejection exists and is tested. The `Hello`-level `VersionMismatch` message is never emitted by any production code. Seam: the session layer that reads `Hello.version` and replies. |
| M-20 | No boolean may shadow `RemoteLinkState` | There is no `RemoteLinkState` yet, so there is nothing to shadow. This must be written **with** the session module. |
