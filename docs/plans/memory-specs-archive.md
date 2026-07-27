# Master Cognitive Memory Specification Archive (v1 – v6 Lineage)

**Document ID:** SPEC-COG-MEM-ARCHIVE-V1-V6  
**Status:** HISTORICAL ARCHIVE / DEPRECATED LINEAGE  
**Audience:** System Architects, Cognitive Engineers, and Developer Agents  

---

## 1. Executive Summary & Purpose

This document serves as the canonical historical archive for the evolution of the **Vox Cognitive Memory Architecture** from Version 1.0 through Version 6.2.

It documents the design choices, architectural paradigms, subsystem configurations, and critical failure modes of each generation. This archive exists to prevent regressions, preserve technical rationale, and document the specific bottlenecks that necessitated the transition to the **v7 Memory Architecture**.

---

## 2. Historical Evolution Map (v1 – v6 Cognitive Lineage)

```text
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v1 Cognitive Era: Working (RAM FIFO) + Episodic (MiniLM-L6) + Semantic │
  └────────────────────────────────────┬────────────────────────────────────┘
                                       │
                                       ▼ [Morph: NLP deprecated, LLM extraction introduced]
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v2 Cognitive Era: Working + Episodic (BGE-M3) + Personal (INT8 DeBERTa) │
  └────────────────────────────────────┬────────────────────────────────────┘
                                       │
                                       ▼ [Absorption: Episodic vector store absorbed into Context]
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v3 Cognitive Era: Working + Personal (10 Flat Collections, 15% Budget)  │
  └────────────────────────────────────┬────────────────────────────────────┘
                                       │
                                       ▼ [Graphing: Directed relations SUPPORTS/CONFLICTS]
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v4 Cognitive Era: Directed Relations Graph + Conversational Self-Healing│
  └────────────────────────────────────┬────────────────────────────────────┘
                                       │
                                       ▼ [Decoupling: 4-Phase Pipeline & Staged WAL Ephemerality]
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v5 Cognitive Era: Decoupled 4-Phase Pipeline (Ingestion/NLI/Dedupe/RAG) │
  └────────────────────────────────────┬────────────────────────────────────┘
                                       │
                                       ▼ [Class-Based Expansion: Seed-and-Expand + MiniLM-L12]
  ┌─────────────────────────────────────────────────────────────────────────┐
  │ v6 Cognitive Era: Hybrid Class-Based Graph Expansion & Turso Transaction│
  └─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Version Eras & Detailed Failure Analysis

### 3.1 The v1 Cognitive Era (Pure Subsystems)
- **Subsystem Configuration:**
  - **Working Memory:** Transient RAM-based rolling FIFO list of messages.
  - **Episodic Memory:** Chunk-based persistent vector store using `all-MiniLM-L6-v2` (384d dense vectors) embedding 5-turn session chunks. Cosine similarity score threshold $= 0.55$.
  - **Semantic Memory:** Rule-based NLP entity extraction pipeline.
- **Failures & Bottlenecks:**
  - *Coordinate Centroid Drift:* Searching multi-topic dense episode chunks using short turn-level queries caused dimensional mismatch; cosine similarity hovered near mean centroid, failing to fetch relevant episodes.
  - *Brittle NLP:* Rule-based NLP entity extraction failed completely on natural, loose human dialogue, pinning CPU ($>400$ms).

### 3.2 The v2 Cognitive Era (Personal Memory Morph)
- **Subsystem Configuration:**
  - **Episodic Memory:** Shifted embedding model to `BGE-M3` (1024d dense vectors, Unit-L2 normalized). Similarity threshold $= 0.65$.
  - **Personal Memory:** Deprecated rule-based NLP; introduced direct LLM extraction during compactions via `COMPACTION_SYSTEM_PROMPT`.
  - **NLI Verification Engine:** Introduced local quantized INT8 ONNX Natural Language Inference model (`cross-encoder/nli-MiniLM2-L6-H768`, ~33M params) to evaluate `Entailment` / `Contradiction` / `Neutral`.
- **Failures & Bottlenecks:**
  - *NLI CPU Pinning:* Evaluating every fact against $K=5$ candidates via local NLI pinned CPU cores, causing audio stutter during live voice playback.
  - *Upstream Duplication:* No pre-filtering existed; LLM continuously re-extracted identical or reworded facts.
  - *Micro-Session Erasure:* Fetching only immediate prior session summary meant a 10-second micro-session erased context from a prior 20-minute discussion.

### 3.3 The v3 Cognitive Era (Episodic Absorption & 10 Flat Collections)
- **Subsystem Configuration:**
  - **Episodic Absorption:** Vector store deprecated; session history absorbed into a dedicated operational `Context` collection using Time-Windowed Context Chaining.
  - **Category Consolidation:** Personal memory consolidated into 10 flat categories (`Identity`, `Constraints`, `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`, `Context`, `Tasks`, `Goals`).
  - **Budgeted Retrieval:** Introduced strict 15% hard cap of LLM context window (7% Tier 1 Core, 8% Tier 2 Semantic).
  - **NLI Cosine Pruning:** Pre-filtered with BGE-M3 cosine similarity ($<0.82 \rightarrow \text{Neutral}$), reducing CPU pinning by 85%.
- **Failures & Bottlenecks:**
  - *Semantic Lossiness:* Compactions flattened multi-dimensional categories into single lossy summary sentences.
  - *Automatic Suppression Defects:* Older facts were automatically deactivated without user visibility or control.

### 3.4 The v4 Cognitive Era (The Directed Self-Healing Graph)
- **Subsystem Configuration:**
  - Introduced Directed Relations Graph (`SUPPORTS`, `CONFLICTS`, `USER_SUPERSEDES`, `SIMILAR`, `MERGED`).
  - Multi-tier cosine similarity routing `[0.65 - 0.95]`.
  - Conversational Self-Healing: Surfaced unresolved contradictions directly into RAG prompt context for active user resolution.
- **Failures & Bottlenecks:**
  - Complex manual graph edge routing and high CPU overhead on multi-hop graph sweeps.

### 3.5 The v5 Cognitive Era (Decoupled 4-Phase Pipeline & Staging WAL)
- **Subsystem Configuration:**
  - **Module Decoupling:** Enforced single-responsibility modules (`constants.rs`, `embedder.rs`, `nli.rs`, `deduplication.rs`, `retrieval.rs`, `ingestion.rs`, `persistence/`, `orchestrator.rs`).
  - **4-Phase Pipeline:**
    1. Phase 1: Dual-Defense Hard Deduplication (Jaccard $= 1.0$ or Cosine $> 0.999$).
    2. Phase 2: Candidate Retrieval & DeBERTa-v3 NLI Classification.
    3. Phase 3: Relation Mapping & Atomic SQLite Transaction Write.
    4. Phase 4: RAG Retrieval, 2-Tier Allocation & Edge Resolution.
  - **Ephemerality Staging:** Intra-session tasks and context written as `status = 'staged'` WAL entries, finalized only on `SessionEnd`.
- **Failures & Bottlenecks:**
  - *Staging WAL Complexity:* Managing staged task states across crashes and session boundaries introduced edge-case state pollution.
  - *Overlapping Categories:* The 10 flat CRM collections led to category boundary blur and high duplication across compactions.

### 3.6 The v6 Cognitive Era (Seed-and-Expand Graph Traversal & Class Taxonomy)
- **Subsystem Configuration:**
  - **Unified Pipeline:** Seed Generation $\rightarrow$ Global Seed Pool Assembly $\rightarrow$ Seed-and-Expand Graph Traversal (`max_hops = 2`) $\rightarrow$ Edge Resolution $\rightarrow$ Context Budgeting $\rightarrow$ Context Reranking.
  - **5-Step Async Ingestion Pipeline:**
    1. O(1) String Deduplication (Jaccard $= 1.0$).
    2. Parallel Multi-Worker Embedding (`paraphrase-multilingual-MiniLM-L12-v2` 384d INT8 ONNX).
    3. Class-Based Dispatch:
       - `Class A` (`Identity`, `Context`): Direct write, zero NLI/LLM.
       - `Class B` (`Constraints`, `Tasks`, `Goals`): Intra-collection NLI (`DeBERTa-v3-xsmall`, threshold $= 0.85$).
       - `Class C` (`Skills`, `Preferences`, `Projects`, `Experiences`, `Relationships`): Inter-collection LLM edge creation (`LFM2.5-230M-Q8_0.gguf`).
    4. Parallel NLI / LLM Edge Classification.
    5. Atomic Turso/SQLite MVCC Persistence Transaction.
  - **Dynamic Context Budgeting:** 5% Operational + 10% Semantic calculated dynamically from runtime `ctx_window_size`.

- **Critical Architectural Flaws & Failure Modes Discovered in v6 Benchmark Audits:**
  1. **Agent Amnesia (User-Centric CRM Bias):** The 10 flat collections (`Identity`, `Constraints`, `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`, `Context`, `Tasks`, `Goals`) focused exclusively on building a user profile. Compactions wiped out the **agent's operational state**—active multi-turn execution steps, agent promises/commitments, tool failure pitfalls, and intermediate task state.
  2. **Taxonomy Ambiguity & 52.3% Semantic Duplication:** Overlap between `Projects`, `Tasks`, `Goals`, and `Experiences` caused LLMs to extract identical concepts into multiple collections simultaneously (e.g. 52.3% semantic redundancy in 1,000-turn benchmarks).
  3. **High Compaction Latency:** Parsing and generating JSON with 10 separate array keys during compaction led to 60s+ latency spikes.
  4. **Entity Drift & Misattribution:** LLMs hallucinated non-human entity transformations (e.g. sourdough starter "Doughvid" $\rightarrow$ pet, AI assistant "Vox" $\rightarrow$ human colleague/manager).

---

## 4. Lineage Summary Table

| Version | Core Architecture | Extraction Engine | Deduplication & NLI | Primary Failure Mode |
| :--- | :--- | :--- | :--- | :--- |
| **v1** | Working + Episodic + Semantic | Rule-based NLP | None | Coordinate centroid drift & brittle NLP |
| **v2** | Working + Episodic + Personal | LLM Compaction | Local INT8 DeBERTa NLI | NLI CPU pinning audio stutter |
| **v3** | Episodic Absorbed into Context | LLM Compaction | Cosine Pruning + NLI | Semantic lossiness & automatic fact suppression |
| **v4** | Directed Relations Graph | LLM Compaction | Multi-Tier Cosine Routing | Complex manual graph maintenance |
| **v5** | Decoupled 4-Phase Pipeline | LLM Compaction | Jaccard + Cosine + DeBERTa | Staging WAL complexity & category overlap |
| **v6** | Seed-and-Expand + Class A/B/C | LLM + DeBERTa + LFM2.5 | O(1) String + MiniLM-L12 | **Agent amnesia, 52.3% redundancy, 10 CRM category blur** |
