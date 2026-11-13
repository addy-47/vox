# Behavioral Specification: Agentic Tools & Tool Runtime Contract

> **Document Type:** Behavioral & Interface Specification  
> **Target Subsystems:** Cognitive Tool Registry, Tool Execution Engine, and Persistence Scratchpad  
> **Status:** Approved Target Specification  
> **Format Rule:** Pure behavioral specification containing zero programming language code snippets or library-specific references. All behaviors, invariants, parameters, return contracts, and state transitions are specified with rigorous, language-agnostic precision.

---

## 1. Scope & System Axioms

This specification defines the functional contracts, parameter schemas, wire translations, and persistence models for tools invoked by the conversational language model in Vox.

The scope is governed by four system axioms:
1. **The Model Proposes, the Harness Disposes**: The language model is an untrusted planner that proposes tool invocations. The Harness evaluates eligibility, executes the tool, enforces security boundaries, and decides state transitions.
2. **Sacred Audio Hot-Path Isolation**: Tool execution, schema validation, vector embeddings, and database retrieval never block, contend with, or leak into the audio input callback, speech detection loop, or speech synthesis worker.
3. **Universal Tool Contract (Zero Backward Compatibility / ZBC)**: Tools are unified cognitive primitives. Tools execute purely functional domain logic and return structured observations. Tools MUST NOT declare execution flow categories (`Terminal` vs `NonTerminal`) and MUST NOT embed speech coordination parameters (`spoken_filler`, `spoken_response`). Conversational speech is generated naturally by the language model and streamed eagerly via the 3-Stream Demuxer.
4. **Canonical Ingress Normalization**: The Harness consumes a single unified tool representation. All provider-specific wire schemas and transport protocols are translated at the provider adapter boundary.

---

## 2. Provider Ingress Translation & Canonical Representation

Different model providers utilize incompatible wire formats for tool declarations and streaming responses. The provider adapter layer must normalize these into a canonical contract before events reach the Harness.

### 2.1 Canonical Representation
The runtime defines exactly three canonical tool entities:
1. **Canonical Tool Definition**: Contains tool name, plain-language description, and JSON Schema parameter specification.
2. **Canonical Tool Call**: Contains a unique invocation identifier, the targeted tool name, and a fully parsed JSON arguments object.
3. **Canonical Tool Result**: Contains the matching invocation identifier, the tool name, the structured output JSON payload, and a boolean error indicator.

### 2.2 Wire Schema Translation Matrix

#### Modular Assistant Providers (HTTP / SSE)
| Provider Protocol | Egress Schema Translation (Harness $\to$ Model) | Ingress Stream Normalization (Model $\to$ Harness) |
| :--- | :--- | :--- |
| **OpenAI-Compatible** (`/v1/chat/completions`) | Maps canonical definitions to `tools: [{"type": "function", "function": {...}}]`. | Accumulates streamed delta fragments from `choices[].delta.tool_calls[]`. Emits `Canonical Tool Call` items into the multi-tool batch. |
| **Google Gemini (REST)** | Maps canonical definitions to `tools: [{"functionDeclarations": [...]}]`. | Parses `candidates[].content.parts[].functionCall` and normalizes to `Canonical Tool Call`. |
| **Ollama Native** (`/api/chat`) | Maps canonical definitions to `tools: [...]`. | Parses `message.tool_calls[]` and normalizes to `Canonical Tool Call`. |
| **Textual Syntax Fallback** (Local GGUF / Non-tool models) | Injects formatted tool schemas into the root system prompt with XML delimiters. | Provider adapter ingress filter intercepts `<tool_call>{"name":"...","arguments":{...}}</tool_call>` from the raw text stream, strips the tags, parses JSON, and emits a `Canonical Tool Call`. Text tokens never leak into speech synthesis. |

#### Realtime S2S Providers (Full-Duplex WebSocket)
| Provider Protocol | Egress Setup Declaration (`RealtimeActor` $\to$ Provider) | Ingress Tool Call (`Provider` $\to` `RealtimeActor`) | Outbound Tool Response (`RealtimeActor` $\to$ Provider) |
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

## 4. Universal Tool Interface & Execution Architecture

### 4.1 Universal Tool Contract (Zero Backward Compatibility / ZBC)
Every cognitive tool in Vox conforms to a single universal contract:
1. **Zero Flow Categorization**: The runtime does not differentiate between terminal and non-terminal tools. Every tool is an asynchronous function taking clean JSON parameters and returning a `ToolResult`.
2. **Zero Speech Schema Parameters**: Tools MUST NOT include `spoken_filler`, `spoken_response`, or any speech generation parameters in their JSON schemas. The language model formulates speech naturally through the standard content token stream.
3. **Preamble Speech Autonomy**: If the model speaks before proposing a tool call (e.g., *"Let me search your notes"*), the 3-Stream Demuxer eagerly chunks and streams those introductory tokens to speech synthesis with `AudioIntent::InterimFiller`. Audio plays concurrently while tools execute.

### 4.2 Multi-Tool Batch Execution & Barriers
1. **Batch Ingress**: When the model proposes $1..N$ tool calls in a single generation pass, the router collects all calls into an ordered batch before execution begins.
2. **Barrier Scheduling**:
   - **Parallel Barrier**: Consecutive read-only observation tools (e.g. `search_memory`, `web_search`) execute concurrently in parallel async tasks via `join_all`.
   - **Exclusive Barrier**: Mutating or stateful tools (e.g. `set_title`) execute sequentially with isolation guarantees.
3. **Tool Failure Contract**:
   - If a tool invocation fails (malformed arguments, timeout, or external failure), the harness **never** aborts the turn. It returns a `ToolResult` with `is_error = true` and a sanitized error string back to the model, allowing the LLM to explain or recover gracefully in speech.
   - Each tool execution is governed by a strict per-tool deadline and captures the turn `CancellationToken`.
4. **Reentrant Step Loop**: Tool observation results are committed immediately to dialogue history at each step boundary. The cognitive coordinator executes up to `MAX_TOOL_STEPS = 20` steps per turn. If step 20 is reached without a terminal answer, a final generation pass with tools disabled produces a complete spoken summary.

### 4.3 Tool Domain Classification (`ToolDomain { Modular, Realtime, All }`)
Every tool definition declares its operational domain scope via the `ToolDomain` enum:
1. **`ToolDomain::Modular`**:
   - Tool is valid strictly in Modular Assistant sessions (`PipelineMode::Modular`).
   - Filtered out and hidden from Realtime sessions.
2. **`ToolDomain::Realtime`**:
   - Tool is valid strictly in Realtime S2S sessions (`PipelineMode::Realtime`).
   - Filtered out and hidden from Modular sessions.
3. **`ToolDomain::All`**:
   - Universal tool valid across all interaction modes (`PipelineMode::Modular` and `PipelineMode::Realtime`).

---

## 5. Persistence Scratchpad Ledger & Context Retention

### 5.1 Separation of Spoken Dialogue and Cognitive Scratchpad
1. Spoken conversation history must be preserved in a dedicated `turns` ledger containing exclusively the user's spoken text, the assistant's spoken text, and sequential turn numbering. It must never contain internal tool syntax or JSON structures.
2. All tool invocations must be recorded in an independent `session_tool_calls` scratchpad ledger without legacy `tool_kind` columns.
3. **Self-Healing Foreign Key Invariant**: Any write to `session_tool_calls` (or session metadata) must execute an idempotent `INSERT OR IGNORE INTO sessions (id, project_id, is_pinned, created_at, updated_at) VALUES (?, 'default', 0, ?, ?)` self-heal before writing to prevent foreign key constraint violations if the initial `SessionStarted` persistence event was delayed or dropped under channel backpressure.
4. **Working Context Tool Retention**: In active working memory, tool calls and admitted observations from completed steps are committed immediately to `ConversationHistoryStage` across the active session. This enables the model to resolve natural verbal follow-ups ("tell me more") directly from conversation context with zero additional network requests. If a turn is cancelled or interrupted before assistant speech begins, the turn-local step history is rolled back.
5. **Context Budget Authority**: Context budget computation belongs strictly to the harness `ContextBudgetStage`. The harness computes the token ceiling and provides it to the tool via `ToolExecutionContext`. Cognitive tools never inspect global settings, never query database compactions, and never estimate history tokens themselves.

---

## 6. Deferred Capabilities (Phase 12.2+)

The following capabilities are formally architected into the data contracts but deferred from Phase 12.1 implementation:
1. **Interactive Multi-Turn Rollback Engine**: User-facing command, state machine, and context reconstruction logic for rewinding active sessions to arbitrary past turns.
2. **Compensating Action Rollback Registry**: Inverse operations to undo external mutations when a turn is cancelled or rewound. Requires a compensating action registry field on `ToolDefinition`.

---

## 7. Foundational Tool Catalog

Each tool registered in the Vox harness runtime adheres to the operational, state, and security invariants defined in Sections 1–5:

1. **[Tool 1: `set_title`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/respond-and-set-tile.md)**
   - Classification: Universal Action
   - Scope: Session title assignment without artificial `spoken_response` wrappers.
2. **[Tool 2: `search_memory`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/search-memory.md)**
   - Classification: Cognitive Observation
   - Scope: Episodic fact retrieval via dense ONNX vector embedding + sparse FTS5 BM25 hybrid RRF ranking without `spoken_filler`.
3. **[Tool 3: `web_search`](file:///home/addy/projects/apps/vox/docs/specs/tools-specs/web-search.md)**
   - Classification: Cognitive Observation
   - Scope: Live web retrieval via 3-stage lifecycle (guarded fanout fetch $\to$ passage chunking & full corpus ranking $\to$ dynamic context-budgeted evidence delivery) without `spoken_filler`.
