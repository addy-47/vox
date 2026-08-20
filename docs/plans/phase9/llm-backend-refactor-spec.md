# Vox LLM Backend Refactor Specification

**Status:** Proposed\
**Date:** 2026-08-08\
**Scope:** LLM provider abstraction, generation parameters, capability
discovery, context/output limits, structured output, streaming, and
memory compaction.

------------------------------------------------------------------------

## 1. Executive Summary

The current Vox LLM backend conflates three different concerns:

1.  **Application-level generation intent** --- what Vox wants
    (`temperature`, output budget, reasoning level, structured output,
    context budget).
2.  **Model/runtime capabilities** --- what a particular model can
    actually do.
3.  **Wire protocol** --- how a provider expresses those capabilities
    (`/v1/chat/completions`, `/v1/responses`, Ollama native `/api/chat`,
    LM Studio APIs, embedded llama.cpp).

This is the primary architectural problem.

The refactor should **not** replace the current hardcoded
OpenAI-compatible JSON with a larger universal JSON object. Instead, Vox
should introduce a provider-neutral generation request and a
capability-aware adapter layer.

### Target architecture

``` text
                    Vox application
                          |
                          v
                +--------------------+
                | GenerationRequest  |
                |--------------------|
                | messages/input     |
                | generation options |
                | output constraints |
                | structured output  |
                | reasoning          |
                | purpose            |
                +---------+----------+
                          |
                          v
                +--------------------+
                | Capability-aware   |
                | Provider Adapter   |
                +---------+----------+
                          |
          +---------------+----------------+
          |               |                |
          v               v                v
      Embedded        OpenAI-style      Native/local
      llama.cpp       HTTP APIs         APIs
          |               |                |
          v               v                v
       llama.cpp       OpenAI /        Ollama / etc.
                       Gemini / NIM /
                       LM Studio
```

The application must not know that one provider calls the output budget
`max_tokens`, another `max_completion_tokens`, another
`max_output_tokens`, or that a local runtime exposes context size as a
model/runtime setting rather than a request field.

------------------------------------------------------------------------

## 2. Current-State Problems

The following current-state findings are taken from the code audit
supplied with this specification. They should be treated as the baseline
to preserve behavior during migration unless explicitly changed.

### 2.1 No unified generation parameter object

`LlmProvider::generate(...)` does not carry a unified generation
configuration. As a result:

-   maximum output tokens cannot be requested;
-   temperature is not centrally controlled;
-   provider defaults differ;
-   compaction inherits normal-chat sampling behavior;
-   context limits are handled differently by local and remote
    providers.

### 2.2 Hardcoded sampling

Current behavior includes:

-   Ollama: `temperature = 0.2`;
-   LM Studio: `temperature = 0.2`;
-   Standard OpenAI-compatible: temperature omitted;
-   embedded GGUF: Qwen gets `top_k(20)`, `top_p(0.95)`, `temp(0.6)`;
    other models use greedy sampling.

This makes the same Vox setting produce materially different model
behavior.

### 2.3 Context size is incorrectly treated as one universal request parameter

The current code uses:

-   `LlmSettings.ctx_size` for embedded llama.cpp;
-   hardcoded `8192` for remote Ollama;
-   hardcoded `8192` for LM Studio;
-   server/model-managed context for standard OpenAI-compatible APIs.

These are different concepts and must remain different in the
architecture.

**Important:** a remote model's context window is normally a model
capability/limit, not a request parameter called `context_window`. The
client may choose how much context to send, but it does not generally
resize the provider's model context window per request.

Ollama's OpenAI compatibility documentation explicitly notes that
context size is not configured through the OpenAI-compatible request;
Ollama configures it through its model/runtime settings. This reinforces
the separation between **model/runtime configuration** and **request
generation options**.

### 2.4 No output-token budget

Current generation is effectively unbounded until EOS/EOG or another
runtime timeout/limit.

This is a correctness, latency, and cost problem for remote providers
and a memory/latency problem for local inference.

OpenAI currently exposes output caps through `max_completion_tokens` on
applicable Chat Completions models and `max_output_tokens` on Responses.
Provider-specific adapters must translate the application's neutral
output budget to the correct wire field. citeturn2search0

### 2.5 Structured output is implemented as prompt inspection

The current Standard OpenAI-compatible backend infers JSON intent by
searching message text for strings such as:

``` text
JSON
AI Memory Extraction Assistant
compaction
compress
```

This is brittle.

Structured output must become an explicit request property:

``` rust
response_format: StructuredOutput
```

The adapter then decides whether/how the provider expresses it.

### 2.6 Compaction is not a first-class generation mode

Compaction currently shares most generation behavior with normal chat,
despite being semantically different.

Compaction should explicitly declare:

``` rust
purpose: GenerationPurpose::MemoryCompaction
```

This permits:

-   a different output budget;
-   deterministic sampling;
-   mandatory structured output;
-   stricter failure handling;
-   different context assembly;
-   different retry policy.

------------------------------------------------------------------------

# 3. Research Findings: OpenAI-Compatible Does Not Mean Identical

There is no universal contract saying that every OpenAI-compatible
endpoint accepts every current OpenAI parameter.

The compatibility surface is provider + endpoint + model dependent.

## 3.1 OpenAI

OpenAI currently has both:

-   `POST /v1/chat/completions`
-   `POST /v1/responses`

The newer Responses API uses concepts such as `input`,
`max_output_tokens`, `reasoning`, and `text`. Chat Completions continues
to exist and uses `messages`, with model-dependent token controls such
as `max_completion_tokens` and legacy `max_tokens`. OpenAI's current
documentation also describes reasoning and verbosity controls for
applicable models. citeturn2search0

**Design implication:** do not make `/v1/chat/completions` the universal
internal abstraction.

## 3.2 Ollama

Ollama exposes an OpenAI compatibility layer and documents a supported
parameter set for `/v1/chat/completions`. It also has its own native
API.

For Vox, Ollama should be treated as a provider with:

``` text
Native API:
  /api/chat

OpenAI-compatible API:
  /v1/chat/completions
```

Context size is a model/runtime concern rather than a generic
OpenAI-compatible request field.

**Design implication:** retain an Ollama adapter, but do not encode
Ollama-specific `num_ctx` into a universal `GenerationRequest`.

## 3.3 LM Studio

LM Studio currently exposes:

-   `/v1/models`
-   `/v1/chat/completions`
-   `/v1/responses`
-   `/v1/embeddings`
-   `/v1/completions`

Its Chat Completions documentation lists parameters including:

``` text
temperature
top_p
top_k
max_tokens
stream
stop
presence_penalty
frequency_penalty
logit_bias
repeat_penalty
seed
```

and its documentation states that these recognized parameters are
honored. LM Studio also supports tools and structured JSON output.
citeturn0search0turn0search1turn0search4turn0search7

**Design implication:** LM Studio demonstrates why the application
should have a richer neutral sampling model than the current Vox
implementation.

## 3.4 Gemini

Google provides an OpenAI-compatible endpoint under:

``` text
https://generativelanguage.googleapis.com/v1beta/openai/
```

Gemini's compatibility layer supports Chat Completions, streaming,
tools/function calling, structured output, and `reasoning_effort`
mappings for supported thinking models. Google explicitly describes the
compatibility layer as a compatibility surface and recommends the native
Gemini API when not constrained to OpenAI client compatibility.
citeturn0search2

Gemini is particularly important for capability handling because
compatibility behavior is not identical to OpenAI semantics. Some
unsupported parameters can be silently ignored in compatibility
contexts. Therefore:

> HTTP 200 does not prove that a parameter was semantically honored.

**Design implication:** capability status must support `Unknown`;
probing cannot rely solely on HTTP status.

## 3.5 NVIDIA NIM

NVIDIA NIM exposes an OpenAI-compatible inference API backed by vLLM.

Current documented inference endpoints include:

``` text
POST /v1/chat/completions
POST /v1/completions
POST /v1/responses
GET  /v1/models
POST /tokenize
POST /detokenize
```

NIM documentation also exposes model discovery through `/v1/models`.
citeturn0search6turn0search10

**Design implication:** NIM should be modeled as a vLLM/OpenAI-style
adapter, but model/runtime-specific capability differences must still be
respected.

------------------------------------------------------------------------

# 4. Core Design Principle

## Do not use this

``` rust
struct OpenAiRequest {
    model: String,
    messages: Vec<Message>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    max_completion_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
    context_window: Option<u32>,
    reasoning_effort: Option<String>,
    ...
}
```

This simply creates a larger version of today's mess.

## Use this instead

``` rust
struct GenerationRequest {
    model: ModelId,
    input: ConversationInput,
    options: GenerationOptions,
    output: OutputConstraint,
    purpose: GenerationPurpose,
}
```

The provider adapter translates this into the provider's actual
protocol.

------------------------------------------------------------------------

# 5. Proposed Domain Model

## 5.1 Generation purpose

``` rust
enum GenerationPurpose {
    Conversation,
    MemoryCompaction,
    StructuredExtraction,
    Other,
}
```

Purpose is not merely metadata. It allows policy to select safe
defaults.

For example:

``` text
Conversation
  normal latency
  normal output budget
  conversational sampling

MemoryCompaction
  deterministic
  bounded output
  structured output required
  strict parse validation
```

------------------------------------------------------------------------

## 5.2 Generation options

``` rust
struct GenerationOptions {
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,

    max_output_tokens: Option<u32>,

    stop: Vec<String>,

    presence_penalty: Option<f32>,
    frequency_penalty: Option<f32>,

    seed: Option<u64>,

    reasoning: Option<ReasoningConfig>,

    verbosity: Option<Verbosity>,
}
```

### Important semantics

`max_output_tokens` is the **Vox-level concept**.

Adapters translate it:

``` text
OpenAI Chat Completions
    -> max_completion_tokens or model-appropriate field

OpenAI Responses
    -> max_output_tokens

LM Studio Chat
    -> max_tokens

Ollama native
    -> options.num_predict

Other providers
    -> provider-specific mapping
```

If a provider has no supported output cap, the adapter must explicitly
report that capability rather than silently pretending the cap exists.

------------------------------------------------------------------------

# 6. Context Management

## 6.1 Separate context capacity from generation request

``` rust
struct ModelCapabilities {
    context_window: Option<u32>,
    max_output_tokens: Option<u32>,
}
```

Do **not** put `context_window` into `GenerationOptions`.

The distinction is:

``` text
context_window
    = maximum capacity of the model/runtime

max_output_tokens
    = desired generation budget for this request

input token budget
    = context_window - reserved output budget
```

This lets Vox perform client-side context management without trying to
"set" a remote model's context window.

## 6.2 Local embedded model

For llama.cpp:

``` text
ModelRuntimeConfig
    ctx_size
    threads
    batch_size
    ubatch_size
```

These are runtime/model loading settings.

They are not generic LLM generation parameters.

## 6.3 Ollama

Do not hardcode:

``` json
"num_ctx": 8192
```

inside every request.

Instead introduce a runtime/model configuration layer:

``` rust
struct LocalRuntimeConfig {
    context_size: Option<u32>,
}
```

The Ollama adapter may map that to `options.num_ctx` when using the
native API.

------------------------------------------------------------------------

# 7. Capability Model

Capability discovery must be explicit.

``` rust
enum Support {
    Supported,
    Unsupported,
    Unknown,
}
```

Then:

``` rust
struct ProviderCapabilities {
    api: ApiCapabilities,
    generation: GenerationCapabilities,
    output: OutputCapabilities,
    model: ModelCapabilities,
}
```

Example:

``` rust
struct GenerationCapabilities {
    temperature: Support,
    top_p: Support,
    top_k: Support,
    stop: Support,
    seed: Support,
    penalties: Support,
    reasoning: Support,
}
```

``` rust
struct OutputCapabilities {
    max_output_tokens: Support,
    json_object: Support,
    json_schema: Support,
    tools: Support,
    streaming: Support,
}
```

------------------------------------------------------------------------

# 8. Capability Sources

Capability information should have provenance.

``` rust
enum CapabilitySource {
    StaticProviderKnowledge,
    OpenApiSchema,
    ModelMetadata,
    ActiveProbe,
    UserOverride,
}
```

A capability record should therefore look conceptually like:

``` rust
struct CapabilityObservation {
    support: Support,
    source: CapabilitySource,
    observed_at: DateTime,
}
```

This avoids the false assumption that one HTTP request can always
establish the truth.

------------------------------------------------------------------------

# 9. Capability Discovery Strategy

Use the following priority order.

## Level 1 --- Static provider knowledge

Maintain provider adapters with documented capabilities.

Example:

``` text
LM Studio /v1/chat/completions
    temperature = Supported
    top_p = Supported
    top_k = Supported
    max_tokens = Supported
    seed = Supported
    tools = Supported
```

This should be the default for known providers.

## Level 2 --- Provider/model metadata

Use endpoints such as:

``` text
GET /v1/models
```

where available.

NVIDIA NIM documents `/v1/models`; Gemini's OpenAI compatibility also
supports listing/retrieving models. citeturn0search6turn0search2

Metadata can provide model identity and, for some providers, richer
runtime information.

## Level 3 --- OpenAPI discovery

If a server exposes:

``` text
/openapi.json
/docs
```

the adapter may inspect it.

Do not assume these endpoints exist.

## Level 4 --- Active probing

Probe only when the capability remains unknown and the feature matters.

Example:

``` text
baseline request

temperature probe
top_p probe
max output probe
structured output probe
tools probe
reasoning probe
```

Never send all candidate parameters together. One unknown parameter per
probe makes failures attributable.

------------------------------------------------------------------------

# 10. Capability Probing Rules

A probe must be:

-   tiny;
-   cheap;
-   deterministic where possible;
-   isolated to one capability;
-   cached;
-   cancellable;
-   opt-in or performed during provider setup, not every generation.

### Example

For `temperature`:

``` json
{
  "model": "MODEL",
  "messages": [
    {"role": "user", "content": "Reply with OK"}
  ],
  "temperature": 0.2,
  "max_tokens": 4,
  "stream": false
}
```

For `tools`, send one minimal valid tool declaration.

For structured output, send a minimal JSON schema.

For output limits, request a tiny cap and verify the response metadata
when the provider exposes it.

------------------------------------------------------------------------

# 11. Probe Results Must Not Be Boolean

Do not implement:

``` rust
supports_temperature: bool
```

Use:

``` rust
enum Support {
    Supported,
    Unsupported,
    Unknown,
}
```

Why?

Because:

``` text
HTTP 400
```

may indicate an unsupported field, but may also mean malformed input or
another validation problem.

And:

``` text
HTTP 200
```

does not necessarily prove semantic support. Gemini's compatibility
layer is a concrete example where unsupported parameters may be ignored.
citeturn0search2

Therefore the probe should record:

``` rust
status_code
provider_error
response_observation
support
```

------------------------------------------------------------------------

# 12. Error Classification

Provider adapters should normalize errors.

``` rust
enum LlmError {
    Authentication,
    ModelNotFound,
    InvalidRequest,
    UnsupportedParameter {
        parameter: String,
    },
    ContextLimitExceeded,
    OutputLimitExceeded,
    RateLimited,
    Timeout,
    Transport,
    Provider {
        status: u16,
        message: String,
    },
    Parse,
    Cancelled,
}
```

The adapter is responsible for converting provider-specific HTTP/JSON
errors into these categories where the evidence is sufficient.

Do not classify every HTTP 400 as `UnsupportedParameter`.

------------------------------------------------------------------------

# 13. Request Translation

The core request should be translated by the adapter.

## OpenAI Chat Completions

Conceptually:

``` rust
GenerationOptions {
    temperature: Some(0.2),
    max_output_tokens: Some(512),
}
```

becomes approximately:

``` json
{
  "temperature": 0.2,
  "max_completion_tokens": 512
}
```

for models where that is the appropriate OpenAI field.

OpenAI documents model-dependent use of `max_completion_tokens` versus
legacy `max_tokens`. citeturn2search0

## OpenAI Responses

``` rust
GenerationOptions {
    max_output_tokens: Some(512),
    reasoning: ...
}
```

becomes:

``` json
{
  "max_output_tokens": 512,
  "reasoning": {
    "effort": "..."
  }
}
```

OpenAI currently uses `max_output_tokens` for Responses and documents
reasoning/verbosity controls for applicable models. citeturn2search0

## LM Studio

Translate to its supported Chat Completions fields, including
`temperature`, `top_p`, `top_k`, and `max_tokens` where applicable.
citeturn0search1

## Ollama

For native `/api/chat`, translate to Ollama's native `options` object
rather than pretending it is an OpenAI request.

## Gemini

Use the OpenAI-compatible layer only when useful. Prefer a dedicated
Gemini adapter if Gemini-specific capabilities become a product
requirement.

This is especially important for thinking controls, because Gemini maps
`reasoning_effort` onto its thinking configuration differently across
model generations. citeturn0search2

## NVIDIA NIM

Use its OpenAI/vLLM-compatible endpoints, while treating model/runtime
capabilities as authoritative. NIM currently exposes both Chat
Completions and Responses. citeturn0search6

------------------------------------------------------------------------

# 14. Structured Output Must Become Explicit

Replace:

``` rust
is_json_request = messages.iter().any(|m| {
    m.content.contains("JSON")
        || ...
});
```

with:

``` rust
enum OutputConstraint {
    Text,

    JsonObject,

    JsonSchema {
        name: String,
        schema: serde_json::Value,
        strict: bool,
    },
}
```

Then:

``` rust
GenerationRequest {
    ...
    output: OutputConstraint::JsonSchema { ... },
}
```

The provider adapter chooses:

``` text
OpenAI
  -> response_format / Responses text.format

LM Studio
  -> response_format.json_schema

Gemini
  -> compatible structured output mapping

Embedded llama.cpp
  -> grammar / constrained decoding if implemented
     otherwise explicit Unsupported
```

**Do not silently downgrade structured output to prompt-only JSON unless
the product explicitly allows that fallback.**

------------------------------------------------------------------------

# 15. Compaction Refactor

Compaction should become:

``` rust
GenerationRequest {
    purpose: GenerationPurpose::MemoryCompaction,

    options: GenerationOptions {
        temperature: Some(0.0),
        max_output_tokens: Some(COMPACTION_OUTPUT_BUDGET),
        ...
    },

    output: OutputConstraint::JsonSchema {
        ...
    },

    ...
}
```

The exact defaults should be configurable, but the important
architectural rule is:

> Compaction must explicitly request deterministic, bounded, structured
> generation.

This removes the current accidental coupling to normal-chat behavior.

------------------------------------------------------------------------

# 16. Policy Layer

Add a policy layer before the provider adapter.

``` rust
struct GenerationPolicy {
    conversation: GenerationDefaults,
    compaction: GenerationDefaults,
    extraction: GenerationDefaults,
}
```

Example:

``` rust
conversation:
    temperature = 0.2
    max_output_tokens = 512

compaction:
    temperature = 0.0
    max_output_tokens = 1024
    output = JsonSchema
```

The policy produces a provider-neutral request.

The provider adapter then handles compatibility.

This prevents provider-specific wire details from leaking into business
logic.

------------------------------------------------------------------------

# 17. Recommended Module Structure

Suggested Rust structure:

``` text
services/llm/
├── mod.rs
├── types/
│   ├── request.rs
│   ├── response.rs
│   ├── generation.rs
│   ├── capabilities.rs
│   └── errors.rs
│
├── policy/
│   ├── mod.rs
│   ├── conversation.rs
│   └── compaction.rs
│
├── providers/
│   ├── mod.rs
│   ├── embedded/
│   │   ├── mod.rs
│   │   ├── llama_cpp.rs
│   │   └── sampler.rs
│   │
│   ├── openai/
│   │   ├── mod.rs
│   │   ├── chat_completions.rs
│   │   └── responses.rs
│   │
│   ├── ollama/
│   │   ├── mod.rs
│   │   └── native.rs
│   │
│   ├── lm_studio/
│   │   └── mod.rs
│   │
│   ├── gemini/
│   │   └── mod.rs
│   │
│   └── nvidia/
│       └── mod.rs
│
├── capabilities/
│   ├── mod.rs
│   ├── discovery.rs
│   ├── probe.rs
│   └── cache.rs
│
└── compaction/
    ├── mod.rs
    └── service.rs
```

This is a target structure, not a requirement to create every file
immediately.

------------------------------------------------------------------------

# 18. Provider Trait

The provider trait should be capability-aware but should not expose
provider wire formats.

Proposed shape:

``` rust
trait LlmProvider {
    fn capabilities(&self) -> &ProviderCapabilities;

    async fn generate(
        &self,
        request: GenerationRequest,
        cancel: CancellationToken,
        tx: StreamSender,
    ) -> Result<GenerationResult, LlmError>;
}
```

Optional:

``` rust
trait CapabilityDiscovery {
    async fn discover_capabilities(
        &self,
        model: &ModelId,
    ) -> Result<ProviderCapabilities, LlmError>;
}
```

The key change is that **generation parameters travel with the
request**.

------------------------------------------------------------------------

# 19. Streaming

Streaming should be normalized above providers.

``` rust
enum StreamEvent {
    Started,
    TextDelta(String),
    ToolCallDelta(...),
    ReasoningDelta(...),
    Usage(Usage),
    Completed,
}
```

Adapters translate:

``` text
OpenAI SSE
Ollama stream
LM Studio stream
Gemini stream
llama.cpp token callbacks
```

into this common event model.

This prevents the application from becoming provider-aware.

------------------------------------------------------------------------

# 20. Model Registry

Introduce:

``` rust
struct ModelDescriptor {
    id: ModelId,
    provider: ProviderId,

    capabilities: ModelCapabilities,

    context_window: Option<u32>,
    max_output_tokens: Option<u32>,

    supports_streaming: Support,
    supports_tools: Support,
    supports_structured_output: Support,
    supports_reasoning: Support,
}
```

Cache this per:

``` text
provider + endpoint + model
```

and invalidate it when:

-   provider configuration changes;
-   model changes;
-   application version changes capability rules;
-   explicit refresh occurs.

------------------------------------------------------------------------

# 21. Configuration

Replace provider-specific scattered settings with two conceptual groups.

## User/product settings

``` rust
struct LlmSettings {
    provider: ProviderId,
    model: String,

    generation: GenerationDefaults,

    context: ContextPolicy,
}
```

## Runtime settings

``` rust
struct RuntimeSettings {
    embedded: EmbeddedRuntimeSettings,
    ollama: OllamaRuntimeSettings,
    ...
}
```

Example:

``` rust
struct ContextPolicy {
    target_input_tokens: Option<u32>,
    reserve_output_tokens: u32,
}
```

This lets Vox control its own context assembly without pretending it can
resize remote model context windows.

------------------------------------------------------------------------

# 22. Context Management Algorithm

For a model with known context capacity:

``` text
context_window
        -
reserved output budget
        =
maximum input budget
```

Then:

``` text
system prompt
+ conversation
+ retrieved memory
+ current turn
----------------
must fit input budget
```

If it does not fit:

1.  trim low-priority retrieved context;
2.  compact conversation history;
3.  retry;
4.  fail explicitly if still over limit.

For unknown remote context capacity, use conservative configured limits
and provider error handling.

Do not rely on the provider to discover this only after a request fails.

------------------------------------------------------------------------

# 23. Migration Plan

## Phase 1 --- Introduce neutral types

Add:

``` text
GenerationRequest
GenerationOptions
OutputConstraint
GenerationPurpose
ProviderCapabilities
Support
LlmError
```

No provider behavior changes yet.

## Phase 2 --- Pass request into `LlmProvider::generate`

Replace the current parameterless generation configuration with the
request object.

## Phase 3 --- Move current hardcoded defaults into policy

Preserve current behavior initially:

``` text
Ollama temperature = 0.2
LM Studio temperature = 0.2
Qwen embedded = current sampler
other embedded = greedy
```

The important change is that these become explicit policy defaults, not
hidden serialization behavior.

## Phase 4 --- Implement output budgets

Add `max_output_tokens` to the domain model.

Map it provider by provider.

## Phase 5 --- Implement explicit structured output

Remove message-string detection.

Compaction explicitly requests JSON schema/object output.

## Phase 6 --- Add capability registry

Start with static documented capabilities for:

``` text
Embedded
Ollama
LM Studio
OpenAI
Gemini
NVIDIA NIM
```

## Phase 7 --- Add active probing

Only probe unknown capabilities.

Cache results.

## Phase 8 --- Add Responses API

Implement Responses as a separate OpenAI-family transport.

Do not force Responses into the Chat Completions wire model.

## Phase 9 --- Remove hardcoded remote context values

Eliminate the current unconditional:

``` text
8192
```

and replace it with explicit runtime/model configuration.

------------------------------------------------------------------------

# 24. Compatibility Matrix to Maintain

The project should maintain a versioned compatibility table.

Initial conceptual matrix:

  -----------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  Capability    Embedded           Ollama Native      LM Studio        OpenAI Chat               OpenAI Responses      Gemini Compat                  NVIDIA NIM
  ------------- ------------------ ------------------ ---------------- ------------------------- --------------------- ------------------------------ -------------------------
  Streaming     Yes                Yes                Yes              Yes                       Yes                   Yes                            Yes

  Temperature   Model/sampler      Provider/model     Yes              Model-dependent           Model-dependent       Model-dependent                Model-dependent

  Top-p         Model/sampler      Yes                Yes              Model-dependent           Model-dependent       Model-dependent                Model-dependent

  Top-k         Runtime-specific   Runtime-specific   Yes              Not universal             Not universal         Native/model-specific          Runtime/model-specific

  Output cap    Runtime            Native option      `max_tokens`     `max_completion_tokens` / `max_output_tokens`   Compatibility/model-specific   Model/runtime-specific
                                                                       legacy                                                                         

  Tools         Runtime            Supported on       Supported        Supported                 Supported             Supported                      Supported
                implementation     compatible models                                                                                                  

  JSON object   Grammar/prompt     Provider/model     Supported        Supported/model dependent Structured output     Supported                      Model/runtime dependent
                dependent          dependent                                                                                                          

  JSON schema   Runtime grammar    Provider/model     Supported        Supported/model dependent Supported             Supported                      Model/runtime dependent
                needed             dependent                                                                                                          

  Reasoning     Model-specific     Model-specific     Supported on     Model-dependent           First-class on        `reasoning_effort` mapping     Model/runtime-dependent
                                                      compatible                                 supported models                                     
                                                      models                                                                                          

  Context       Runtime-specific   No generic OpenAI  Runtime/server   No                        No                    No                             Runtime/model config
  resize per                       field              config                                                                                          
  request                                                                                                                                             
  -----------------------------------------------------------------------------------------------------------------------------------------------------------------------------

This table is intentionally **not a promise of universal support**. It
is a starting registry that must be backed by provider documentation and
model metadata.

LM Studio's current documentation confirms its Chat Completions
parameter list and its Responses endpoint; Gemini documents OpenAI
compatibility and reasoning mappings; NVIDIA documents Chat Completions
and Responses; OpenAI documents separate token-control semantics for
Chat Completions and Responses.
citeturn0search0turn0search1turn0search2turn0search6turn2search0

------------------------------------------------------------------------

# 25. Testing Requirements

Every adapter should have contract tests for:

### Request serialization

-   temperature;
-   top_p;
-   top_k where supported;
-   max output;
-   stop;
-   seed;
-   reasoning;
-   tools;
-   JSON object;
-   JSON schema;
-   streaming.

### Capability behavior

For every capability:

``` text
Supported
Unsupported
Unknown
```

must be testable.

### Error normalization

Test:

``` text
400 invalid request
401 authentication
404 model
429 rate limit
5xx provider
timeout
context exceeded
```

and verify normalized `LlmError`.

### Compaction

Test that compaction:

-   explicitly requests structured output;
-   has an output budget;
-   does not depend on prompt-string inspection;
-   validates parsed JSON;
-   retries/fails predictably.

------------------------------------------------------------------------

# 26. Non-Goals

This refactor should **not** attempt to:

-   make every provider expose every OpenAI feature;
-   invent a fake universal context-window request parameter;
-   silently ignore unsupported user-requested parameters;
-   silently downgrade JSON schema to prompt-only JSON;
-   probe every provider capability on every startup;
-   make provider-specific JSON visible to application/business logic.

------------------------------------------------------------------------

# 27. Design Decisions

### Decision 1 --- Provider-neutral request

**Required.**

Business logic must express intent, not HTTP payloads.

### Decision 2 --- Capability-aware adapters

**Required.**

A provider/model may support only a subset of the neutral request.

### Decision 3 --- `Unknown` capability state

**Required.**

HTTP status alone is insufficient.

### Decision 4 --- Separate context capacity from output budget

**Required.**

Context window is a model/runtime capability; output budget is a
per-request preference.

### Decision 5 --- Explicit compaction mode

**Required.**

Compaction is not ordinary chat with a different prompt.

### Decision 6 --- Separate Chat Completions and Responses transports

**Required.**

They have materially different request semantics.

### Decision 7 --- Structured output as a first-class constraint

**Required.**

No more message-string inspection.

------------------------------------------------------------------------

# 28. Acceptance Criteria

The refactor is complete when:

-   [ ] `LlmProvider::generate` accepts a unified `GenerationRequest`.
-   [ ] Application code contains no provider-specific JSON field names.
-   [ ] Temperature is configurable through the neutral request/policy.
-   [ ] Output token budgets are configurable.
-   [ ] Context capacity is represented separately from output budget.
-   [ ] Embedded context size is represented as runtime configuration.
-   [ ] Ollama `num_ctx` is no longer hardcoded to 8192.
-   [ ] LM Studio context length is no longer hardcoded to 8192.
-   [ ] Compaction has its own generation policy.
-   [ ] Compaction explicitly requests structured output.
-   [ ] String-based JSON intent detection is removed.
-   [ ] Provider capability information is represented explicitly.
-   [ ] Capability state supports `Supported`, `Unsupported`, and
    `Unknown`.
-   [ ] Provider errors are normalized.
-   [ ] Chat Completions and Responses have separate transports.
-   [ ] OpenAI, Ollama, LM Studio, Gemini, and NVIDIA NIM can be
    represented without modifying the core request model.
-   [ ] Unsupported provider features are either omitted by policy or
    surfaced explicitly; they are never silently treated as supported.
-   [ ] Capability discovery is cached.
-   [ ] Adapter contract tests cover request serialization and failure
    modes.

------------------------------------------------------------------------

# 29. Final Recommendation

The central refactor is **not** "add more parameters to
`StandardOpenAi`."

It is:

``` text
CURRENT

Business logic
    ↓
LlmProvider
    ↓
provider-specific hardcoded JSON


TARGET

Business logic
    ↓
GenerationPolicy
    ↓
GenerationRequest
    ↓
Capability-aware ProviderAdapter
    ↓
Provider-specific protocol
```

This architecture solves the underlying problem:

> Vox defines **what it wants**; the provider adapter decides **how to
> ask for it**.

That gives the project room to support OpenAI Chat Completions, OpenAI
Responses, Ollama, LM Studio, Gemini, NVIDIA NIM, embedded llama.cpp,
and future providers without continually expanding one giant
provider-specific request struct.

------------------------------------------------------------------------

## 30. Research Sources

-   OpenAI --- controlling response length, token limits, reasoning, and
    verbosity: urlOpenAI Help Centerturn2search0
-   LM Studio --- OpenAI-compatible endpoints: urlLM Studio OpenAI
    Compatibilityturn0search0
-   LM Studio --- Chat Completions parameters: urlLM Studio Chat
    Completionsturn0search1
-   LM Studio --- tool use: urlLM Studio Tool Useturn0search4
-   LM Studio --- structured output: urlLM Studio Structured
    Outputturn0search7
-   Google --- Gemini OpenAI compatibility: urlGemini OpenAI
    Compatibilityturn0search2
-   Google --- API versioning: urlGemini API Versionsturn0search5
-   NVIDIA --- NIM API reference: urlNVIDIA NIM API
    Referenceturn0search6
-   NVIDIA --- NIM quickstart/model discovery: urlNVIDIA NIM
    Quickstartturn0search10

**Research date:** 2026-08-08.
