# AGENTS.md — Vox Workspace Rules

---

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

---

## 2. Workspace Directory Map

| Path | Purpose | Rules |
|---|---|---|
| `app/src-tauri/src/` | Production Rust source | No test logic. No benchmarks. |
| `app/src-tauri/tests/` | Integration tests (`cargo test --tests`) | Named `<feature>_test.rs`. Tests public API only. |
| `app/src-tauri/benches/` | Performance benchmarks (`cargo test --benches`) | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`. |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`) | Standalone tools. No `#[test]`. No assertions. |
| `.agents/rules/` | Role-specific agent instruction files | Read relevant file before acting in that role. |
| `docs/plans/` | Architecture specs and phase plans | Source of truth for specs. Do not contradict. |
| `docs/features/` | Implemented feature ledgers | Update after completing features. |
| `sandbox/` | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`. |
| `temp/` | Ephemeral runtime files: logs, raw LLM outputs | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned. |
| `submodules/` | Git submodules | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/` | Local model weights | Canonical manifest: `~/.vox/models/models_manifest.json`. |

**Remote GPU server:** `hypr4@100.86.62.14` (creds in `temp/server.txt`). Ollama + LMS available. **Never kill running server processes.**

---

## 3. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
> - **WRITE TASK (Adding/editing code, refactoring, fixing bugs):** You MUST read `.agents/rules/code-style-guide.md` AND the relevant role rule file (e.g. `.agents/rules/backend-engineer.md` or `frontend-engineer.md`) BEFORE modifying code.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

## 4. Agent Roles

| Role | Rule File | Scope |
|---|---|---|
| System Architect | `.agents/rules/system-architect.md` | Strategy, gates, plan approval |
| Backend Engineer | `.agents/rules/backend-engineer.md` | `app/src-tauri/src/` implementation |
| Frontend Engineer | `.agents/rules/frontend-engineer.md` | `app/src/` implementation |
| QA Engineer | `.agents/rules/qa-engineer.md` | Test audit, benchmark validation |

**Subagent reuse rule:** Re-use existing subagent conversation IDs via `send_message`. Do not spawn duplicate subagents per turn.

---

## 5. Current Phase — Gate 1: Local Model Benchmarking & Edge Verification

### What is being validated
1. **Class B Intra-Collection NLI** (`Constraints`, `Tasks`, `Goals`) — `deberta-v3-xsmall` ONNX. Candidate filter: `same_collection_candidate_search = 0.40`. NLI threshold: `0.85`.
2. **Class C Inter-Collection LLM Edge Generation** (`Skills`, `Preferences`, `Projects`, `Experiences`, `Relationships`) — `LFM2.5-230M-Q8_0.gguf`. Candidate filter: `inter_collection_candidate_search = 0.75`.
3. **Class A Strict Isolation** (`Identity`, `Context`) — zero NLI or LLM calls. Direct write only.
4. **Deterministic Inverse Edge Mapping** — every forward edge auto-generates its inverse at runtime in SQLite.

### Collection Taxonomy (`core/constants.rs`)
| Constant | Collections | Behavior |
|---|---|---|
| `PM_CLASS_A_COLLECTIONS` | `Identity`, `Context` | Direct isolation. No NLI. No LLM. |
| `PM_CLASS_B_COLLECTIONS` | `Constraints`, `Tasks`, `Goals` | Intra-collection NLI only. |
| `PM_CLASS_C_COLLECTIONS` | `Skills`, `Preferences`, `Projects`, `Experiences`, `Relationships` | Inter-collection LLM edge creation only. |

### Gate 1 Pass Criteria
| Metric | Target |
|---|---|
| Class B NLI latency | < 20ms per pair |
| Class B NLI accuracy | ≥ 90% |
| Class C LLM edge latency | < 100ms per pair |
| Class C edge precision | ≥ 85% vs gold reference |
| Connection policy compliance | 100% |
| Inverse edge auto-creation | 100% of forward edges |
| Class A isolation false-positive rate | 0% |

### Bans — Never accept as Gate pass
- Exit code 0 without reading metric output
- Mock model fallbacks (must run actual local weights)
- LLM edges between forbidden collections (Class A or Class B intra-collection)
- Vanity counts ("20 edges created") without checking against connection matrix

### Model Paths & Harness
- NLI: `~/.vox/models/nli/deberta-v3-xsmall/model_quantized.onnx`
- Class B LLM: `~/.vox/models/llm/LFM2.5-230M-Q8_0.gguf` (+ `Q4_K_M`, `Q4_0` variants)
- Embedding: `~/.vox/models/embedding/bge-m3/model_quantized.onnx` (1024-dim)
- Primary Harness: `benches/vox_multi_session_bench.rs` (`cargo test --bench vox_multi_session_bench`)
- Specs: `docs/plans/v6-memory-architecture-spec.md`, `docs/features/memory-architecture.md`
