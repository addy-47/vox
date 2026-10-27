# AGENTS.md — Vox Workspace Rules

---

## 1. MANDATORY: Work Log Hook

> 🛑 **POST-TASK HOOK: Record Work in `work_log.md`**
> After every completed task, record your entry in `docs/plans/<current_phase>/work_log.md`.
> Compact work_log once it reaches 500 lines.

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
| `docs/plans/phase14/` | Active phase. Plans and `work_log.md` live here. |
| `temp/.env` / `temp/server.txt` | API keys and remote GPU server creds. Not versioned. |
| `~/.vox/vox.db` | Local Turso DB. Inspect via `tursodb ~/.vox/vox.db "<SQL>"`. |

**Remote GPU server:** creds in `temp/server.txt`. **Never kill running server processes.**

---

## 3. Execution & Testing Invariants

0. **Compile Gate:** Always use script below , never any shallow or ad-hoc cargo checks.
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

**Local Turso CLI:**
```bash
tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
```

### 4.2 Code Modification Gate

> 🛑 **Before touching code:** Read the corresponding style guide and engineer rule file in `.agents/rules/` for the area you are working on.These are the hard rules that must be followed no matter the task you are working on. 
>
> 🛑 **After modifying code:** Use the `/review` discipline to review the diff you have created and check if there are any violations with respect to the corresponding style guide and engineer rule file in `.agents/rules/` for the area you are working on.If there are reiterate until there are no violations.

---

## 5. Specifications & Spec Alignment (Non-Drift Gate)

All authoritative specifications live in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Reading the relevant spec is mandatory before designing or modifying code.

> 🛑 **Spec Alignment Invariant:** Every agent must adhere strictly to the approved specifications.
> 1. **Divergence / Additions:** If behavior, commands, or schemas diverge from or are absent from the relevant spec — STOP and ask for approval. If approved, update the spec FIRST (following `/create-spec`), then write code. Specs must never quietly drift from code.
> 2. **Legacy Code:** If existing code implements behavior not captured in the spec — confirm with the user before pruning the code or updating the spec.

---

## 6. Current Phase & Work Log

> 📖 **Phase 14 (active):** All completed work and previous agent activity are recorded in [`docs/plans/phase14/work_log.md`](file:///home/addy/projects/apps/vox/docs/plans/phase14/work_log.md).
> Read `work_log.md` to understand recent milestones, established architectural patterns, and verified seam implementations.

---

## 7. Backlog

See **[Phase 14 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase14/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.
- **Backend Ticket**: [Native Barcode Scanner Plugin](file:///home/addy/projects/apps/vox/.scratch/ticket_tauri_barcode_scanner.md) (`tauri-plugin-barcode-scanner`) for Android/iOS ML Kit camera integration.

---
