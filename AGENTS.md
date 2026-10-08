# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or can impact a future agent's work]
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling)
>
> 1. Migrate **only the delta**: append to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)` heading just the Section 5 entries added since the last migration. Dedupe against the file first — never re-archive entries already present there (snapshotting the whole Section 5 duplicates history).
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Keep the deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests                                   | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks                              | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role.                                                        |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict.                                                         |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`, `nexus-rs`.       |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

0. **Command Context Gate:** Only use `cargo clippy --all-targets` this to verify syntax no other cmds. Test cmds must not be run without explicit user approval.

1. **Sequential Execution with Release flag:** Run benchmarks, evals ,test suites or any cargo command strictly one at a time to prevent CPU, memory, and I/O contention and always use the `--release` flag.
2. **Isolated Test Runner (`cargo-nextest`):** Use the cmd listed below and its variations . 
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution without cold compilation , always use a timeout or cron to monitor test runs and avoid hangs.
4. **Test with External Dependencies(`#[ignore]`):** Cloud or remote server tests must be run manually only with explicit user approval: `cargo nextest -- --ignored`.
---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)
**Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, 
> 🛑 **MANDATORY PRE-EXPLORATION HOOK:**
> - Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY PRE-MODIFICATION HOOK:**
> - You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK:**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Agentic Vox Runtime:** Tool-calling runtime (`respond_and_set_title`, `search_memory`, `web_search`) with a reentrant loop (max 5 iterations), `session_tool_calls` persistence, and capability gating via `model_capabilities.json`.
- **Semantic Structured Personal Memory & Dictation:** Canonical JSON memory model with versioning and consolidation; owner-stamped `VoxEvent` dictation pipeline eliminating TOCTOU races, with a 15k disfluency dataset passing the over-deletion gate (1.734% harmful).
- **Web Retrieval (nexuss 0.1.2, 62%→88% answer-bearing):** Deterministic 24-query multi-domain corpus with self-verified ground truth; answer-presence gate, engine health/circuit breaker, hybrid direct-cosine + BM25 ranking, token handles, and query caching. Published to crates.io.
- **Frontend Platform & Performance:** Responsive SSOT pinned by invariant tests, touch/coarse-pointer capability layer, safe-area insets, `useVirtualRows` virtualization, theme-transition rAF interpolation, and locked overlay radius scale. `tsc` and `pnpm build` green.
- **Android Platform Preparation (Batch 1 of 9):** OpenSSL removed from the Android target graph and unused `zbus` deleted; toolchain wired via `CARGO_TARGET_*` env vars with no committed NDK path. **`enigo` blocks the Android build** (no Android input backend) — Android `cargo check` now gates on Batch 4's dictation exclusion. Plan audit corrected ~20 false claims; see the Phase 12.2 doc.
- **MemoryTimeline Component & Ingestion Pipeline:** Replaced and deleted `MemorySessionRail` with `MemoryTimeline`; added a 35–40% aggregate vertical ingestion timeline in primary accent mapped to `pipeline_processing_enabled` (dimmed when disabled); added Lenis GPU-eased session accordions highlighting 3D graph branches and showing non-personal facts; fixed `pipeline_processing_enabled` side effect triggering ingestion sweeps; automated failed item retries on next sweep; throttled `MemoryIngestionUpdated` events to 1/sec; and completed full backend style guide compliance (extracted sweep logic to `services/memory/ingestion/sweep.rs`, removed inline `crate::` paths, eliminated `_` parameter masking, and fixed error propagation). Clippy and `tsc` green.
- **Memory Timeline & 3D Graph UI Modernization:** Standardized 3D camera orbit pivot locked to `(0, 0, 0)` preventing disorientation during node/session fly-to; implemented complete edge and umbilical conduit vanishing when a session is isolated; replaced random fact connectors with dynamic accent session anchor nodes and interactive `SessionNodeTooltip` (project text, title, date, category breakdown, NO PILLS); resolved fact tooltip clutter by replacing session IDs with session titles and removing redundant active status; added visible category counts to `MemoryLegendOverlay` button text; completely overhauled `MemoryTimeline` with subtext memory counts, category filter underline tabs, category vs chronological sorting toggles, dynamic color-accent borders on fact cards, and a top intelligence overview with real-time session search and extraction totals.
- **Timeline Zero-Pill Architecture & Deep-Linking:** Overhauled `MemoryTimeline` using continuous vertical spine and timeline nodes inspired by shadcn-timeline / assistant-ui; eliminated all faux-pill copy, boxed badges, and container borders across ingestion pipeline telemetry and episodic session streams per `design-spec.md` §5.1; fixed 3D graph vanishing bug by decoupling session node tooltip inspection from session isolation; deep-linked "Focus in Timeline" in `SessionNodeTooltip` to trigger `openPanel("sessions")`, expand the targeted session, and smoothly scroll it into view; verified with `tsc --noEmit`, `check_invariants.mjs` (5/5), `pnpm build`, and `cargo clippy --all-targets`.
- **Memory Timeline UX Polish & Performance Remediation:** Aligned session accordions with `SessionPanel` UX (`MessageSquare`/`MessageSquarePlus` icons, no chevrons, memory count only in subtext, right-hand recency replaced on expansion with interactive Sort & Filter dropdowns); removed timeline spine from session stream and accent left borders from observation cards; replaced search pill with expandable underline search; added breathing room and larger typography to Knowledge Pipeline timeline; gated temporary session toggle when `!isIdle`; paused `MemoryGraph` and `VoxOrb` rendering when panels are open to eliminate animation jitter; guarded WebGL `forceContextLoss` against dropped contexts; and locked `RestorePulse` coordinates to the central orb.
- **Timeline Pipeline Feedback & Menu Alignment:** Enabled dynamic pipeline state badges ("Extracting" with active pulse, "Queued", "Up to date", and "Disabled" with 35% dimming/grayscale when processing disabled); removed redundant extraction metric heading; rendered memory counts with primary accent color; equipped Sort and Filter actions with `<Tooltip>` triggers and portaled dropdown menus opening downward-right; aligned `ProjectContextMenu` and `SessionContextMenu` coordinate anchors to open downward-right into the primary canvas; and synchronized `RestorePulse` inward convergence with 800ms dialogue restoration. Passed `tsc --noEmit`, `check_invariants.mjs` (5/5), `pnpm build`, and `cargo clippy --all-targets --release`.
- **Wizard UI Responsive Overhaul & Design Spec Compliance:** Fixed broken mobile/Android wizard layout and design spec violations: replaced unrounded 90° cards with standard `rounded-xl`/`rounded-2xl` containers (`FeatureCard`, `StatusCard`, audio cards, live test boxes, completed tip); eliminated all faux-pill copy from `ModelCategory` ("Mandatory"/"Optional"), `LiveTestStep` ("Processed"), and `WizardHeader`/`WizardErrorPanel` (boxed step kickers); removed all-caps from error descriptions; standardized typography with responsive heading ramps (`text-2xl sm:text-3xl lg:text-4xl`) and normalized letter-spacing/weights; adapted grids to responsive columns (`grid-cols-1 sm:grid-cols-2`); replaced hardcoded Tailwind colors (`red-500`, `amber-500`, `emerald-500`) with theme tokens (`rgb(var(--danger))`, `rgb(var(--warning))`, `rgb(var(--accent))`); eliminated fixed-width overflow (`w-[400px]`) in the tray mockup with a responsive `max-w-[400px]` container; and updated desktop-centric copy ("computer"/"menu bar") to device-neutral phrasing. Passed `pnpm build` and `cargo clippy --all-targets`.
- **Menu System & Restore Pulse Overhaul:** Eliminated tall `min-h-[40px]` rows across `SessionContextMenu` and `ProjectContextMenu`; restyled all context and timeline menus to Antigravity compact aesthetic (`rounded-xl`, `p-1.5`, `gap-0.5`, `rounded-md py-1.5 px-2.5` items); replaced hardcoded color tokens in `MemoryTimeline` with dynamic theme variants (grey for disabled, accent for processing, darker accent for up to date); rebuilt ingestion pipeline spine using dedicated `w-4.5` centered column with connecting vertical segments eliminating drifting and magic offsets; extended `RestorePulse` to 1400ms organic convergence terminating at the orb's outer perimeter (`scale: 0.55`) with GPU layer promotion (`will-change-transform`, reduced shadow) and full diagnostic tracing. Passed `check_invariants.mjs` (5/5) and `cargo clippy --all-targets --release`.
- **Restore Pulse Polish & Session Panel Active Lag RCA:** Softened `RestorePulse` visual intensity (`border-[rgba(var(--accent),0.4)]`, soft `shadow-[0_0_14px_rgba(var(--accent),0.25)]`, opacity peaked at `0.45`), increased convergence duration to 1700ms terminating at orb perimeter (`scale: 0.58`), and eliminated duplicate pulse by fixing key rebinding. Diagnosed and resolved `SessionPanel` open lag during active voice: extracted `VoiceSessionActionsContext` to decouple action invocations (`selectSession`, `startNewConversation`) from high-frequency voice streaming tokens (`transcript`, `assistantText`), replaced monolithic context consumption in `SessionPanel` with atomic Zustand selectors (`activeSessionId`, `isRestoring`, `restoringSessionId`), and added `[trace-session-panel]` logging. Passed `check_invariants.mjs` (5/5) and `cargo clippy --all-targets --release`.
- **Memory Timeline Dropdown Fix & Graph Synchronization:** Fixed EdgePanel outside-dismiss bug where clicking sort or filter dropdown items closed the panel by stamping dropdown containers with `data-context-menu`; linked timeline session accordion selection to 3D graph camera navigation via `graphRef.current?.flyToSession`; added single-frame render dispatch (`renderOnce`) to WebGL buffer update effect ensuring immediate visual update when paused. Passed `check_invariants.mjs` (5/5) and `cargo clippy --all-targets --release`.
