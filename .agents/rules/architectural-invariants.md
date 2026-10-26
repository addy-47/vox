---
trigger: always_on
---

---
trigger: always_on
description: Cross-stack architectural invariants, state machine laws, hardware tiers, and authoritative specification index for Vox. Applies to all architect, backend, frontend, and QA roles.
---

# Vox — System Architecture Invariants

These invariants govern the entire Vox runtime across Rust (`app/src-tauri/`), React (`app/src/`), sidecars (`sidecars/`), and specifications (`docs/specs/`). No spec, ticket, or code change may violate them.

---

## 1. Core System Axioms

1. **Zero Backward Compatibility (ZBC):** Never introduce compatibility shims, legacy feature flags, dual code paths, or deprecated event aliases. When an interface, state machine, or wire contract is flawed, replace it cleanly across all consumers in a single atomic sweep.
2. **Spec-First Single Source of Truth (SSOT):** Every behavioral, state machine, wire protocol, database schema, or IPC contract change must be locked in `docs/specs/` before implementation begins. Code conforms to specs; specs never drift into append-only changelogs.
3. **Graceful Hardware Tiering:** Vox operates across a 5-tier capability matrix (Tier 1A: 8GB CPU-only local; Tier 1B: 8GB+ discrete GPU local; Tier 2A: Hybrid Remote LLM; Tier 2B [Default]: Hybrid Cloud LLM; Tier 3: Realtime WebSocket S2S). Features must degrade cleanly to Tier 1A without hard-failing on missing optional models.

---

## 2. Lifecycle & State Machine Laws

1. **Decoupled Operational Lifecycles:** Never entangle independent domain lifecycles into a single state enum or command.
   - *Example (Remote Control):* Tier 1 Trust/Pairing (`PairingState` + `paired_devices` DB) $\neq$ Tier 2 Transport Liveness (`RemoteLinkState`) $\neq$ Tier 3 Pipeline Session (`InteractionState`). Completing Tier 1 must never auto-start Tier 2; dropping Tier 2 must never mutate Tier 1 storage.
   - *Example (Persistence vs. Routing):* Ephemeral `turn_id` (cancellation/routing) $\neq$ persistent `session_id` (conversation ledger).
2. **Orthogonal Zero-State Invariant (Bijective State Space):** No state enum may use a vague catch-all variant (`Disconnected`, `Idle`, `None`) to represent multiple distinct domain conditions. If an entity can be *Unconfigured/Unpaired* vs. *Configured/Paired but Inactive*, they MUST be explicit, separate variants (`Unpaired` vs. `Paired`).
3. **Zero Shadow Booleans:** Never introduce boolean flags or getters (`is_connected`, `controller_attached`, `is_recording`, `is_llm_loaded`) that shadow or duplicate canonical state enums or `Option<Engine>` residency. Every consumer queries the canonical enum directly.
4. **Unidirectional State Projection (Backend Owns Truth):**
   - The Rust backend is the sole owner of state machines. It projects state outward to UI surfaces via canonical IPC events (`state_changed`, `remote_state_changed`, `remote_pairing_state_changed`).
   - The React frontend may hydrate initial state once on mount (`get_link_state`, `get_settings`), then reacts strictly to pushed events.
   - **Frontend Anti-Pattern Ban:** The UI must NEVER join a database query (`pairedDevices.length > 0`) with a runtime transport state to guess what screen or label to render. If the UI needs to know a state, the canonical state payload must carry it.
5. **Discrete State Verbs (No Toggle Commands):** Never expose `toggle_*` functions or IPC commands. Always use explicit, idempotent state-transition verbs (`engage_session` / `disengage_session`, `connect_host` / `disconnect_host`).

---

## 3. Boundary & Concurrency Laws

1. **Fixed Pipeline Topology:** `Audio Capture → AEC → VAD → STT → LLM (Harness) → TTS → Playback`. Stages communicate via bounded channels and atomic cancellation tokens. No stage may synchronously block an adjacent stage.
2. **Sacred Audio Hot-Path:** Zero heap allocations, zero mutex lock acquisitions, zero regex compilations, and zero disk/DB I/O inside active audio callbacks or token-streaming hot loops.
3. **Thread & Runtime Placement:**
   - **Dedicated OS Threads:** Blocking C++/ONNX/GGUF inference workers (VAD, STT, LLM, TTS). C++ loops must check `Arc<AtomicBool>` cancellation tokens on every token/chunk.
   - **Single-Writer Event Router (`vox-router`):** Serializes all `InteractionState` transitions (`RULE-EVT-01`). Never perform blocking I/O or inference on the router thread.
   - **Tokio Async Tasks:** Network I/O, WebSocket Link Supervisors, background process management, Turso DB persistence workers, and Tauri IPC dispatch.
4. **Four-Boundary Distinction:** Designs and specs must explicitly distinguish:
   - **Boundary 1 (UI $\to$ Local Backend):** Tauri IPC Commands (`invoke` strictly inside `app/src/services/`).
   - **Boundary 2 (Local Backend $\to$ UI):** Tauri IPC Events (`emit` / `listen` via `eventsService.ts`).
   - **Boundary 3 (Backend $\leftrightarrow$ Peer/External):** Network Wire Protocol (Noise_XX WebSocket frames, HTTP provider payloads, sidecar stdio/IPC).
   - **Boundary 4 (Backend $\leftrightarrow$ Durable Storage):** Turso SQLite MVCC (`paired_devices`, `sessions`, `turns`, `session_tool_calls`) and `~/.vox/` config/cache files.

---

## 4. Authoritative Specification Ledger (`docs/specs/`)

| Domain | Authoritative Spec |
| :--- | :--- |
| Pipeline Lifecycle & Event Routing | `docs/specs/events-spec.md` |
| LLM Agent Harness & Step Loop | `docs/specs/harness-spec.md` & `docs/specs/tools-specs/` |
| Tauri IPC Commands & Event Payloads | `docs/specs/ipc-spec.md` |
| Remote Control & Thin-Client Protocol | `docs/specs/remote-control-spec.md` |
| Turso Database Schemas & Migrations | `docs/specs/db-spec.md` |
| Filesystem Layout & Config Split | `docs/specs/storage-spec.md` |
| Working & Personal Memory | `docs/specs/memory-spec.md` |
| Audio/Mic Ownership & Teardown | `docs/specs/ownership-spec.md` |
| Provider Wire Manifest & Catalog | `docs/specs/providers-spec.md` |
| Dictation & Keystroke Injection | `docs/specs/dictation-spec.md` |
| Notifications & Toast Routing | `docs/specs/notifications-spec.md` |
| Sidecar Process Management | `docs/specs/sidecar-spec.md` |
| UI Design System ("Liquid Space") | `docs/specs/design-spec.md` |