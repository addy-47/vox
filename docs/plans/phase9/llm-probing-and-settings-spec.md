# 📄 Architectural Specification: LLM Capability Probing, Provider Differentiation & Settings Decoupling

> **Status**: Approved for Implementation  
> **Phase**: Phase 9  
> **Target Subsystems**: `src-tauri/src/services/llm/`, `src-tauri/src/ipc/settings/`, `app/src/shared/components/settings/models/`, `app/src/shared/lib/`  
> **Key Invariants**: Zero Guessing, Zero Stale Hardcoded Registries, Accurate Streaming Probing, Layman CPU Thread Abstraction, Decoupled Component Architecture.

---

## 1. Context & Problem Statement

Vox's LLM subsystem historically treated all models with a single monolithic schema. This led to four architectural bugs:

1. **Shallow & Broken Capability Probing**:
   - Remote cloud endpoints (such as NVIDIA NIM, Groq, OpenRouter, and vLLM) were falsely routed through Ollama-only HTTP endpoints (`/api/show`, `/api/ps`).
   - When these endpoints returned HTTP 404, the system fell back to hardcoded `4096 ctx`, incorrectly reported `CPU Only`, and failed to compute TPS on blocking requests.
   - Tool calling probes sent generic conversational prompts (`"What is the weather in Tokyo?"`) without `tool_choice`, causing models to output plain English text instead of structured function call objects.

2. **Generic & Stale Settings Schema**:
   - The LLM Settings tab forced raw CPU thread dials (`[2, 4, 8, 12]`) and tiny context token buttons (`[512, 1024, 2048, 4096, 8192]`) even when connected to remote cloud clusters with 128k+ context windows and remote GPU execution.
   - Hardcoded context window registries become stale within weeks as new models and quantization variants release exponentially.

3. **Component Coupling & Code Duplication**:
   - `LlmCatalogView.tsx` conflated model discovery, 2-column catalog rendering, capability probing, search filtering, and generation parameters inside a single 580-line file.
   - The `fzf`-style fuzzy matching algorithm was embedded directly inside the view rather than living in a shared utility module.

---

## 2. Core Architectural Principles

### 2.1 The Zero-Guessing Rule (No Stale Static Registries)
* **Never hardcode static model lookup tables in source code.**
* If an endpoint exposes its native context window via metadata (e.g. Ollama `/api/show` $\to$ `model_info["context_length"]`, or standard response headers), Vox extracts and verifies that exact number.
* If an endpoint does **not** expose its context window (such as standard OpenAI `/v1/models` endpoints), Vox explicitly reports **`Endpoint Managed`** or **`Provider Default`** without inventing artificial numbers (like `4096`).

### 2.2 Output Token Budget: Provider Native by Default + Runtime Smoke Testing
* **Default Request Payload**: `max_tokens: null` (omitted from API payload). The server generates naturally up to its native architectural limit (e.g. Gemini 2.0's 65,536 tokens, GPT-4o's 16,384 tokens).
* **Intent-Based Voice Presets**:
  - `Native Max (Default)`: Omitted / `null` (uncapped).
  - `Voice Concise`: `300` tokens (low-latency conversational turns).
  - `Conversational`: `1,000` tokens.
  - `Custom Cap`: Open integer input with **no arbitrary client-side ceiling**.
* **Runtime Smoke Validation (<100ms)**:
  - When the user inputs a custom cap (e.g. `100,000`), a 1-token probe is sent to the server.
  - If the server accepts $\to$ `Valid ✓`.
  - If the server rejects with HTTP 400 (e.g. `"Value 100000 is greater than maximum allowed 16384"`), Vox parses the error message, surfaces a clear warning badge (*"Server maximum is 16,384 tokens"*), and renders a 1-click **`[Auto-clamp to 16,384]`** action.

### 2.3 Local CPU Thread Abstraction
* Raw integer buttons (`[2, 4, max, all]`) are replaced with intelligent hardware profiles:
  - **`Auto (Optimal)`** *(Default)*: Automatically computed as `std::cmp::max(2, available_parallelism() - 2)` to guarantee audio engine and UI render loop headroom.
  - **`Power Saver`**: Minimal allocation (`std::cmp::max(1, available_parallelism() / 2)`), preserving laptop battery and keeping thermals cool.
  - **`Max Performance`**: Full CPU core allocation for dedicated desktop inference.
* In **Remote / Cloud mode**, thread controls are **completely removed**.

---

## 3. Subsystem Specifications

### 3.1 Subsystem 1: Shared Fuzzy Utility (`shared/lib/fuzzy.ts`)

Extract the `fzf`-style subsequence matching and multi-term scoring engine into a standalone utility:

```typescript
export interface FuzzyMatchResult {
  matches: boolean;
  score: number;
}

export function fuzzyMatch(pattern: string, target: string): FuzzyMatchResult;
export function fzfScoreItem<T>(terms: string[], item: T, getFields: (item: T) => string[]): number | null;
```

**Scoring Invariants**:
- Exact matches: `+1000`
- Substring matches: `+500`
- Word boundary matches (following `/`, `-`, `_`, `.`, or camelCase): `+30` per character
- Consecutive match streak: `+15 * streak`
- Gap distance penalty: `-min(gap * 2, 20)`

---

### 3.2 Subsystem 2: High-Fidelity Streaming Capability Probe (`capability_probe.rs`)

#### Phase 1: SSE Streaming Inference Probe
1. **Endpoint Classification**:
   - Check if `base_url` or `provider_name` matches known cloud hosts (`integrate.api.nvidia.com`, `groq.com`, `openrouter.ai`, `together.xyz`, `deepseek.com`, `mistral.ai`, `openai.com`, `googleapis.com`).
   - If Cloud API $\to$ set `gpu_status = "Cloud GPU/TPU Cluster"`, `server_has_gpu = true`, `is_gpu_accelerated = true`, and skip local Ollama `/api/show` / `/api/ps` calls.
2. **Streaming TTFT & TPS Measurement**:
   - Send `POST /v1/chat/completions` with `"stream": true` and payload:
     ```json
     {
       "model": "<target_model_id>",
       "messages": [{"role": "user", "content": "Write 'नमस्ते' in Devanagari and 'Hello' in English."}],
       "max_tokens": 50,
       "temperature": 0.1
     }
     ```
   - Record elapsed time to first chunk $\to$ **`ttft_ms`** (true Time to First Token).
   - Record time between first chunk and final chunk $\to$ **`tps = tokens_received / stream_duration`** (pure inter-token generation throughput).
   - Check stream content for Devanagari unicode range (`\u0900..\u097F`) $\to$ **`supports_devanagari`**.

#### Phase 2: Strict JSON Schema Function Calling Probe
1. Send structured function definition with `tool_choice: "auto"`:
   ```json
   {
     "model": "<target_model_id>",
     "messages": [{"role": "user", "content": "Fetch database record for user ID 402."}],
     "tools": [{
       "type": "function",
       "function": {
         "name": "lookup_user",
         "description": "Retrieves user record by integer ID",
         "parameters": {
           "type": "object",
           "properties": { "user_id": { "type": "integer" } },
           "required": ["user_id"]
         }
       }
     }],
     "tool_choice": "auto",
     "max_tokens": 80
   }
   ```
2. Verify response contains `choices[0].message.tool_calls` where `function.name == "lookup_user"` and arguments parse to `{"user_id": 402}` $\to$ **`supports_tools = true`**.

#### Phase 3: Self-Hosted Metadata Verification (Ollama/vLLM only)
- If endpoint is `localhost` / `127.0.0.1`:
  - Query `/api/show` $\to$ extract `model_info["context_length"]` if present.
  - Query `/api/ps` $\to$ extract `size_vram` to report `"GPU Accelerated (VRAM: X MB)"`.

---

### 3.3 Subsystem 3: Frontend Component Decoupling

#### Component Structure

| Component | Path | Responsibility |
|---|---|---|
| **`LlmCatalogView.tsx`** | `src/shared/components/settings/models/LlmCatalogView.tsx` | Pure model catalog rendering, 2-column grid, search input with `fuzzy.ts`, capability test trigger, custom model ID validator. |
| **`LlmSettingsView.tsx`** | `src/shared/components/settings/models/LlmSettingsView.tsx` | Pure generation parameters and engine settings. Switches between **Mode A (Local GGUF)** and **Mode B (Remote / Cloud)**. |
| **`ModelsCard.tsx`** | `src/shared/components/settings/models/ModelsCard.tsx` | Parent controller mounting `LlmCatalogView` on `"model"` tab and `LlmSettingsView` on `"settings"` tab. |

#### `LlmSettingsView.tsx` UI Layout

```
+-------------------------------------------------------------------+
|  LLM ENGINE CONFIGURATION                                         |
+-------------------------------------------------------------------+
|  [Mode A: Local GGUF]                                             |
|  Hardware Allocation:   [ Auto (Optimal) ]  [ Power Saver ] [ Max ]|
|  RAM Context Budget:    [ 2k ]  [ 4k ]  [ 8k ]  [ 16k ]           |
|  Temperature:           [ Precise 0.2 ] [ Balanced 0.7 ] [ Creative]|
+-------------------------------------------------------------------+
|  [Mode B: Remote / Cloud Server]                                  |
|  Response Length Cap:   (o) Native Max (Default - Uncapped)       |
|                         ( ) Voice Concise (~300 tokens)           |
|                         ( ) Conversational (~1000 tokens)         |
|                         ( ) Custom: [ 16384 ] [ Verify & Probe ]  |
|  Sampling Temperature:  [ Precise 0.2 ] [ Balanced 0.7 ] [ Creative]|
|  Request Timeout:       [ 10s ]  [ 30s ]  [ 60s ]                 |
|  Endpoint Specs:        Context: Managed | Cloud Acceleration     |
+-------------------------------------------------------------------+
```

---

## 4. Implementation Steps

1. **Step 1: Extract `shared/lib/fuzzy.ts`**
   - Create generic `fuzzyMatch` and `fzfScoreItem` utilities.
   - Refactor `LlmCatalogView.tsx` to import from `shared/lib/fuzzy.ts`.

2. **Step 2: Upgrade Backend Capability Prober (`capability_probe.rs`)**
   - Implement SSE streaming client for accurate `ttft_ms` and `tps`.
   - Update function calling test with `lookup_user` schema and `tool_choice: "auto"`.
   - Classify cloud domains to report Cloud GPU Clusters and skip 404-prone Ollama endpoints.
   - Remove hardcoded `4096` context window fallback.

3. **Step 3: Create Decoupled `LlmSettingsView.tsx`**
   - Build Mode A (Local GGUF: Auto/Power/Max thread profiles + RAM context).
   - Build Mode B (Remote Cloud: Native Uncapped / Voice Presets / Custom with Smoke Probe).
   - Wire smoke probe IPC call to validate custom token limits.

4. **Step 4: Clean Up `LlmCatalogView.tsx` & `ModelsCard.tsx`**
   - Remove settings code from `LlmCatalogView.tsx`.
   - Mount `LlmSettingsView` in `ModelsCard.tsx` when `activeCategoryTab === "settings"`.

5. **Step 5: End-to-End Verification**
   - Test live capability probe against NVIDIA NIM (`meta/llama-3.1-8b-instruct`).
   - Verify `pnpm build` (`tsc && vite build`) and `cargo check`.
   - Sync `AGENTS.md` and documentation.
