# Service Architecture Standardization Checklist

## Batch 1: STT Standardization (`services/stt`)
- [x] 1.1 `mkdir -p app/src-tauri/src/services/stt/providers`
- [x] 1.2 `git mv` `nemotron.rs`, `qwen.rs`, `embedded.rs` into `services/stt/providers/`
- [x] 1.3 Author `services/stt/providers/mod.rs` (`SttProvider`, `SttEngine`, `SttProviderKind`)
- [x] 1.4 Author `services/stt/factory.rs` (`create_stt_provider`, `create_stt_instance_from_settings`)
- [x] 1.5 Surgically update `services/stt/providers/embedded.rs` internal imports
- [x] 1.6 Refactor `services/stt/mod.rs` to pure constants, module declarations, and re-exports
- [x] 1.7 Update `core/engine.rs` to call `stt::create_stt_instance_from_settings`
- [x] 1.8 Verify with `cargo check`

## Batch 2: VAD Standardization (`services/vad`)
- [x] 2.1 `mkdir -p app/src-tauri/src/services/vad/providers`
- [x] 2.2 `git mv` `silero_onnx.rs`, `ten_onnx.rs`, `earshot_vad.rs` into `services/vad/providers/`
- [x] 2.3 Author `services/vad/providers/mod.rs` (`VadEngine`, `VadBackend`, inherent dispatch methods)
- [x] 2.4 Move `VadCommand` from `services/vad/mod.rs` into `services/vad/actor.rs`
- [x] 2.5 Author `services/vad/factory.rs` (`create_vad_instance_from_settings`)
- [x] 2.6 Refactor `services/vad/mod.rs` to pure constants, module declarations, and re-exports
- [x] 2.7 Update `core/engine.rs` to call `vad::create_vad_instance_from_settings`
- [x] 2.8 Verify with `cargo check`

## Batch 3: LLM Standardization (`services/llm`)
- [x] 3.1 Author `services/llm/provider.rs` (`LlmProvider`, `LlmEngine`, `ProviderCapabilities`, `ProviderKind`, request/options types, `LlmError`, `LlmStreamEvent`, `global_llama_backend()`)
- [x] 3.2 Author `services/llm/factory.rs` (`create_llm_provider`, `create_llm_provider_from_llm_settings`)
- [x] 3.3 Surgically edit `services/llm/actor.rs` (remove factory functions, invoke `factory::create_llm_provider`)
- [x] 3.4 Refactor `services/llm/mod.rs` to pure constants, module declarations, and re-exports
- [x] 3.5 Verify with `cargo check`

## Batch 4: Final Validation & Clippy Gate
- [x] 4.1 Run `cargo clippy --all-targets` (must produce 0 warnings and 0 errors)
- [x] 4.2 Append milestone summary to `AGENTS.md` Section 5
- [x] 4.3 Copy internal artifacts to `docs/plans/phase11/service_standardization_plan.md` and `service_standardization_checklist.md`
