# Behavioral Specification: Agentic Tools & Tool Runtime Contract Suite

> **Document Type:** System Behavioral & Interface Specification Index  
> **Target Subsystems:** Cognitive Tool Registry, Tool Execution Engine, and Persistence Scratchpad  
> **Status:** Approved Target Specification  
> **Format Rule:** Pure architectural specification containing zero library-specific code snippets.

---

## Specification Suite Index

The Vox Agentic Tools specification suite has been decomposed into focused, modular specifications:

1. **[Core Low-Level Design & Runtime Contract (`lld.md`)](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/lld.md)**
   - System axioms (Model proposes, harness disposes; audio hot-path isolation; canonical normalization).
   - Ingress wire schema translation (OpenAI, Gemini REST, Ollama native, text syntax fallback, Realtime S2S).
   - Ingress capability gating & session boot blocking probe.
   - Dual tool classification (`Terminal` vs `NonTerminal`) and domain classification (`Modular`, `Realtime`, `All`).
   - Ephemeral scratchpad ledger vs permanent spoken turns database boundary.
   - Deferred capabilities (rollback engine, compensating action registry).

2. **[Tool 1: `respond_and_set_title` / `set_session_title`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/respond-and-set-tile.md)**
   - Terminal single-pass speech delivery and asynchronous session title persistence.

3. **[Tool 2: `search_memory`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/search-memory.md)**
   - Episodic memory retrieval combining dense ONNX vector embedding (`all-MiniLM-L6-v2`) and sparse SQLite FTS5 BM25 with Reciprocal Rank Fusion ($k=60$).

4. **[Tool 3: `web_search`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/web-search.md)**
   - Unified web retrieval across a 3-stage lifecycle:
     - **Stage 1 (Raw Search & Ingestion)**: SSRF/DNS-pinned fanout fetch and HTML-to-Markdown extraction.
     - **Stage 2 (Passage Chunking & Corpus Ranking)**: Deterministic 150-word chunking scored via `sparse`, `dense`, or `hybrid` (RRF).
     - **Stage 3 (Context-Bounded Delivery)**: Live `ContextBudgetStage` utilization evaluation with a 30% remaining-budget ceiling.
     - **Security & Injection Sandboxing**: `<web_search_evidence>` XML tag encapsulation and negative inoculation system rules.
     - **v2 Roadmap**: Zero-network passage pagination (`web_search_more`).
