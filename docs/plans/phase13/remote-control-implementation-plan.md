# Vox Remote Control — Implementation Plan (Orchestration Loop)

**Goal:** a phone pairs with a Vox laptop host over LAN, becomes the host pipeline's microphone, speaker and UI surface, and the whole path is proven by automated tests against **real models** over a **real network**. The frontend (TypeScript / React) is out of scope and is owned by the user.

**SSOT:** [`docs/specs/remote-control-spec.md`](../../specs/remote-control-spec.md) · **Decisions:** [`remote-control-adr.md`](remote-control-adr.md) · **Draft tickets:** [`remote control-plan.md`](remote%20control-plan.md)

---

## 1. Rules of the loop

1. **One wave at a time.** No two waves overlap.
2. **One agent implements a wave and writes its own tests.** A second agent in the same wave would double the iteration cost (ADR-RC-13).
3. **At the wave boundary, an independent gate runs:** a test-engineer pass (evidence) followed by a QA-engineer pass (adversarial review + verdict).
4. **A failed gate produces scoped fix findings**, not a re-run. The fixes re-enter the same path.
5. **On an approved gate: commit, push, then STOP.** The next wave does not begin until the user inspects the pushed code and approves it.
6. **The user is the final gate.** Every wave ends at a human inspection point.
7. **No blockers are papered over.** A blocker is reported, not stubbed, mocked, or skipped.
8. **Two failed attempts at the same fix without a new hypothesis → stop and escalate** to the user.

### 1.1 No ticket files

There is no per-ticket backlog. A wave is a short paragraph in the wave table
below, and the wave's acceptance criteria are the spec clauses it names. A
ticket per clause produced forty-four files that duplicated the spec, and the
duplication is where the drift came from.

**The spec is the backlog.** `docs/specs/remote-control-spec.md` names every
behaviour and every `VERIFY-RC-*` criterion. When the two disagree, the spec
wins and the code is wrong.

**The mutation log is the evidence.** One file per wave,
`docs/plans/phase13/remote-control-mutation-log.md`, records every mutant that
was seeded against the wave's code and what happened to it. A test that has
never been shown to go red is not evidence.

---

## 2. Sub-agent roster (persistent across the entire loop)

| Role | Agent | Owns | Produces | Gates? |
| :--- | :--- | :--- | :--- | :--- |
| **Implementer** | `backend-engineer` | Production code **and** the wave's tests | The diff, the wave's green test run | No |
| **Verifier** | `test-engineer` | Independent evidence | A structured evidence report: raw test output, mutation results, coverage of the acceptance criteria | No |
| **Auditor** | `qa-engineer` | The verdict | `APPROVED` / `REJECTED` / `INSUFFICIENT EVIDENCE`, with exact reproduction pointers | **Yes** |

Roles are re-invoked, not respawned. Prior findings, prior rejections and prior context carry forward. The Auditor sees every previous verdict on the wave.

**Agent invocation order per wave:** Implementer → Verifier → Auditor → (fixes → Verifier → Auditor)* → commit → push → stop.

---

## 3. Wave plan

Each wave is one commit on `ft/remote-control`, pushed to `origin/ft/remote-control`, then gated by the user.

| Wave | Delivers | Depends on |
| :--- | :--- | :--- |
| **W0** Foundation | ADR + spec amendments (done, this commit), `AGENTS.md` §7, `snow`/`mdns-sd`/`qrcode` deps, `remote` settings domain, `paired_devices` table, `secret_keys()` azure fix, `supertonic` default TTS | — |
| **W1** Protocol | `services/remote/protocol/` (messages, framing, capabilities, errors), `link/` traits, `LoopbackLink`, the conformance suite; plus the two W0 carry-overs (ticket 34, the cached Devanagari engine; ticket 35, the voice catalogue through the provider enum) | W0 |
| **W2a** Security | `security/identity.rs` (the device key and the `DeviceId` derivation), `security/key_store.rs`, `security/noise.rs` (the Noise_XX handshake and encrypted transport), `security/pairing.rs` + `security/trust.rs`; plus the link-contract corrections (a close reason that carries its cause, and the fatal-or-skippable classification moved into `protocol/`) and the Emma voice-collision fix | W1 |
| **W2b** Transport | `link/ws.rs` + `link/session.rs` — the real TCP → WebSocket → Noise transport with two sockets per link, the `SessionToken` message type, and the two transport-boundary obligations (the protocol MUSTs enforced at the receive path, and the conformance suite covering the log-a-warning clause) | W2a |
| **W3** Session & control plane | `session/` (role, state, manager, heartbeat, snapshot), `control/` (routing, args, gateway, redaction), the `ControlOrigin` guard rule, host admission rules | W2 |
| **W4** Event plane | `control/event_policy.rs`, the composite `IpcEmitter`, ~20 emit-site conversions, `remote_state_changed`, rate limits | W3 |
| **W5** Audio | `audio/` (capture, sink, pacing), the extracted 16 kHz normaliser, AEC bypass, `SessionOrigin` capture arbitration, dictation gating | W3 |
| **W6** Proof | `testing/reference_client.rs`, the end-to-end turn suite (real STT → real LLM → real TTS over real TCP+Noise), the negative-path suite | W4, W5 |
| **W7** Phone client | Controller-side session, audio uplink, playout + jitter buffer, mode switch, the controller IPC surface | W6 |
| **W8** Discovery, hardening, handoff | `discovery/` (hints, mDNS, candidates), the security hardening pass, spec sync, delete `AGENTS.md` §7, `AGENTS.md` §5 | W7 |

**W6 is the milestone.** Until it passes, the feature is unproven: no turn has crossed a real network with real models.

### 3.1 What each wave must deliver

Each wave is one commit. The acceptance criteria are the spec clauses the
wave names, and the `VERIFY-RC-*` numbers it closes.

| Wave | Delivers | Spec clauses closed |
| :--- | :--- | :--- |
| **W0** Foundation | The ADR and the spec amendments, `AGENTS.md` §7, the `snow` / `mdns-sd` / `qrcode` dependencies, the `remote` settings domain, the `paired_devices` table, the `secret_keys()` Azure fix, `supertonic` as the default TTS | — |
| **W1** Protocol | `services/remote/protocol/` (messages, framing, capabilities, errors), the `RemoteLink` trait, `LoopbackLink`, the link conformance suite; plus the two W0 carry-overs (the cached Devanagari engine and the voice catalogue through the provider enum) | VERIFY-RC-01, VERIFY-RC-15 |
| **W2a** Security | `security/identity.rs` (the device key and the `DeviceId` derivation), `security/key_store.rs`, `security/noise.rs` (the Noise_XX handshake and encrypted transport), `security/pairing.rs`, `security/trust.rs`; plus the link-contract corrections (a close reason that carries its cause, and the fatal-or-skippable classification moved into `protocol/`) | VERIFY-RC-01, VERIFY-RC-02, VERIFY-RC-03, VERIFY-RC-06 |
| **W2b** Transport | `link/ws.rs` and `link/session.rs` — the real TCP → WebSocket → Noise transport with two sockets per link, the `SessionToken` message type, and the two transport-boundary obligations (the protocol MUSTs enforced at the receive path, and the conformance suite covering the log-a-warning clause) | VERIFY-RC-13, VERIFY-RC-21 |
| **W3** Session & control plane | `session/` (role, state, manager, heartbeat, snapshot), `control/` (routing, args, gateway, redaction), the `ControlOrigin` guard rule, host admission rules | VERIFY-RC-05, VERIFY-RC-07, VERIFY-RC-08, VERIFY-RC-16, VERIFY-RC-17, VERIFY-RC-18, VERIFY-RC-22 |
| **W4** Event plane | `control/event_policy.rs`, the composite `IpcEmitter`, ~20 emit-site conversions, `remote_state_changed`, rate limits | VERIFY-RC-09, VERIFY-RC-10 |
| **W5** Audio | `audio/` (capture, sink, pacing), the extracted 16 kHz normaliser, AEC bypass, `SessionOrigin` capture arbitration, dictation gating | VERIFY-RC-11, VERIFY-RC-12 |
| **W6** Proof | `testing/reference_client.rs`, the end-to-end turn suite (real STT → real LLM → real TTS over real TCP + Noise), the negative-path suite | VERIFY-RC-19, VERIFY-RC-20 |
| **W7** Phone client | Controller-side session, audio uplink, playout + jitter buffer, mode switch, the controller IPC surface | — |
| **W8** Discovery, hardening, handoff | `discovery/` (hints, mDNS, candidates), the security hardening pass, spec sync, delete `AGENTS.md` §7, `AGENTS.md` §5 | — |

---

## 4. Gates

### 4.1 Sub-agent gate (end of every wave)

- **Gate:** the Auditor issues `APPROVED` only when (a) the Verifier's evidence covers every acceptance criterion of the wave, (b) the diff matches the spec clauses it claims to implement, (c) the compile gate is 0 warnings on both targets, and (d) the full test suite is green.
- **On `REJECTED`:** the Auditor's findings become scoped fixes. The Implementer fixes them. The Verifier and Auditor re-run **on the fixes only**.
- **On `INSUFFICIENT EVIDENCE`:** the Auditor states exactly which evidence is missing. The Verifier produces it. No code is written.
- **On `APPROVED`:** commit, push, stop for the user.

### 4.2 HITL gate (end of every wave)

**The user inspects the pushed code and approves before the next wave begins.** No wave starts on an internal approval alone.

### 4.3 Scheduled check-ins

Every 20 minutes during an active wave, an adversarial prompt is injected:

- Has every issue actually been looked at, or assumed fixed?
- Is the current phase still detailed enough, or is it coasting on a vague scope? If vague — stop and force `/modify-plan` before continuing.
- Is progress real, or is the loop repeating the same failed attempt without a new hypothesis?

Check-ins never approve. Only the sub-agent gate and the user's HITL gate do.

---

## 5. Commit and push policy

- One commit per wave. The commit message states the wave and the verification command with its result.
- The commit is pushed to `origin/ft/remote-control` **before** the user inspects it.
- A wave's commit must leave the repository green: `./scripts/verify-targets.sh` at 0 warnings on both targets, and the full `cargo nextest` suite passing.
- Linear history. No merge commits.
- **Before the feature ships, the branch history may be rewritten.** The feature is not released, so a squashed history is worth more than a granular one. After the first release, the history is append-only.

### 5.1 Environment

```bash
export ANDROID_HOME=/opt/android-sdk
export NDK_HOME=/opt/android-sdk/ndk/27.2.12479018
export PATH="/root/.cargo/bin:$PATH"      # cargo-nextest
cd app/src-tauri
```

> **Known upstream issue (not fixed by this work):** `scripts/android-env.sh` sets `NDK_HOME` to a bare version directory name instead of a full path, so its auto-discovery branch only works when `NDK_HOME` is already exported. Exporting `NDK_HOME` explicitly, as above, works around it. The Android clippy gate has been confirmed to pass with NDK 27.2.12479018.

### 5.2 Commands

```bash
# compile gate (both targets, 0 warnings)
cd app/src-tauri && ./scripts/verify-targets.sh

# test suite
cd app/src-tauri && RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1 --no-fail-fast

# single test file
cd app/src-tauri && cargo nextest run --test <name> --release --nocapture --test-threads=1
```

### 5.3 Test models

Real model weights are provisioned at `~/.vox/models` from the public `addyo07/vox-models` Hugging Face repository:

| Path | Contents |
| :--- | :--- |
| `stt/nemotron-3.5/` | Nemotron STT (encoder, decoder, joiner, tokens) |
| `llm/qwen/` | qwen-3.5-0.8b q4_k_m GGUF |
| `tts/supertonic-3/` | Supertonic TTS |
| `tts/kokoro/` | Kokoro TTS |
| `embedding/minilm-l12-v2/` | MiniLM embeddings |

Re-provision with:

```bash
hf download addyo07/vox-models --include "stt/nemotron-3.5/*" --include "llm/qwen/*" \
  --include "tts/supertonic-3/*" --include "embedding/minilm-l12-v2/*" \
  --local-dir ~/.vox/models
```

**No fake or mock model providers.** The end-to-end suite uses the real models.

---

## 6. Risks

| Risk | Mitigation |
| :--- | :--- |
| The composite emitter touches ~20 emit sites across `pipeline/`, `services/`, `monitoring/` | Its own wave (W4) and its own gate. Two commits inside the wave: trait first, then call sites. |
| `mdns-sd` may not build for `aarch64-linux-android` | W0 verifies it. If it fails, discovery degrades to QR-hint plus manual entry and mDNS moves to a later ticket. |
| The idle monitor auto-pauses after 7 minutes and offloads models after 5 | Both are suppressed while a controller is attached (W3). |
| `on_end` tears the engine down when dictation is `Idle` | Guarded in W5. |
| The phone's uplink must not stall the VAD actor | In-order, gap-padded 256-sample delivery (W5). |
| A new inbound socket is a new attack surface | Noise mutual auth, pairing, IP rate limits, size caps, handshake deadline (W2, W8). |
