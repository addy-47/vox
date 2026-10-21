# AGENTS.md — Vox Workspace Rules

---

## 1. MANDATORY: AGENTS.md Sync Hook

> 🛑 **POST-TASK HOOK — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Update Section 5.**
> After every completed task, update Section 5. **One bullet per work seam** — if the work you did belongs to the same lane (same feature, same subsystem, same logical thread) as an existing bullet, extend that bullet. Only work belonging to a genuinely different lane earns a new bullet.
> Do NOT write to `docs/`, `recent_work.md`, or any other file — Section 5 is the only target.
> [Skip for tasks with no lasting accountability: one-off lookups, read-only investigations, questions answered without touching files.]
>
> **Step 2 — Only when approaching 110 lines: Migrate Section 5.**
> After appending, count total lines. At or above **110 lines**:
> 1. Migrate Section 5 to `docs/plans/<current_phase>/recent_work.md` under `## Past Work (YYYY-MM-DD)`.
> 2. Replace Section 5 with a compact 3–5 bullet summary of only the highest-level milestones.

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
| `docs/plans/phase13/` | Active phase. Plans and `recent_work.md` live here. |
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

## 4. Invariants & Specs

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

### 4.3 Spec Alignment (Non-Drift Hook)

> 🛑 Every agent must adhere strictly to the approved specifications.
> 1. **Divergence / Additions:** If behavior, commands, or schemas diverge from or are absent from the relevant spec — STOP and ask for approval. If approved, update the spec FIRST (following `/create-spec`), then write code. Specs must never quietly drift from code.
> 2. **Legacy Code:** If existing code implements behavior not captured in the spec — confirm with the user before pruning the code or updating the spec.

### 4.4 Active Specifications Ledger

All authoritative specs live in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/) , read the all relevant specs for your task as mandatory.explorations 

---

## 5. Current Phase Summary

> 📖 **Phase 13 (active):** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md)
> Phase 12 Archive: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md)
- **Frontend hardening:** test suite (vitest 162) + 0 ESLint errors; warm-nav SWR, route-transition unmount/reveal, session-panel reconcile/virtualization; catalog modal shell and store decomposition.
- **Settings hub re-layout:** 12:00 Persona · 02:00 Memory · 04:00 **Language** · 06:00 Appearance · 08:00 **Speech** · 10:00 Interaction. Small-screen inline lists share the Language card's 260px cap; catalogs render all options in their native scrollers (no “Show N more” paging), have no extra bottom scroll padding, and carry one `Expand` affordance (the feather inverts at the end, the label moves in flow instead of vanishing); Voice Catalog tracks end state directly on native scroll, the TTS grid has one scroll container, Speech category cells have a fixed 94px desktop height with no title-caption gap, and side-card connector anchors use a shared slot center so the 8:00/4:00 pair stays level.
- **Settings Speech card:** Model Hub retired — stage tiles (glyph, name, caption) over detection/transcription/synthesis, `Select voice` header breadcrumb + catalog with its own expand trigger, floating detection gear, seven TTS engines with a per-engine voice modal, header speech-rate gauge. Model rows are logo + name + tier/footprint with persistent info/delete glyphs and family marks (`ModelLogo`). `tuning` domain removed, `models`→`speech` renamed everywhere. The TTS modal header carries the domain commit control (Apply & Restart / Changes Saved), icon-only “Restart” below 1024px. Engine tiles are **modal-only**: `TtsEngineGrid` takes `variant="rows" | "cards"`, the inline Speech-card face stays rows, and `TtsModelsModal` renders 188px `SelectableCard` tiles (shared with the provider modal, `leading` slot) plus the `EngineFlowBanner` pinned **below** the rail+grid (same placement as the voice preview banner); tiles carry only the tier badge (On-device/Cloud, no footprint) at the bottom-left, voices top-right, delete bottom-right.
- **Settings Language card (04:00):** provider rows (on-device, keyed cloud, custom servers) with the saved provider first, unconnected ones dimmed with an inline key field (tick/cross accept-reject) that replaces the pill content, plus a dashed add card that opens the catalog modal; `Select model` swaps the face for that provider's catalog (search/sort/facets) with back navigation. Provider switches now write a model that belongs to the target provider (`resolveProviderModel`: revert to the saved provider restores the saved model, anything else starts empty), so "Apply & Restart" clears on revert; a cloud/server selection with no model shows a `missing-model` footer instead of a committable restart. `LlmTuningControls` resolves capabilities through `resolveCapabilitiesById` (a cache miss is inert, never a crash). The model catalog header carries the three LLM knobs as glyph popovers (`LlmTuningControls`: response length, context window, creativity) and the catalog sort/search were fixed. `llm` scope maps to the `language` domain. `settings/models/` retired: provider surface → `language/`, generic primitives → `shared/` (`CatalogRowCard`, `AddCard`, `CardTitleBar`, `CatalogModalShell`, `CatalogReveal` — the shared feather + `Expand` reveal used by the provider, remote-model, TTS-engine, and voice catalogs). Provider rows select only; the trailing arrow is a separate "Browse models" hitbox that never re-selects an already active provider. The provider modal always opens on the **Available** filter (no `initialFilter` prop). `CatalogReveal` is the only expand affordance (label "Expand" + maximize glyph, suppressed inside a modal; the floating corner `Maximize2` is gone, replaced by `hideInlineTrigger`). Both Language modals (provider catalog + model catalog) carry the `language` commit control in the header (Apply & Restart / Changes Saved, icon-only “Restart” below 1024px); a missing cloud key renders one credential banner, never a second “Connection failed” banner for the same cause.
- **Voice pipeline + backend gates:** segmentation/onset RCA fixes, turn-identity gates on TTS playback, RULE-HARN-03 dedup moved pre-side-effect; `verify-targets.sh` clippy/lint-arch gate green.
- **Monitoring null-safety:** `latest` snapshot can be null before first fetch; guarded Monitoring.tsx with a typed empty fallback so `is_vad_loaded` (and siblings) never dereference null.

---

## 6. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.
