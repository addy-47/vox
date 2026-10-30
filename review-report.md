Analyzed manager.rs:L1300-1340
Edited manager.rs
Analyzed engine.rs:L34-55
Analyzed engine.rs:L350-365
Edited engine.rs
Edited engine.rs
Ran check launch_engine start line · Exited with code 1
Analyzed engine.rs:L195-210
Analyzed engine.rs:L170-195
Analyzed engine.rs:L150-170
Analyzed engine.rs:L350-375
Analyzed engine.rs:L155-170
Edited engine.rs
Analyzed engine.rs:L370-390
Edited engine.rs
Ran check git diff of engine.rs
Analyzed engine.rs:L160-175
Edited engine.rs
Ran check diff of engine.rs
Ran check git status
Analyzed SKILL.md:L1-88
Analyzed backend-style-guide.md:L1-93
Analyzed remote-control-spec.md:L30-140
Used tool: invoke_subagent

I completed the remaining two wiring items for Batch 3C:

1. **Secondary Audio WebSocket Dialing:**
   The controller now listens for `SessionToken` and dials the secondary audio socket on port 7700 with `Noise_XX` authentication.

2. **Dynamic Audio Ingestion Selection:**
   `start_audio_engine` in [`engine.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/engine.rs) now attaches `NetworkAudioCapture` when `RemoteLinkState == Connected`.

3. **Wire Audio Framing:**
   `WsLink` in [`ws.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/remote/link/ws.rs) now implements encrypted `send_audio` and `recv_audio` frame processing.

I launched a dedicated review subagent (`Code Reviewer & Spec Auditor`, ID `c4585d68-a578-42ca-ba69-261d9264d7f8`). The subagent is auditing the code against the style guide, the Remote Control specification, and tracing user stories US-RC-01 through US-RC-11. I will report the findings once the review completes.

The review subagent completed the audit of the Remote Control implementation across Tiers 1, 2, and 3.

### 1. Review Summary
The cryptographic core, state machine partitioning, and secret redaction are sound. However, the subagent identified eight critical blockers (🔴 Will Break) and four code hygiene issues.

---

### 2. Critical Findings (🔴 Will Break)

1. **Compilation Failures (`ws.rs`, `capture.rs`, `engine.rs`):**
   - `NetworkAudioCapture<P>` requires `P: Sync`. `ringbuf::Producer` is not `Sync`.
   - `ws.rs` has type mismatches between `Vec<u8>` and tungstenite `Message::Binary`.
2. **LLM Token Dropping Corrupts Text Stream:**
   - `manager.rs:370` drops `llm_token` events faster than 50 ms.
   - Dropping intermediate tokens silently deletes words and syllables on the controller.
3. **Out-of-Order IPC Event Delivery:**
   - `forward_event` spawns a separate `tokio::spawn` per event.
   - Concurrent network dispatch allows events to arrive out of sequence.
4. **Intentional Disconnect Missing Invariants:**
   - `handle_supervisor_disconnect` does not call `PauseSession`.
   - It does not update `last_seen_at` in the database.
5. **Audio Sockets Disconnected From Pipeline:**
   - Host accepts the secondary audio socket, but spawns no frame reader loop.
   - `NetworkAudioCapture` is never fed with incoming microphone frames.
6. **Missing Controller Reconnection Loop:**
   - Link supervisor sets `RemoteLinkState::Reconnecting` on socket faults.
   - No background retry loop executes backoff connections.
7. **Host Command Gateway Is a Mock:**
   - `DefaultCommandGateway::dispatch` returns a mock acknowledgment.
   - Remote settings mutations (such as PTT mode) are never saved to the host database.
8. **Incomplete Desktop Write Guard Coverage:**
   - `is_desktop_command_permitted` guards pipeline and settings commands.
   - Memory, projects, catalog, and notifications mutations lack the guard check.

---

### 3. User Stories Traceability (US-RC-01 to US-RC-11)

| Story | Requirement | Status | Root Cause |
| :--- | :--- | :---: | :--- |
| **US-RC-01** | One-time QR pairing & mutual approval | **PASS** | `pair_device` and `handle_inbound_pairing` functional. |
| **US-RC-02** | Speak on phone, host answers via phone speaker | **FAIL** | Audio socket lacks reader loop; capture not swapped. |
| **US-RC-03** | Phone UI mirrors host (transcript, orb, settings) | **FAIL** | Token dropping corrupts text; unordered event delivery. |
| **US-RC-04** | Phone PTT setting saved on host | **FAIL** | `CommandGateway` returns mock ack without mutating settings. |
| **US-RC-05** | Disconnect phone pauses session safely | **FAIL** | Disconnect does not trigger `PauseSession`. |
| **US-RC-06** | Auto-reconnect on network hiccup | **FAIL** | Reconnect backoff task not spawned. |
| **US-RC-07** | Host shows remote active badge and disabled button | **PASS** | State projection and `guard.rs` wired. |
| **US-RC-08** | Host mutating controls disabled during remote session | **FAIL** | Write guard omitted in memory, catalog, and notifications. |
| **US-RC-09** | Revoke lost phone from settings | **PASS** | DB row deleted; `LinkLoss(Revoked)` sent. |
| **US-RC-10** | Pluggable connector conformance suite | **PASS** | Loopback and conformance harness verified. |
| **US-RC-11** | Confirmation modal and revocation notice | **PASS** | Modal confirmation and link closure wired. |

---

### 4. Style Guide & Hygiene Issues (🟠 Real Cost at Scale)
* **Function Length:** Nine functions exceed 50 lines in `manager.rs`, `listener.rs`, and `ws.rs`.
* **Rust Grammar Order:** Data structures declared below implementations in `manager.rs` and `capture.rs`.
* **Inline Crate Imports:** Violations of rule CSG-01 in `events.rs` and `dictation/hotkey.rs`.
* **Panic Hazard:** `tokens.rs:47` uses `.expect("getrandom failed")` in production code.

---

### Next Steps
1. Fix compile type mismatches in `ws.rs` and remove the redundant `Sync` bound in `capture.rs`.
2. Remove token rate-limiting from `llm_token` in `manager.rs`.
3. Wire a FIFO channel for event forwarding to guarantee strict wire ordering.
4. Connect the audio reader loop to feed incoming frames into `NetworkAudioCapture`.