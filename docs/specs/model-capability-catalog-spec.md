# Model Capability Catalog & Hierarchical Discovery Specification

## Name & Concept
This specification governs **Model Capability Discovery, Baseline Synchronization, Setting Boundary Constraints, and Compaction Budget Allocation**.

---

## Purpose
Users connect different Large Language Models (LLMs) across local embedded engines, self-hosted servers, and cloud providers. These models vary drastically in input context windows (from 8,192 to over 1,000,000 tokens) and maximum completion limits (from 2,048 to 16,384+ tokens).

Hardcoding model parameters into application code creates fragile software that breaks as providers update their offerings. Conversely, relying on trial-and-error HTTP error negotiation at runtime stalls user voice turns. 

This specification establishes a language-agnostic contract for:
1. Maintaining an updatable external baseline catalog of model specifications.
2. Resolving model capabilities through an empirical, provenance-tracked hierarchy.
3. Dynamically bounding user-facing settings between invariant code floors and discovered capability ceilings.
4. Deterministically calculating compaction output token limits without arbitrary magic numbers.

---

## Must Be True

### 1. External Baseline Catalog & Runtime Synchronization
1. The system must maintain an offline-capable baseline catalog of model specifications (e.g., sourcing model metadata schemas comparable to open-source model databases like `models.dev`).
2. The catalog must record for each known model:
   - Unique model identifier and display name.
   - Model family identifier (e.g., `llama-3.1`, `qwen2.5`, `gemma3`).
   - Published context window (maximum input tokens).
   - Published maximum output tokens (maximum completion tokens).
   - Functional capabilities (tool calling, structured output).
3. The system must bundle a static offline snapshot of this baseline catalog so that application startup and core capabilities function with zero network dependency.
4. When network access is available, the system must periodically perform non-blocking background synchronization against the authoritative remote catalog using conditional HTTP requests (`If-None-Match` with ETag or timestamp caching):
   - An HTTP 304 response must cause zero disk writes and zero re-parsing.
   - An HTTP 200 response must atomically replace the local persistent cache and update the in-memory catalog registry.
   - Network timeouts, DNS failures, or remote HTTP errors must fail silently without interrupting ongoing user interactions or corrupting the existing cached baseline.

### 2. Hierarchical Capability Resolution with Provenance
1. Every capability attribute (specifically `context_window` and `max_output_tokens`) must be associated with an explicit provenance tier:
   - `CatalogBaseline`: Sourced directly from the synchronized model catalog.
   - `FamilyBaseline`: Inferred from a matching model family when an exact model ID is not listed.
   - `ProbedServer`: Empirically verified or reported by the active model runtime or provider API.
   - `Unknown`: Explicitly unobservable and unverified.
2. The resolution precedence must strictly follow:
   $$\text{ProbedServer} > \text{CatalogBaseline} > \text{FamilyBaseline} > \text{Unknown}$$
3. For local embedded models, specifications must be read directly from local model manifests or binary metadata headers.
4. For native local server engines (e.g., Ollama), the true loaded context length, VRAM allocation, and family details must be queried from native server inspection endpoints.
5. For remote cloud endpoints:
   - If an endpoint exposes authoritative capability metadata, that metadata must take precedence.
   - If an endpoint does not expose capability metadata, the system must use the catalog baseline as the initial hypothesis and verify connectivity via an empirical probe.
   - If the endpoint successfully processes the probe, the capability is stamped as verified.
   - If the endpoint rejects a parameter with an explicit HTTP 400 parameter ceiling error, the discovered limit must be recorded and refined for that endpoint.
6. If an attribute cannot be determined from either the catalog or probing, it must remain `Unknown`. The system must never fabricate a synthetic default and pretend it is authoritative.

### 3. User Setting Boundaries (Code Floor vs. Probed Ceiling)
1. User-configurable operational settings (`context_window` and `max_output_tokens` for normal conversational turns) must be constrained dynamically by the interface:
   - **Minimum Floor**: Governed strictly by system code invariants. The `context_window` must never be configured below 8,192 tokens.
   - **Maximum Ceiling**: Governed strictly by the resolved capability ceiling (`max_output_tokens` and `context_window` from the capability resolution engine).
2. If a model's context window or output capacity is in an `Unknown` state, the interface must display the setting as "Server-Managed / Unknown" and reject unvalidated manual input until an empirical probe is performed.
3. User settings for conversational output tokens govern only normal dialogue turns (short voice responses); they do not govern internal maintenance operations.

### 4. Compaction Output Token Budgeting Contract
1. Compaction output token allocation is an internal maintenance budget governed strictly by application code, completely decoupled from conversational output token settings.
2. The compaction budget must be calculated proportionally from the active `context_window` setting:
   $$\text{slice} = \text{floor}(\text{context\_window} \times 0.15)$$
3. The effective compaction output token limit must be bounded by the resolved model capability:
   $$\text{effective\_output\_tokens} = \min(\text{slice},\; \text{probed\_max\_output\_tokens})$$
4. If `probed_max_output_tokens` is `Unknown`, the system must use `slice` as the request limit.
5. If `slice` exceeds `probed_max_output_tokens`, the output limit must clamp strictly to `probed_max_output_tokens` to prevent provider rejections.
6. If `slice` is less than `probed_max_output_tokens`, the system must dispatch `slice`, preventing excessive token allocation on modest context windows.

### 5. Compaction Context Headroom & Input Isolation
1. Critical inline compaction triggers when active session utilization reaches or exceeds 85% of `context_window`.
2. The token utilization calculation includes the session base prompt, identity memory profile, and all conversation turns.
3. The compaction prompt input must strictly exclude the session base prompt, assistant persona instructions, and personal memory profile. Compaction input consists exclusively of:
   - The compaction task instructions and schema.
   - Prior compaction summary (if one exists from a previous cycle).
   - Uncompacted user and assistant conversation turns.
4. Because the session base prompt and personal profile are excluded from the compaction input, the compaction request inherently operates with verified headroom below the 85% trigger threshold.

---

## Must Not Happen

1. **No Hardcoded Parameter Arrays in User Interface**: The UI must never hardcode static context window options (such as `[2048, 4096, 8192]`) for remote server or cloud models.
2. **No Invented Values**: The system must never replace an unknown capability with a synthetic magic number and present it to the user or pipeline as verified.
3. **No Conversational Bleed into Compaction**: The user's conversational `max_output_tokens` setting (e.g. 300 tokens) must never be passed to a compaction request.
4. **No Runtime Error Guessing Loops**: The system must not attempt compaction using trial-and-error downgrading during a live voice turn. The output budget must be resolved prior to dispatch.
5. **No Context Window Below Invariant Floor**: No configuration, mutation, or provider discovery must allow an active context window below 8,192 tokens.

---

## Out of Scope

1. **Inference Hosting / Proxying**: This specification does not cover hosting models, routing API traffic, or managing API keys.
2. **Conversation Turn Sampling**: Temperature, Top-P, Top-K, and frequency penalties for conversational generation are governed by the LLM Harness specification.
3. **Memory Deduplication & Storage**: Deduplication thresholds, Turso SQLite schemas, and consolidation into the durable Personal Memory document are governed by `memory-spec.md` and `db-spec.md`.
