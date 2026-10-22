# AGENTS.md — Vox Workspace Rules

---

## 1. MANDATORY: Work Log Hook

> 🛑 **POST-TASK HOOK: Record Work in `work_log.md`**
> After every completed task, record your entry in `docs/plans/<current_phase>/work_log.md`.
> **One bullet per work seam:** if your work extends an existing task or lane, update that bullet instead of appending a new one. Do not create new bullets for the same task.
> [Skip for tasks with no lasting accountability: one-off lookups, read-only investigations, questions answered without touching files.]

---

## 2. Key Paths

Only paths with non-obvious rules that agents cannot infer from the codebase index.

| Path | Rules |
| ---- | ----- |
| `app/src-tauri/src/` | No test logic. No benchmarks. Rust grammar order: `backend-style-guide.md §2.1`. |
| `app/src-tauri/tests/` | Named `<feature>_test.rs`. Public `vox_lib` API only. |
| `app/src-tauri/scripts/verify-targets.sh` | **The compile gate.** Always use this — never bare `cargo check` on a cross target. |
| `.agents/rules/` | Read the relevant rule file before acting in any named role. `android-style-guide.md` is mandatory for any change touching `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, `Cargo.toml` target tables, or any `cfg(`. |
| `docs/specs/` | Authoritative behavioral contracts — SSOT. Never contradict. |
| `docs/plans/phase13/` | Active phase. Plans and `work_log.md` live here. |
| `temp/.env` / `temp/server.txt` | API keys and remote GPU server creds. Not versioned. |
| `~/.vox/vox.db` | Local Turso DB. Inspect via `tursodb ~/.vox/vox.db "<SQL>"`. |

**Remote GPU server:** creds in `temp/server.txt`. **Never kill running server processes.**

---

## 3. Execution & Testing Invariants

0. **Compile Gate:** Always:
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
1. **Sequential + Release:** Run benchmarks, evals, test suites, or any cargo command one at a time. Always `--release`.
2. **Test runner (`cargo-nextest`):**
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ Full suite baseline: ~45.5s (excluding cold compile). Set a timeout or cron — never leave a run unmonitored.
3. **External-dependency tests (`#[ignore]`):** Run only with explicit user approval:
   ```bash
   cargo nextest run -- --ignored
   ```

---

## 4. Invariants & Gates

### 4.1 Architectural Invariants (Non-Negotiable)

**DATABASE ENGINE (NON-NEGOTIABLE):** WE DO NOT USE SQLITE. Vox exclusively uses the **Turso Database Engine** (`turso` crate: pure-Rust, async-native, MVCC concurrent writers). Never assume SQLite, C toolchains, or rusqlite locking constraints. Reference: `docs/features/turso-database-engine.md`.

**Zero Backward Compatibility (ZBC):** Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers or legacy paths.

**Pre-exploration:** Before any codebase exploration, architecture lookup, graph query, or broad search — read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.

**Local Turso CLI:**
```bash
tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
```

### 4.2 Code Modification Gate

> 🛑 **Before touching code:** Read the corresponding style guide and engineer rule file in `.agents/rules/` for the area you are working on.
>
> **Android gate:** If the change touches `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, `Cargo.toml` target tables, or any `cfg(` — also read [`android-style-guide.md`](file:///home/addy/projects/apps/vox/.agents/rules/android-style-guide.md). Android support is incomplete; confirmed traps there have caused false "done" reports.

---

## 5. Specifications & Spec Alignment (Non-Drift Gate)

All authoritative specifications live in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Reading the relevant spec is mandatory before designing or modifying code.

> 🛑 **Spec Alignment Invariant:** Every agent must adhere strictly to the approved specifications.
> 1. **Divergence / Additions:** If behavior, commands, or schemas diverge from or are absent from the relevant spec — STOP and ask for approval. If approved, update the spec FIRST (following `/create-spec`), then write code. Specs must never quietly drift from code.
> 2. **Legacy Code:** If existing code implements behavior not captured in the spec — confirm with the user before pruning the code or updating the spec.

---

## 6. Current Phase & Work Log

> 📖 **Phase 13 (active):** All completed work and previous agent activity are recorded in [`docs/plans/phase13/work_log.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/work_log.md).
> Read `work_log.md` to understand recent milestones, established architectural patterns, and verified seam implementations.

---

## 7. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.

---

## 8. TEMPORARY — Remote Control Module Map (delete when the feature lands)

> ⏳ Added by the Remote Control feature. **This section is deleted in the final wave (W8).** It is not a permanent rule of the codebase. Full rationale: [`docs/plans/phase13/remote-control-adr.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/remote-control-adr.md). Behavioral contract: [`docs/specs/remote-control-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/remote-control-spec.md).

### 8.1 Module ownership

Applies to `app/src-tauri/src/services/remote/`.

| Module | Owns | Must never contain |
| :--- | :--- | :--- |
| `protocol/` | wire message types, binary framing, capability bitmask, wire error codes | I/O, state, Tauri types |
| `link/` | `RemoteLink` trait, `LoopbackLink`, `WsLink`, link lifecycle | protocol semantics, app state |
| `security/` | device identity, key storage, Noise handshake, pairing, trust store | session state, IPC commands |
| `session/` | `RemoteLinkState`, `SessionRole`, session manager, heartbeat, snapshot | protocol encoding, Tauri commands |
| `control/` | routing table, arg specs, command gateway, redaction, event policy | transport, audio |
| `discovery/` | address hints, mDNS advertisement and browsing, dial candidates | session state |
| `audio/` | network capture, network sink, frame pacing | session state, protocol encoding |
| `testing/` | headless reference client | production logic |

### 8.2 Rules

1. **`protocol/` is pure.** No `tokio`, no `tauri`, no filesystem. It is the single source of truth for the wire contract; both roles and the test suite import it with no I/O.
2. **`RemoteLinkState` (`session/state.rs`) is the single source of truth for remote state.** No `is_connected`, `controller_attached`, `is_remote_connected` or equivalent boolean may shadow it anywhere in the codebase. This mirrors `.agents/rules/backend-style-guide.md` §5.
3. **Host and Controller are roles, not branches.** `session/manager.rs` implements the session once, parameterised by `SessionRole`. Never fork session logic per role.
4. **`control/routing.rs` is a static table.** Every IPC command and every `update_setting` domain/key must appear in it, including the `model_parameters` pseudo-domain. A test asserts completeness.
5. **The desktop-vs-remote distinction is an explicit `ControlOrigin` parameter** on the affected service functions. Never a thread-local, never a global flag.
6. **The backend owns remote state and pushes it.** The frontend may read it once at boot, never to drive it. There is exactly one `remote_state_changed` event.
7. **`ipc/` holds thin Tauri command wrappers only.** Domain logic belongs to the service module that owns the domain.
8. **New modules land in the tree above, not as flat files in `services/remote/`.** If a new capability does not fit one of the eight modules, it needs a new module with a stated ownership, not a ninth file at the root.
