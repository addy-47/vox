# Service Architecture Standardization Plan (TTS, STT, VAD, LLM)

## 1. Executive Summary & Intent

The objective is to eliminate structural chaos and inconsistencies across the 4 core pipeline services (`services/tts`, `services/stt`, `services/vad`, and `services/llm`) while respecting each domain's architectural nuances:
- **TTS** is already the reference implementation for plugin-based backends: `providers/` holds the interchangeable synthesizers, `providers/mod.rs` defines `TtsProvider`, `factory.rs` handles resolution, `actor.rs` runs the worker loop, and `mod.rs` contains only constants and re-exports.
- **STT** currently has loose files (`nemotron.rs`, `qwen.rs`, `embedded.rs`) flat at root, with traits and factory logic trapped in `mod.rs`. It naturally maps to the provider plugin structure (`providers/` + `factory.rs`).
- **VAD** has loose engine files (`silero_onnx.rs`, `ten_onnx.rs`, `earshot_vad.rs`) flat at root, `VadCommand` misplaced in `mod.rs`, 60 lines of backend dispatch logic in `mod.rs`, no `factory.rs` (forcing `core/engine.rs` to do manual model path checking and fallback), and an overgrown `actor.rs` (709 LOC). It naturally maps to `providers/` + `factory.rs` + moving `VadCommand` into `actor.rs`.
- **LLM** is a dual-paradigm subsystem (`embedded/` for local GGUF/llama-cpp vs `transport/` for remote OpenAI/Ollama REST/SSE + `catalog/` for probe/discovery). It does **NOT** have a `providers/` directory, and mechanically forcing one breaks its domain taxonomy. Instead, LLM standardizes by extracting its provider trait and generation types into `provider.rs`, extracting factory instantiation out of `actor.rs` into `factory.rs`, and reducing `mod.rs` to pure declarations, constants, and re-exports.

---

## 2. Architectural Invariants Across All 4 Services

Every service adheres to these durable invariants from `backend-style-guide.md`:
1. **`mod.rs` has ZERO business logic (§2):**
   - Subsystem-level constants at top (`*_SAMPLE_RATE`, `*_CHUNK_SIZE`, model filenames/dirs).
   - Module declarations (`pub mod actor; pub mod factory; ...`).
   - Clean `pub use` re-exports of public types so external callers (`core/engine.rs`, `pipeline/`, `tests/`) use consistent `use crate::services::<domain>::{<Trait>, <Factory>, <Command>};`.
   - Zero trait definitions, zero factory logic, zero method implementations inside `mod.rs`.
2. **`factory.rs` owns instantiation and settings/path resolution (§2):**
   - Each service exposes a factory function converting settings into the boxed trait or backend enum.
   - `core/engine.rs` delegates initialization cleanly to each service's factory without embedding raw path checks or fallback loops in the engine bootloader.
3. **`actor.rs` owns the worker thread loop and command enum (§2, §6):**
   - Owns `<Service>Command` (`TtsCommand`, `SttCommand`, `VadCommand`, `LlmCommand`).
   - Owns channels, handles, thread priority, and execution loop.
   - Never embeds provider instantiation or disk discovery.
   - Kept strictly under the 600 LOC ceiling.
4. **No micro-file fragmentation:**
   - Every file must have a cohesive, load-bearing responsibility (no 30-50 LOC trivial files).

---

## 3. Per-Service Nuanced Specifications

### 3.1 `services/tts` (Reference Implementation — No Code Changes)
- `mod.rs` (61 LOC): Constants & clean re-exports.
- `actor.rs` (230 LOC): `TtsCommand`, worker loop, `spawn_tts_worker`.
- `factory.rs` (107 LOC): `create_tts_provider`, `resolve_reference_audio`.
- `providers/mod.rs` (51 LOC): `TtsProvider` trait, `TtsProviderKind`, `SynthesisContext`.
- `providers/`: `chatterbox.rs`, `chatterbox_remote.rs`, `edge_tts.rs`, `kokoro.rs`, `supertonic.rs`.
- `voice.rs` (558 LOC): Voice management.

### 3.2 `services/stt` (Adopt Provider Plugin Structure)
- **Problem:** `nemotron.rs`, `qwen.rs`, `embedded.rs` float at root; `SttProvider`, `SttEngine`, `SttProviderKind`, and `create_stt_provider` are in `mod.rs`.
- **Target Structure:**
  - `services/stt/providers/`:
    - `mod.rs` (~50 LOC): Defines `pub trait SttProvider: Send`, `pub trait SttEngine: Send + Sync`, `pub enum SttProviderKind`. Re-exports engines.
    - `embedded.rs`: Implements `SttProvider` wrapping `SttEngine`.
    - `nemotron.rs`: Implements `SttEngine` for Nemotron ONNX.
    - `qwen.rs`: Implements `SttEngine` for Qwen3-ASR ONNX.
  - `services/stt/factory.rs` (~65 LOC):
    - `create_stt_provider(config, model_path, num_threads) -> Result<Box<dyn SttProvider>, anyhow::Error>` (moved from `mod.rs`).
    - `create_stt_instance_from_settings(settings: &SttSettings, models_dir: &Path) -> Result<Box<dyn SttProvider>, String>` (encapsulates path resolution from `core/engine.rs`).
  - `services/stt/stitcher.rs` (213 LOC): Kept intact as domain algorithm module.
  - `services/stt/actor.rs` (420 LOC): Kept intact (`SttCommand`, handles, worker loop).
  - `services/stt/mod.rs` (~50 LOC):
    - Module declarations (`pub mod actor; pub mod factory; pub mod providers; pub mod stitcher;`).
    - Constants (`QWEN_ASR_MODEL_DIR`, `NEMOTRON_MODEL_DIR`, model filenames, timeouts).
    - Re-exports (`SttProvider`, `SttEngine`, `SttProviderKind`, `create_stt_provider`, `create_stt_instance_from_settings`, `spawn_stt_worker`, `stitch_transcripts`).

### 3.3 `services/vad` (Adopt Provider Plugin Structure & Extract Factory)
- **Problem:** `silero_onnx.rs`, `ten_onnx.rs`, `earshot_vad.rs` float at root; `VadCommand` is in `mod.rs`; `VadEngine` & 60 LOC dispatch methods are in `mod.rs`; `core/engine.rs` contains 90 lines of VAD path checks and fallbacks; `actor.rs` is 709 LOC (exceeds 600 LOC ceiling).
- **Target Structure:**
  - `services/vad/providers/`:
    - `mod.rs` (~90 LOC): Defines `pub trait VadEngine`, `pub enum VadBackend`, and `impl VadBackend` methods (`predict`, `noise_gate_multiplier`, `is_above_noise_gate`, `update_threshold`, etc.). Re-exports engines.
    - `silero_onnx.rs`: Silero ONNX engine.
    - `ten_onnx.rs`: Ten ONNX engine.
    - `earshot_vad.rs`: Pure-Rust Earshot engine.
  - `services/vad/factory.rs` (~70 LOC):
    - `create_vad_instance_from_settings(settings: &VadSettings, models_dir: &Path) -> Result<VadBackend, String>` (absorbed from `core/engine.rs` lines 130-218).
  - `services/vad/actor.rs` (~520 LOC):
    - Receives `VadCommand` (moved here from `mod.rs` to align with `tts/actor.rs`, `stt/actor.rs`, `llm/actor.rs`).
    - Helper methods for window validation trimmed to bring file well under 600 LOC.
  - `services/vad/telemetry.rs` (52 LOC) & `utils.rs` (143 LOC): Kept intact.
  - `services/vad/mod.rs` (~60 LOC):
    - Module declarations (`pub mod actor; pub mod factory; pub mod providers; pub mod telemetry; pub mod utils;`).
    - Constants (`MODEL_DIR_VAD`, `MODEL_FILE_VAD`, chunk sizes, sample rates).
    - Re-exports (`VadEngine`, `VadBackend`, `VadCommand`, `VadOperationalMode`, `spawn_vad_actor`, `create_vad_instance_from_settings`).

### 3.4 `services/llm` (Nuanced Subsystem Architecture)
- **Problem:** LLM already has clean domain folders (`embedded/`, `transport/`, `catalog/`). Mechanically forcing an artificial `providers/` directory makes no sense. However:
  - `mod.rs` (244 LOC) violates style guide by containing `LlmProvider` and `LlmEngine` traits, `LlmError`, `LlmStreamEvent`, `GenerationRequest`, `GenerationOptions`, `ProviderCapabilities`, `ProviderKind`, and `global_llama_backend()`.
  - `actor.rs` (418 LOC) contains `create_llm_provider` and `create_llm_provider_from_llm_settings`.
- **Target Structure:**
  - `services/llm/provider.rs` (~170 LOC):
    - Single cohesive hub for LLM provider contract and generation types:
      - `pub trait LlmProvider: Send + Sync`
      - `pub trait LlmEngine`
      - `pub struct ProviderCapabilities`, `pub enum Support`, `pub enum ProviderKind`
      - `pub enum GenerationPurpose`, `pub enum ReasoningMode`, `pub struct GenerationOptions`, `pub enum OutputConstraint`, `pub struct ConversationInput`, `pub struct GenerationRequest`
      - `pub enum LlmError`, `pub enum LlmStreamEvent`
      - `pub fn global_llama_backend() -> &'static LlamaBackend`
  - `services/llm/factory.rs` (~55 LOC):
    - `create_llm_provider_from_llm_settings(llm_settings: &LlmSettings, llm_path: &Path) -> Result<Box<dyn LlmProvider>, String>` (moved from `actor.rs`).
    - `create_llm_provider(settings: &VoxSettings, llm_path: &Path) -> Result<Box<dyn LlmProvider>, String>` (moved from `actor.rs`).
  - `services/llm/actor.rs` (~370 LOC):
    - Worker thread loop, `LlmCommand`, `spawn_llm_worker`, `warm_up_llm`, `cool_down_llm`.
    - Calls `factory::create_llm_provider`.
  - `services/llm/embedded/`, `services/llm/transport/`, `services/llm/catalog/`:
    - Preserved as the natural domain modules of the LLM subsystem!
  - `services/llm/mod.rs` (~70 LOC):
    - Module declarations (`pub mod actor; pub mod catalog; pub mod embedded; pub mod factory; pub mod provider; pub mod transport;`).
    - Domain constants (`QWEN_MODEL_DIR`, `GEMMA_MODEL_DIR`, batch size, timeouts).
    - Re-exports of `LlmProvider`, `LlmEngine`, `create_llm_provider`, `LlmCommand`, request types, and catalog types. Zero business logic.

---

## 4. Work Batches & CLI Execution Strategy

All file moves and renames will be performed using CLI commands (`git mv` / `mkdir`), ensuring token efficiency and clean git tracking without rewriting entire files.

### Batch 1: STT Standardization (`services/stt`)
1. CLI: `mkdir -p app/src-tauri/src/services/stt/providers`
2. CLI: `git mv app/src-tauri/src/services/stt/nemotron.rs app/src-tauri/src/services/stt/qwen.rs app/src-tauri/src/services/stt/embedded.rs app/src-tauri/src/services/stt/providers/`
3. Author `services/stt/providers/mod.rs` (declares submodules, defines `SttProvider`, `SttEngine`, `SttProviderKind`).
4. Author `services/stt/factory.rs` (defines `create_stt_provider` and `create_stt_instance_from_settings`).
5. Update `services/stt/providers/embedded.rs` imports (surgical update to `super::...`).
6. Update `services/stt/mod.rs` to pure constants and re-exports.
7. Update `core/engine.rs` to call `stt::create_stt_instance_from_settings`.
8. Verify with `cargo check`.

### Batch 2: VAD Standardization (`services/vad`)
1. CLI: `mkdir -p app/src-tauri/src/services/vad/providers`
2. CLI: `git mv app/src-tauri/src/services/vad/silero_onnx.rs app/src-tauri/src/services/vad/ten_onnx.rs app/src-tauri/src/services/vad/earshot_vad.rs app/src-tauri/src/services/vad/providers/`
3. Author `services/vad/providers/mod.rs` (declares submodules, defines `VadEngine`, `VadBackend`, inherent dispatch methods).
4. Move `VadCommand` from `services/vad/mod.rs` into `services/vad/actor.rs`.
5. Author `services/vad/factory.rs` (`create_vad_instance_from_settings` absorbed from `core/engine.rs`).
6. Update `services/vad/mod.rs` to pure constants and re-exports.
7. Update `core/engine.rs` to call `vad::create_vad_instance_from_settings`.
8. Verify with `cargo check`.

### Batch 3: LLM Standardization (`services/llm`)
1. Author `services/llm/provider.rs` containing `LlmProvider`, `LlmEngine`, `ProviderCapabilities`, `ProviderKind`, request/options types, `LlmError`, `LlmStreamEvent`, `global_llama_backend()`.
2. Author `services/llm/factory.rs` containing `create_llm_provider` and `create_llm_provider_from_llm_settings`.
3. Surgically prune `actor.rs` (remove factory functions, call `factory::create_llm_provider`).
4. Surgically prune `mod.rs` (pure declarations, constants, and re-exports).
5. Verify with `cargo check`.

### Batch 4: Final Validation & Clippy Gate
1. Run `cargo clippy --all-targets` (must produce 0 warnings and 0 errors).
2. Update `AGENTS.md` Section 5 with the completed milestone.
3. Synchronize `docs/plans/phase11/service_standardization_plan.md` and `service_standardization_checklist.md`.
