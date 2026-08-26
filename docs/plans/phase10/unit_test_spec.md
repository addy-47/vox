# Unit Test Specification — Phase 10 Algorithmic & State Invariants

---

## 1. Overview & Testing Philosophy

This document is the **Single Source of Truth (SSOT)** for Phase 10 Unit Tests (UT) in Vox.
Following the testing hierarchy (**UT → IT → E2E → Benches**):
- **Pure In-Memory Execution**: Zero network requests, zero file I/O, zero model weights.
- **Microsecond Scale**: Tests run purely on CPU stack/heap in microseconds.
- **Algorithmic Invariants & Boundary Guards**: Asserts exact mathematical identities, state machine transitions, and intentional failure/edge cases.

---

## 2. Invariant Matrix & Target Modules

| # | Target Module | Functions Tested | Core Invariants Proved | Negative & Boundary Cases |
|---|---------------|------------------|------------------------|---------------------------|
| **1** | [`services/utils.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/utils.rs) | `should_flush` | • Sentence terminals (`.!?।`) flush immediately regardless of TPS/time.<br>• Clause punctuation (`,;—`) dynamically scales: at TPS 0.5/3.0 flushes at 3 words; at TPS 6.0 clause flush is disabled.<br>• Timeout starvation triggers only at word boundaries. | • Mid-word incomplete tokens (e.g. `"The quick bro"`) return `false` even if timeout expired.<br>• TPS clamping boundaries: $tps \le 0.5 \to 0.5$, $tps \ge 6.0 \to 6.0$ without panics. |
| **2** | [`services/utils.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/utils.rs) | `stitch_transcripts` | • Suffix-to-prefix word overlap stitching via Levenshtein edit distance.<br>• Soft subslice containment returns prefix unmodified.<br>• Empty prefix/suffix returns counterpart cleanly. | • Disjoint non-overlapping chunks concatenate cleanly with single whitespace.<br>• Case/punctuation variations match correctly. |
| **3** | [`services/memory/deduplication.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/deduplication.rs) | `jaccard_similarity`, `is_exact_duplicate` | • Punctuation and casing normalization: $J=1.0$ for matching word sets.<br>• Exact duplicates detected if $J \ge 1.0$ or $\text{cosine} \ge 0.98$.<br>• Disjoint sets yield $J=0.0$. | • Below-threshold overlap (e.g. $J=0.60$) rejects duplicate (`false`).<br>• Empty vs empty yields $1.0$; empty vs non-empty yields $0.0$ (no zero-division). |
| **4** | [`services/vad/earshot_vad.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/earshot_vad.rs) | `EarshotVadEngine::predict` debouncing | • Speech start requires $\ge 15$ consecutive active frames (~240ms).<br>• Silence hangover requires $\ge 40$ consecutive inactive frames (~640ms). | • 14 active frames leave `is_speech == false`.<br>• 39 inactive frames keep `is_speech == true`.<br>• 3-frame noise burst is rejected. |
| **5** | [`utils/audio_filters.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/audio_filters.rs) | `FilterBank::tick`, `process_chunk`, `LowPass` | • Subtractive Filter Bank identity: $\text{Low} + \text{Mid} + \text{High} \approx \text{Input}$ within $\epsilon = 10^{-5}$.<br>• DC pass-through ($x=1.0 \to y \to 1.0$) and high-frequency attenuation. | • Silent buffer `[0.0; N]` returns RMS `(0.0, 0.0, 0.0)`.<br>• Empty slice `&[]` returns `(0.0, 0.0, 0.0)` without panic. |
| **6** | [`services/memory/scope_router.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/scope_router.rs) | `route_scope` | • Exhaustive check across all 4 `MemoryScope`s:<br>  - `ChitChat` $\to$ empty SQL & empty vector.<br>  - `User` $\to$ vector: `[Profile, Constraints]`.<br>  - `Domain` $\to$ vector: `[Entities, Directives, Constraints]`.<br>  - `Temporal` $\to$ SQL: `[Directives, Narrative]`, vector: `[Constraints]`. | • Asserts `Identity` is never routed to SQL.<br>• Asserts `ChitChat` never allocates retrieval context. |
| **7** | [`services/llm/capability_probe.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capability_probe.rs) | `resolve_chat_url`, `parse_token_ceiling_from_error` | • URL normalization is idempotent across root URL, `/v1`, `/chat/completions`, and trailing slashes.<br>• Token ceiling regex extracts integer values from HTTP 400 error payloads. | • Out-of-bounds ceiling values (<256 or >2,000,000) return `None`.<br>• Unrelated error text returns `None`. |
| **8** | [`services/memory/formatter.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/formatter.rs) | `format_relative_timestamp`, `format_user_profile_context` | • Relative timestamp humanization correctly produces exact buckets: `"Just now"`, `"X minutes ago"`, `"X hours ago"`, `"Yesterday"`, `"X days ago"`, `"X weeks ago"`.<br>• XML builder generates `<user_profile>` structure. | • Negative timestamp deltas (clock skew) return `"Just now"` without underflow panic.<br>• Empty section blocks are omitted entirely. |
| **9** | [`core/state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs) & [`services/pipeline/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/mod.rs) | `InteractionOwner`, `target_window` | • Binary owner conversion: `0 <-> Dictation`, `1 <-> Assistant`, fallback $\to$ `Dictation`.<br>• Target window routing: `Dictation -> "tray"`, `Assistant -> "main"`. | • Unknown integer value (e.g. `99`) safely falls back to `InteractionOwner::Dictation`. |

---

## 3. Implementation Rules

1. **Inline Rust `#[cfg(test)]`**: Tests reside in the respective file to test both public and private algorithmic helpers directly.
2. **Zero `#[allow(...)]` Suppressions**: Invariant strictly enforced per `AGENTS.md`.
3. **Deterministic Output**: No random seeds or non-deterministic floats without epsilon comparisons (`(a - b).abs() < 1e-5`).
