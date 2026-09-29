# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling):
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
> 🛑 **MANDATORY CONTEXT GATE:
**Exploration hook:** Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
> - **ANY WRITE TASK whether its Backend/Frontend/Tests:** You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK (NON-NEGOTIABLE):**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Model-agnostic TTS stack & voice scoping (2026-09-28):** Trait-contract caps, bare-slug voice scoping, wire-format retirement, frontend knowledge deletion; all green (clippy, build, vitest 8/8, nextest 149/149).
- **STT boundary-clipping fix & Wayland paste dynamic loader (2026-09-28):** Pre-roll retention + 300ms stream warmup fixing onset clipping; dynamic dlopen AT-SPI loader + `Ctrl+Alt+V` default shortcut; full nextest suite passing.
- **ZipVoice acoustic profiling & Chatterbox pacing sweeps (2026-09-29):** Prompt-swap root cause exonerated weights/params (bright Kokoro prompt doubled HF energy); steps/guidance tuning matrix executed; candidate audition evaluation.
- **Cognitive memory, agentic runtime & desk cleanups (2026-09-28):** Indexed-block consolidation, Approach 4 JSON memory spec, reentrant tool loop with 122 tests, owner-stamped events, hotkey preemption.
- **Storage, Filesystem & Configuration Architecture Spec (2026-09-29):** Authored `docs/specs/storage-spec.md` formalizing 6-tier filesystem layout (`config/`, `data/`, `models/`, `cache/`, `diagnostics/`, `run/`), 3-way configuration decomposition (`settings.jsonc`, `providers.jsonc`, `agent.jsonc`), JSONC/JSONL format standards, and automated boot migration.
- **Chatterbox cfg-0.3 default & 24-prompt pack (2026-09-29):** `DEFAULT_CHATTERBOX_CFG_WEIGHT=0.3` in prod (`chatterbox.rs`); extended short voices via `voice_clone` (iris 50-word text stalls at 40.16s cap, seed-independent — used shorter text); 24 transcript-guided prompts with Qwen-verified `reference.txt` in `sandbox/zipvoice_prompts/{voice-3,voice-6,voice-10}/` + `manifest.json`, clone sources in `sandbox/cb_extended/`. No bench run; user verifying.
- **Sprint 1/8 atlas HITL (2026-09-29):** Staged atlas-3s/6s/10s from sandbox pack, benched numbers sentence at defaults. atlas-3s rendered 26.6s audio vs 16–17s for 6s/10s (same text) — short-prompt slowdown/repetition anomaly; metrics + wavs handed over, verdict pending.
- **Atlas decommissioned; sprint rules locked (2026-09-29):** 10s+6s tiers dropped (slow), ZipVoice README prescribes <3s single-speaker prompts; 4 runs/sprint (orig+3s × int8/fp32, threads 4). Sprint 2 nova done: fp32 marginally brighter (+0.01 HF), slower RTF; identical durations across weights (quantization doesn't move prosody timing). Verdict pending.
- **Sprint 2 verdict: fp32+orig wins nova (2026-09-29**, no pruning yet). **Sprint 3 vera done:** orig renders 25.4s vs 3s 14.8s (slow speaker either way); fp32 NOT brighter here (orig 0.095 vs int8 0.101 — within noise), so the fp32 edge isn't universal. Verdict pending.
- **Sprint 3 verdict: vera = fp32+3s (2026-09-29). Sprint 4 alfred done:** 3s renders 12.4s vs orig 17.9s; all four HF 0.044–0.067 (darkest sprint yet — alfred pack prompt is HF-poor); fp32 no edge. Verdict pending.
- **Sprint 4 verdict: alfred = fp32+3s (2026-09-29). Sprint 5 maya done:** orig and 3s render near-identical durations (15.0/14.9s); all four HF 0.039–0.051, fp32 no edge either. Verdict pending.
- **Sprint 5 verdict: maya = fp32+3s (2026-09-29). Sprint 6 claire done:** durations identical across all four (14.4s); HF 0.041–0.051, no separation on any axis. Verdict pending.
- **Sprint 6 verdict: claire = fp32+3s (2026-09-29). Sprint 7 iris done:** orig prompt outputs brighter (HF 0.099–0.102) than 3s (0.062–0.065) — first sprint where orig clearly beats 3s; fp32 ≈ int8. Verdict pending.
- **Sprint 7 verdict: iris = fp32+orig (2026-09-29). Sprint 8 sage done (final):** orig brighter (0.082–0.098) than 3s (0.065–0.077), fp32 ≈ int8; orig renders 18.4s vs 13.4s. Verdict pending — all 8 sprints complete after this call.
- **Final pack + learnings doc (2026-09-29):** Winners installed (vera/alfred/maya/claire/sage → new 3s clips, nova/iris keep org, atlas pruned, manifest 7 entries); runnable `zipvoice-fp32/` dir added beside `zipvoice/` (swap-tested, int8 md5-verified); full learnings in `docs/plans/phase12/tts-param-tweaking.md`.

---

---

## 6. Backlog

- **[STT / Fixed 2026-09-28] Nemotron-3.5 initial boundary clipping:** Root-caused to (1) VAD onset gate dropping one 16ms frame + (2) streaming transducer blank-lock on zero-context onsets (no CTC path exists — backlog terminology was wrong). Fixed via pre-roll retention (`segmenter.rs`) + 300ms stream warmup (`NEMOTRON_WARMUP_SILENCE_SAMPLES`); bench-verified. Follow-up data bug: `vera` pack `reference.txt` omits the spoken lead-in "transcript" (confirmed by Qwen).
- **[TTS / Root-caused 2026-09-29] ZipVoice voice quality & acoustic profile:** Prompt-swap proof shows the cloner reproduces reference timbre (bright Kokoro prompt → HF 0.138 vs 0.043–0.095 on VCTK-derived packs); weights/params/pipeline exonerated, steps 8→4 correct per upstream. Fix = curate brighter reference clips (HF content, expressive) or shelf-EQ existing ones; model surgery not needed.
- **[Benchmark] Pipeline bench & Kokoro vs ZipVoice comparative report:** Execute `pipeline_bench` sequentially across Kokoro and ZipVoice baselines and produce comparative TTFA, RTF, E2E latency, and audio quality assessment report.

