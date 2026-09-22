# Behavioral Specification: Agentic Tools & Tool Runtime Contract

> **Document Type:** Behavioral & Interface Specification  
> **Target Subsystems:** Cognitive Tool Registry, Tool Execution Engine, and Persistence Scratchpad  
> **Status:** Approved Target Specification  
> **Format Rule:** Pure behavioral specification containing zero programming language code snippets or library-specific references. All behaviors, invariants, parameters, return contracts, and state transitions are specified with rigorous, language-agnostic precision.

---

## 1. Scope & System Axioms

This specification defines the functional contracts, operational classifications, parameter schemas, wire translations, and persistence models for tools invoked by the conversational language model in Vox.

The scope is governed by four system axioms:
1. **The Model Proposes, the Harness Disposes**: The language model is an untrusted planner that proposes tool invocations. The Harness evaluates eligibility, executes the tool, enforces security boundaries, and decides state transitions.
2. **Sacred Audio Hot-Path Isolation**: Tool execution, schema validation, vector embeddings, and database retrieval never block, contend with, or leak into the audio input callback, speech detection loop, or speech synthesis worker.
3. **Dual Classification Duality**: Tools are strictly partitioned into **Terminal Tools (`Terminal`)** and **Non-Terminal Tools (`NonTerminal`)**. Terminal tools resolve the conversational turn in a single pass by delivering the spoken voice response directly alongside the action parameters. Non-terminal tools perform intermediate cognitive tasks (such as retrieval), delivering an immediate spoken filler phrase to eliminate dead air before triggering a reentrant cognitive pass.
4. **Canonical Ingress Normalization**: The Harness consumes a single unified tool representation. All provider-specific wire schemas and transport protocols are translated at the provider adapter boundary.

---

## 2. Provider Ingress Translation & Canonical Representation

Different model providers utilize incompatible wire formats for tool declarations and streaming responses. The provider adapter layer must normalize these into a canonical contract before events reach the Harness.

### 2.1 Canonical Representation
The runtime defines exactly three canonical tool entities:
1. **Canonical Tool Definition**: Contains tool name, plain-language description, JSON Schema parameter specification, and operational classification (`Terminal` or `NonTerminal`).
2. **Canonical Tool Call**: Contains a unique invocation identifier, the targeted tool name, and a fully parsed JSON arguments object.
3. **Canonical Tool Result**: Contains the matching invocation identifier, the tool name, the structured output JSON payload, and a boolean error indicator.

### 2.2 Wire Schema Translation Matrix

#### Modular Assistant Providers (HTTP / SSE)
| Provider Protocol | Egress Schema Translation (Harness $\to$ Model) | Ingress Stream Normalization (Model $\to$ Harness) |
| :--- | :--- | :--- |
| **OpenAI-Compatible** (`/v1/chat/completions`) | Maps canonical definitions to `tools: [{"type": "function", "function": {...}}]`. | Accumulates streamed delta fragments from `choices[].delta.tool_calls[]`. Emits a single completed `Canonical Tool Call` once argument JSON is closed. |
| **Google Gemini (REST)** | Maps canonical definitions to `tools: [{"functionDeclarations": [...]}]`. | Parses `candidates[].content.parts[].functionCall` and normalizes to `Canonical Tool Call`. |
| **Ollama Native** (`/api/chat`) | Maps canonical definitions to `tools: [...]`. | Parses `message.tool_calls[]` and normalizes to `Canonical Tool Call`. |
| **Textual Syntax Fallback** (Local GGUF / Non-tool models) | Injects formatted tool schemas into the root system prompt with XML delimiters. | Provider adapter ingress filter intercepts `<tool_call>{"name":"...","arguments":{...}}</tool_call>` from the raw text stream, strips the tags, parses JSON, and emits a `Canonical Tool Call`. Text tokens never leak into speech synthesis. |

#### Realtime S2S Providers (Full-Duplex WebSocket)
| Provider Protocol | Egress Setup Declaration (`RealtimeActor` $\to$ Provider) | Ingress Tool Call (`Provider` $\to$ `RealtimeActor`) | Outbound Tool Response (`RealtimeActor` $\to$ Provider) |
| :--- | :--- | :--- | :--- |
| **Google Gemini Live** (`BidiGenerateContent`) | Maps canonical realtime definitions to `setup.tools: [{"functionDeclarations": [...]}]`. | Parses `serverContent.toolCall.functionCalls[]` into `Canonical Tool Call`. | Encodes to `toolResponse: {"functionResponses": [{"id": call_id, "response": {"output": result}}]}`. |
| **Deepgram Agent** (`/v1/agent/converse`) | Maps canonical realtime definitions to `settings.configuration.functions: [...]`. | Parses `FunctionCallRequest` into `Canonical Tool Call`. | Encodes to `FunctionCallResponse: {"function_call_id": call_id, "output": result}`. |

### 2.3 Streaming Argument Accumulation Invariant
Tool execution cannot commence on partial JSON fragments. Provider adapters must buffer incremental argument deltas internally and emit a canonical tool call event only when the argument payload has concluded and validated as well-formed JSON. Realtime S2S providers deliver complete function call frames atomically within control messages.

---

## 3. Ingress Capability Gating & Notification Rules

### 3.1 Capability Gate Contract
1. A model's tool-calling capability must be definitively verified prior to offering any tool schema during a session.
2. Capability status is read from a persistent capability discovery cache indexed by the model's provider and identifier. Realtime models (Gemini Live, Deepgram Agent) support tools via the Realtime Service adapter layer without HTTP capability probing.
3. **Session Boot Blocking Probe Invariant**:
   - If a modular model lacks a cached capability record when a session begins, a capability probe executes off the router thread within a strict 4.0-second time boundary before the session transitions to an interactive state (`session_starting = true`).
   - The 4.0-second timeout strictly bounds the HTTP probe round-trip itself, decoupled from model runtime cold-load latency.
   - The session must not transition to `Ready` or accept user speech until capability resolution finishes.
   - If the probe returns a definitive negative (e.g. HTTP 400 "Function calling not supported"), the system records the model as unsupported in persistent cache. Transient network timeouts are treated as session-scoped without permanently marking the model as unsupported.
   - Embedded GGUF models bypass HTTP probing entirely and check `models_manifest.json`.
4. **User Notification**: When a session boots with an unsupported model, the system emits a persistent informational system notification (`group_key: "model_tool_unsupported"`) stating that the active model does not support tool calling and will operate in conversational text-only mode.

---

## 4. Tool Classification & Execution Contracts

Every tool registered in the runtime must strictly declare one of two behavioral categories:

### 4.1 Category A: Terminal Action Tools (`Terminal`)
- **Intent**: Performs an action or mutation and directly resolves the conversational turn in a single generation pass without requiring a follow-up model inference step.
- **Spoken Response Parameter Invariant**: Every `Terminal` tool must include a required `spoken_response: String` parameter in its schema. The model formulates its complete spoken reply inside this parameter alongside the action arguments.
- **Single-Pass Audio Delivery**: Upon invocation, the Harness extracts `spoken_response` and immediately dispatches it to speech synthesis as `AudioIntent::TurnResponse`. Conversational speech begins physical playback without dead air, zero 2-pass delay, and zero secondary LLM call.
- **Pipeline State Invariant**: Invocation of a terminal tool does not transition the conversation pipeline into the working state. The pipeline transitions directly from `Thinking` to `Speaking`.
- **Context Exclusion Invariant**: Terminal tool invocations and their execution outcomes are excluded from subsequent turn prompt histories. They do not pollute working token budgets.
- **Persistence Invariant**: The invocation parameters and execution outcome must be recorded into the `session_tool_calls` scratchpad ledger. Metadata updates (e.g. session title) execute as an awaited persistence write followed by `IpcEvent::SessionsChanged`.

### 4.2 Category B: Non-Terminal Cognitive Tools (`NonTerminal`)
- **Intent**: Retrieves factual data, queries internal state, or accesses external information that the model requires to formulate its conversational answer.
- **Spoken Filler Parameter Invariant**: Every `NonTerminal` tool must include a required `spoken_filler: String` parameter in its schema. The model formulates a brief, natural 3-5 word spoken filler (e.g. *"Checking your notes on that..."*) to eliminate dead air while the tool runs.
- **Pipeline State Invariant**: Invocation of a non-terminal tool transitions the conversation pipeline from `Thinking` to `InteractionState::Working` (triggering `NonTerminalPhase`).
- **Audio Intent Invariant**: The Harness immediately dispatches `spoken_filler` to speech synthesis as `AudioIntent::InterimFiller`. If `spoken_filler` is missing or empty, the Harness falls back to localized phrase selection (`select_filler_phrase`).
- **Turn Loop Invariant**: Non-terminal tools trigger a reentrant cognitive pass. The captured tool result is appended to the turn-local ephemeral scratchpad, re-entering Phase 2 Step 4 with the tool observation.
- **Tool Failure Contract**:
  - If a tool invocation fails (unknown tool, malformed arguments, timeout, or runtime error), the harness must **never** abort the turn. It returns a `ToolResult` with `is_error = true` and a concise, sanitized error string back to the model, allowing the LLM to explain or recover gracefully via speech.
  - Each tool execution is governed by a strict per-tool deadline and captures the turn `CancellationToken`.
  - On `MAX_TOOL_ITERATIONS = 5` breach, the harness executes a final generation pass with `tools = None` rather than aborting, giving the user a complete verbal summary.
- **Context Inclusion Invariant**: The tool call and structured observation result remain present in the turn-local ephemeral scratchpad during the active turn.
- **Persistence Invariant**: The invocation parameters and execution result must be recorded into `session_tool_calls`.

### 4.3 Domain Projections (`ToolDomain::Modular` vs `ToolDomain::Realtime`)
Every tool definition co-locates schemas and descriptions for both interaction domains within its implementation:
1. **`ToolDomain::Modular`**:
   - Embeds speech coordination parameters (`spoken_response` for `Terminal`, `spoken_filler` for `NonTerminal`).
   - Driven by `Harness::execute_turn` and local `TtsActor`.
2. **`ToolDomain::Realtime`**:
   - Strips all speech parameters; exposes clean business fields (`query`, `title`).
   - The remote realtime model synthesizes PCM audio natively over the WebSocket; local `TtsActor` is dormant.
   - Driven event-driven via `RealtimeActor` / `RealtimeSession` over WebSocket without local reentrant prompt assembly.

---

## 5. Persistence Scratchpad Ledger

### 5.1 Separation of Spoken Dialogue and Cognitive Scratchpad
1. Spoken conversation history must be preserved in a dedicated `turns` ledger containing exclusively the user's spoken text, the assistant's spoken text, and sequential turn numbering. It must never contain internal tool syntax or JSON structures.
2. All tool invocations (both terminal and non-terminal, modular and realtime) must be recorded in an independent `session_tool_calls` scratchpad ledger.
3. **Self-Healing Foreign Key Invariant**: Any write to `session_tool_calls` (or session metadata) must execute an idempotent `INSERT OR IGNORE INTO sessions (id, project_id, is_pinned, created_at, updated_at) VALUES (?, 'default', 0, ?, ?)` self-heal before writing to prevent foreign key constraint violations if the initial `SessionStarted` persistence event was delayed or dropped under channel backpressure.
4. **Ephemeral In-Memory Scratchpad**: In active working memory, tool call and observation messages exist exclusively in a turn-local scratchpad owned by `execute_turn` (modular) or active turn state (realtime). Upon turn completion or cancellation, this scratchpad is dropped, ensuring 100% parity between live working memory and DB-restored memory.

---

## 6. Deferred Capabilities (Phase 12.2+)

The following capabilities are formally architected into the data contracts but deferred from Phase 12.1 implementation. Harness-level deferred runtime behaviors (interactive capability probe, filler phrase schema, adaptive recursion, MCP daemon) are documented in `docs/specs/harness-spec.md §10`.

1. **Interactive Multi-Turn Rollback Engine**: User-facing command, state machine, and context reconstruction logic for rewinding active sessions to arbitrary past turns. When implemented, leverages the separated ledgers to execute atomic rollbacks: pruning spoken turns in `turns`, pruning tool traces in `session_tool_calls`, invalidating post-rollback `session_compactions`, and reconstructing context from the latest valid compaction snapshot.
2. **Compensating Action Rollback Registry**: Inverse operations to undo external mutations when a turn is cancelled or rewound. Requires a compensating action registry field on `ToolDefinition`.

---

## 7. Foundational Tool Catalog

### 7.1 Tool 1: `respond_and_set_title` (Modular) / `set_session_title` (Realtime)
- **Classification**: Action / State Mutation.
- **Implementor**: `services/harness/stages/tools/respond_and_set_title.rs`.
- **Modular Specification (`respond_and_set_title`)**:
  - **Classification**: Terminal.
  - **Description**: Responds conversationally to the user while assigning a concise 3 to 5 word title to initialize this new conversation session.
  - **Parameters**:
    - `spoken_response` (String, required): Your natural, concise conversational spoken response to the user's message.
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Turn 1 Injection Gate**: Injected in the model request strictly on the first turn if title is unassigned.
    2. **Turn 2+ Suppression**: Permanently omitted from subsequent turn requests.
    3. **Single-Pass Audio Delivery**: Dispatches `spoken_response` directly to local speech synthesis as `AudioIntent::TurnResponse`.
- **Realtime Specification (`set_session_title`)**:
  - **Classification**: Action.
  - **Description**: Assigns a concise 3 to 5 word title to initialize this new conversation session.
  - **Parameters**:
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Continuous Availability**: Declared continuously in WebSocket session configuration frames.
    2. **Execution-Level Idempotency Guard**: If invoked when the session title is already set, the tool checks `ctx.is_title_already_set()`, skips database write and `IpcEvent::SessionsChanged`, logs invocation to `session_tool_calls` as ignored, and returns a graceful rejection (`{"status": "ignored", "message": "Session title has already been set and is locked for this session."}`).
    3. **Native Provider Audio**: Zero local TTS audio dispatch; the provider model manages vocal output natively.

### 7.2 Tool 2: `search_memory` (Modular & Realtime)
- **Classification**: Cognitive Observation.
- **Implementor**: `services/harness/stages/tools/search_memory.rs`.
- **Description**: Searches episodic project memory for past factual decisions, completed work, blockers, next steps, or technical context.
- **Modular Parameters**:
  - `query` (String, required): The semantic search phrase or keyword expression to look up.
  - `spoken_filler` (String, required): A natural, brief 3 to 5 word spoken filler phrase to say aloud to the user right now while searching.
- **Realtime Parameters**:
  - `query` (String, required): The semantic search phrase or keyword expression to look up. (No spoken filler field).
- **Threshold Authority Rule**:
  - The model is not permitted to specify result limits or threshold cutoffs. User-configured memory settings (`top_k_facts` and `semantic_similarity_cutoff`) apply directly.
- **Search Scope Boundary**:
  - Queries active episodic facts tagged as objective, work done, blocker, next step, or pitfall (excludes personal identity facts).
- **Execution Flow**:
  1. **Modular**: Harness dispatches `spoken_filler` to local TTS (`AudioIntent::InterimFiller`), executes hybrid RRF search, appends facts to scratchpad, and triggers reentrant cognitive pass.
  2. **Realtime**: RealtimeActor executes hybrid RRF search directly, encodes facts into provider `toolResponse` frame, and sends over WebSocket. The provider consumes facts and continues streaming voice audio.
