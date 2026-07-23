---

# Vox Cognitive Memory Subsystem (v4) - Cognitive Audit Framework

This document defines the strict, common-sense criteria and evaluation standards for conducting high-fidelity audits on the **Vox Cognitive Memory Subsystem**. 

When validating memory recall, graph databases, or RAG context assemblies, **raw database counts and successful exit codes are entirely irrelevant.** An audit must evaluate semantic correctness, logical relationship validity, and actual conversational precision.

---

## 1. LLM Response Precision & Synthesis

An audit must evaluate if the generated assistant responses are accurate, contextual, and free of noise.

### 1.1 Hallucination & Fact Precision
*   **The Check**: Scan generated responses and extracted facts for fictional entities, assumed constraints, or imported knowledge (e.g., legacy test data like "Sister Emma", "EcoTrack", "dairy-free", "half-marathon").
*   **Target**: **0% Hallucination**. The memory state must strictly reflect the user's explicit transcript input.

### 1.2 Multi-Preference Synthesis
*   **The Check**: When the user provides multiple, separate preferences over time (e.g., "adding walnuts" in Session 1, "adding sunflower seeds" in Session 2), does the LLM successfully synthesize both in its active recall?
*   **Target**: The LLM must not "forget" earlier preferences when new ones are introduced, nor should it hallucinate that they are mutually exclusive unless explicitly stated.

---

## 2. Graph Relationship Logical Validity (Edge Auditing)

Every relationship edge (`CONFLICTS`, `SUPPORTS`, `SIMILAR`, `MERGED`) written to the database must be audited for logical validity.

### 2.1 False Positive Contradiction Analysis
*   **The Check**: Inspect all `CONFLICTS` edges in `memory_relations`. Verify that they represent actual logical contradictions (e.g., "The user moved to Seattle" vs. "The user moved to Chicago").
*   **Common Failure Mode**: The NLI cross-encoder falsely flags unrelated facts containing semantic overlap as conflicting (e.g., "learning Japanese" vs. "learning Spanish", or "developing a Rust module" vs. "practicing Japanese").
*   **Target**: **Minimize NLI noise**. False positives must be identified, counted, and analyzed.

### 2.2 Missing Relationship Edges (False Negatives)
*   **The Check**: Scan active facts in `memory_facts` for unresolved semantic conflicts or supports that lack a linking edge.
*   **Target**: Any clear contradiction or supporting reinforcement must have a corresponding relationship edge written by the worker.

### 2.3 Error Clustering by Collection Category
*   **The Check**: If false positives or missing edges are discovered, audit which of the 10 collections (e.g., `Projects`, `Tasks`, `Preferences`, `Goals`, `Experiences`) they cluster in.
*   **Value**: This identifies whether the NLI model behaves poorly on short, procedural task statements vs. longer preference statements, allowing us to adjust similarity thresholds on a per-collection basis.

---

## 3. Temporal Lineage & Prompt Assembly

Audit how the consolidated memory graph is formatted and presented to the LLM inside the `<user_profile>` context block.

### 3.1 Prompt Header Formatting & Self-Healing
*   **The Check**: Verify that the context retriever correctly extracts active similarities and conflicts, formatting them under explicit decoration headers:
    *   `[Unresolved Contradictions]` -> `- [Unresolved Conflict] "Fact A" CONFLICTS WITH "Fact B"`
    *   `[Unresolved Near-Duplicates]` -> `- [Unresolved Similarity] "Fact A" is SIMILAR TO "Fact B"`
*   **Target**: Expose logical tension directly to the prompt so that the LLM has full situational awareness to resolve conflicts in active conversation (Human-in-the-Loop).

### 3.2 Token Crowding & Starvation
*   **The Check**: Ensure that a single highly active collection (e.g., 30 detailed baking `Preferences` or `Experiences`) does not completely crowd out or starve other critical collections (e.g., 2 active `Tasks` or `Goals`) from the prompt's token budget during RAG retrieval.
*   **Target**: Proportional retrieval allocation across all 10 categorized collections.

---

## 4. Operational & Compaction Efficiency

Audit the background database workers and compaction loops for overhead and redundancy.

### 4.1 Stateful Compaction Deduplication (Template B)
*   **The Check**: Verify that when subsequent compactions run, the LLM-extracted state is diff-subtracted against the previous state (`current_personal_memory`). Only genuine additions, deletions, or updates should be enqueued.
*   **Target**: **0% Redundant Enqueues**. Identical or slightly rephrased facts must not spawn duplicate background jobs.

### 4.2 Exact-Match O(1) Short-Circuiting
*   **The Check**: If a new enqueued fact matches an existing active fact with an alphanumeric Jaccard score of `1.0` or Cosine similarity of `1.0`:
    *   The NLI cross-encoder must be bypassed entirely.
    *   The existing fact's `created_at` timestamp must update to `now` (chronological reinforcement).
    *   The new enqueued fact must be set to `superseded` (historical lineage).
*   **Target**: Eliminate redundant CPU execution overhead and prevent active node duplication.
