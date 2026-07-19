# Vox Code Alignment Audit Report
**Benchmark vs. Production Execution Pipeline**

- **Benchmark File:** `app/src-tauri/src/bin/vox_multi_session_bench.rs`
- **Production Pipeline File:** `app/src-tauri/src/services/pipeline.rs`
- **Audit Date:** July 19, 2026
- **Auditor:** Vox Code Alignment Auditor

---

## Executive Summary

An adversarial, line-by-line comparative audit of the benchmark script (`vox_multi_session_bench.rs`) and the actual production execution pipeline (`pipeline.rs`) has revealed **critical, structural misalignments**.

While the benchmark script simulates a highly optimized, dynamically scaled, and well-behaved dual-path execution flow with an `8192`-token context window and background opportunistic compactions, the real-world production pipeline is severely crippled by **hardcoded constraints, missing hot-path optimizations, and unintegrated features**.

These differences lead to a severe divergence in behavior: the benchmark exhibits smooth memory retention and sparse compactions, whereas the production app suffers from aggressive active-turn compaction, high TTFT, massive context/history loss, and unnecessary resource consumption on generic user inputs.

For these reasons, the release validation is graded as a **FAIL**.

---

## Detailed Audit Findings

### 1. Context Window Caps Mismatch & Hardcoded Constraints
One of the most severe discrepancies found is the maximum context window token limits configured for the `ConversationManager`.

*   **Benchmark Configuration:**
    *   The benchmark parses and respects a command-line argument `ctx_size` (defaulting to `8192` tokens):
        ```rust
        // vox_multi_session_bench.rs (Line 366)
        let mut conv_mgr = ConversationManager::new(ctx_window); // ctx_window = 8192
        ```
    *   This allows the working memory to grow up to `8192` tokens before triggering compaction maintenance (at 85% utilization, or ~6963 tokens).
*   **Production Configuration:**
    *   In production, the `ConversationManager` is instantiated inside `core/state.rs` with a hardcoded limit of `2048` tokens:
        ```rust
        // core/state.rs (Lines 411-413)
        conversation_manager: Arc::new(std::sync::Mutex::new(
            crate::services::memory::ConversationManager::new(2048),
        )),
        ```
    *   Although a public helper `set_max_context_tokens(&mut self, max_tokens: usize)` exists in `working_memory.rs`, **it is never called anywhere in the production codebase**.
    *   Even if the user's settings cache specifies `settings.llm.ctx_size = 8192` (which is the enforced minimum for OpenAICompat/remote providers like Gemini or Ollama), the production `ConversationManager` remains hardlocked to `2048` tokens.

> [!CAUTION]
> **Behavioral Divergence:** The production app triggers context maintenance (FIFO or LLM Compaction) at 85% of `2048` tokens (~1740 tokens), while the benchmark waits until ~6963 tokens. This makes production compactions occur nearly **4x more frequently** than simulated, resulting in major latency spikes and severe conversational history truncations that the benchmark never suffers from.

---

### 2. Double-Inverted RAG Retrieval Block Sizes
The size parameters passed to the RAG context retrieval system are completely inverted between the benchmark and production.

*   **The Mismatch:**
    *   **Benchmark:** Restricts retrieved personal context to exactly `2048` tokens:
        ```rust
        // vox_multi_session_bench.rs (Line 462)
        retrieve_personal_context(&conn, &query_vector, &memory_settings, 2048, None)
        ```
        But it runs a `ConversationManager` with `8192` tokens max window.
    *   **Production:** Retrieves personal context up to the setting's `ctx_size` (which is forced to `8192` tokens for remote providers):
        ```rust
        // pipeline.rs (Lines 547-548)
        settings_snap.llm.ctx_size as usize, // 8192 tokens for remote/OpenAICompat
        ```
        But it passes this retrieved context to a `ConversationManager` that is hardlocked to `2048` tokens!

> [!IMPORTANT]
> **Impact:** If the production app retrieves more than `1536` tokens of personal memories (Identity, Constraints, Tasks, Goals, and Semantic Profiles) during a RAG lookup, the retrieved context alone will exceed the `ConversationManager`'s soft/critical threshold (`2048 - 512 = 1536` tokens). 
>
> On the very first turn, the system is forced to run active, blocking compactions inside `build_context`, leading to **infinite compaction loops or immediate truncation of the conversation history**. The benchmark does not simulate or experience this critical architectural bug.

---

### 3. Hot-Path Query Classification Missing in Production
The benchmark script prides itself on classifying queries to isolate generic interactions from semantic ones, reducing unnecessary DB queries.

*   **Benchmark:**
    ```rust
    // vox_multi_session_bench.rs (Lines 446-451)
    let classification = classify_query(&user_prompt);
    if classification.is_generic() { ... } else { ... }
    ```
*   **Production:**
    *   `classify_query` is **completely missing** from `pipeline.rs`.
    *   Every single turn—regardless of whether it's a generic greeting (e.g., *"hello"*, *"yes"*, *"okay"*)—unconditionally generates high-dimensional embeddings and fires SQLite vector cosine distance queries against Turso. This causes extreme and unnecessary CPU/GPU resource overhead on the hot path.

---

### 4. Background & Opportunistic Compactions are Dead Code
The memory architecture specs outline "Point-of-Idle" background compactions to keep active-turn latency minimal.

*   **Benchmark:**
    *   On every simulation turn, the benchmark checks for opportunistic compaction triggers, spawning background generation threads to compress active turns asynchronously and committing them:
        ```rust
        // vox_multi_session_bench.rs (Lines 599-600)
        if let Some((snap_len, snap_msgs, _)) = conv_mgr.try_trigger_opportunistic() { ... }
        ```
*   **Production:**
    *   **There is no opportunistic compaction in production.** `try_trigger_opportunistic` and `commit_opportunistic` are never called in `pipeline.rs`.
    *   The production app will *never* clean or compress active conversation history when the user is silent. All compactions are forced to run as blocking steps during active-turn generation, directly inflating user TTFT.

---

### 5. Final Session Compaction Missing in Production
When a session ends, episodic summaries are meant to be written back to the SQLite DB.

*   **Benchmark:**
    *   At the end of a session, the remaining uncompacted turns are explicitly compiled, compacted, and pushed to personal memory:
        ```rust
        // vox_multi_session_bench.rs (Lines 692-771)
        // [S{} FINAL COMPACTION] ...
        ```
*   **Production:**
    *   When the auto-sleep timeout or user disengagement ends a session, the pipeline does not execute any final compaction. It merely calls `latest_summary()` (which returns a static cached block or empty string) and sends it to the DB worker. Any final active conversational turns are permanently lost and never ingested into the user's episodic/personal profile.

---

### 6. Minor Mismatches and Silent Discrepancies
*   **Barge-in / Cancellation Flow:** The benchmark runs in a pure synchronous clean-room environment. It cannot simulate user barge-ins, active state interrupts, or local VAD silence timers. Thus, the pipeline cancellation routines (`pop_last_user_turn`) are untested by the benchmark.
*   **Similarity Threshold Configuration:** 
    *   The benchmark prints: `Similarity Threshold : 0.65 (Strict BGE-M3 Multi-Vector)`
    *   However, it actually configures: `candidate_similarity_search_threshold: 0.82` (Line 280).
    *   Furthermore, the vector DB retrieval function `retrieve_personal_context` does **not** actually filter results using `candidate_similarity_search_threshold` at all. It simply grabs the top-K nearest neighbors via Turso's SQL query and sorts them chronologically.

---

## Alignment Comparison Table

| Feature / Metric | Benchmark Script (`vox_multi_session_bench.rs`) | Production Execution Pipeline (`pipeline.rs`) | Match | Severity |
| :--- | :--- | :--- | :---: | :---: |
| **Conversation Manager Flow** | Starts session explicitly; runs clean synchronous sequence. | Starts session lazily; manages barge-in, cancellations, and sleep state. | ⚠️ *Partial* | Low |
| **Working Memory Size** | Configurable via CLI, defaults to **`8192` tokens**. | Hardlocked to **`2048` tokens** in `state.rs` (dead-code setter). | ❌ **No** | **Critical** |
| **RAG Retrieval Limit** | Hardcoded to **`2048` tokens**. | Evaluates dynamically up to **`8192` tokens** (based on settings). | ❌ **No** | **Critical** |
| **Query Classifier (Sieve)**| Runs `classify_query` on every turn. | Skipped completely. Generates embeddings for all inputs. | ❌ **No** | **High** |
| **Opportunistic Compactions**| Triggered and committed asynchronously at point-of-idle. | Dead code. Never triggered. All compactions are on-turn/blocking. | ❌ **No** | **High** |
| **Final Session Compaction**| Dedicated compaction run on remaining uncompacted turns. | Only reads `latest_summary()`; uncompacted turns are discarded. | ❌ **No** | **High** |
| **Similarity Threshold** | Logs `0.65` but uses `0.82` (Not filtered anyway during retrieval). | Uses settings `0.82` (Not filtered anyway during retrieval). | ⚠️ *Partial* | Low |

---

## Verdict & Release Recommendation

### **[ALIGNMENT RESULT] FAIL**

The benchmark script diverges drastically from the production pipeline in its memory constraints, hot-path optimizations, and compaction mechanics. It is **highly misleading** as a validation tool since it passes successfully by running in an optimized `8192`-token sandbox, while the production execution pipeline is throttled by a hardlocked `2048`-token context window and lacks the background opportunistic optimizations simulated by the bench.

### **Remediation Action Items:**
1.  **Sync Context Windows:** Fix the production bug by having the pipeline call `set_max_context_tokens` with `settings.llm.ctx_size` upon warming up the LLM, or when settings are updated.
2.  **Harmonize Retrieval Budgets:** Ensure RAG retrieved context is strictly smaller than the working memory's budget (e.g., `retrieve_personal_context` limit should be a fraction of the actual working memory size, not matching the full `ctx_size` which triggers immediate maintenance).
3.  **Integrate Query Classification:** Implement the DistilBERT query-sieve (`classify_query`) inside `pipeline.rs` before generating embeddings, bypassing retrieval for generic turns.
4.  **Activate Opportunistic Compactions:** Integrate the `try_trigger_opportunistic` and `commit_opportunistic` flows into the production event loop under the `PlaybackFinished` or `SessionEnd` hooks to prevent on-turn TTFT spikes.
