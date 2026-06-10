# AGENTS.md — Vox Repository Guide

> Compact instructions for AI agents working in this codebase. Every line answers: "Would an agent likely miss this without help?"

---

## Architecture (Non-Obvious Facts)

**Vox is a Tauri 2 desktop app with a Rust core library (`vox_lib`). Current version: v0.8.2 → v0.8.3 (Phase 9 — LLM Provider Architecture).**

- `app/` — Frontend workspace (React 19, Vite 7, TailwindCSS 4, TypeScript)
- `app/src-tauri/` — Tauri backend. The lib target is `vox_lib` (not `app`). The `vox_lib` crate contains ALL core logic.
- `app/src-tauri/src/services/` — Engine subsystems: `vad/`, `stt/`, `llm/`, `tts/`, `translit.rs`, `audio.rs`, `pipeline.rs`, `ptt.rs`, `playback.rs`, `utils.rs`
- `app/src-tauri/src/core/` — Shared core: `events.rs` (VoxEvent enum), `settings.rs` (VoxSettings), `state.rs` (InteractionState, InteractionOwner, PipelineAtomics), `constants.rs` (model paths, timing), `metrics.rs`
- `app/src-tauri/src/ipc/` — Tauri IPC command handlers: `pipeline.rs`, `settings.rs`, `tray.rs`, `history.rs`, `audio.rs`, `monitoring.rs`, `setup.rs`
- `app/src-tauri/src/persistence/` — SQLite session persistence (`rusqlite`)
- `app/src-tauri/src/monitoring/` — Telemetry aggregator, system monitor, runtime snapshots
- `app/src-tauri/src/setup/` — First-run onboarding logic
- `app/src-tauri/src/wizard.rs` — Setup wizard window configuration + model health checks
- `app/src-tauri/src/tray.rs` — Tray icon, overlay window management
- `app/src-tauri/src/services/tts/` — TTS engine. `supertonic.rs` is the sole TTS engine. Uses sherpa-onnx native `OfflineTtsSupertonicModelConfig` (99M params, 31 languages, INT8 quantized ~144MB). Progress callback must capture owned Arcs/Senders (`'static`). `actor.rs` dispatches TTS commands. `mod.rs` re-exports.
- `app/src-tauri/src/services/traits.rs` — Engine trait contracts (`VadEngine`, `SttEngine`, `LlmEngine`, `TtsEngine`). Pure sync interfaces; no thread/Tauri awareness.
- `app/src-tauri/src/services/stt/` — Two engines: `nemotron_onnx.rs` (primary, parakeet-rs Nemotron-3.5) and `qwen_onnx.rs` (legacy, sherpa-onnx Qwen3-ASR)
- `app/src-tauri/src/services/vad/` — Two backends: `earshot_vad.rs` (default, pure Rust) and `ten_onnx.rs` (legacy, sherpa-onnx ONNX)
- `app/src-tauri/src/bin/` — Standalone binaries: `tts-bench.rs`, `vox-bench.rs`, `test-translit.rs`
- `manifests/` — Model validation manifests (`app_manifest.json`, `models_manifest.json`).
- `scripts/` — Benchmark scripts (Python) and release/packaging scripts.
- `vox-models/` — Local model storage directory.

**Two separate Cargo workspaces** with independent `Cargo.lock` files:
1. `app/src-tauri/` (main app)
2. `app/src-tauri/plugins/tauri-plugin-positioner/`

**Frontend redesign (Phase 0/1 — Liquid Space design system, v0.8.3):**
- **Glass Elevation System & Page Transparency** — 4 levels using standard frosted glassmorphism (`backdrop-filter`) with elegant translucency. All page roots are transparent (`bg-transparent`) to let the underlying layouts' `AmbientBackground` and animated blobs show through. Specs: `.glass-whisper` (8px blur, 0.20 dark / 0.45 light), `.glass-surface` (16px blur, 0.45 dark / 0.65 light), `.glass-card` (24px blur, 0.65 dark / 0.80 light), `.glass-elevated` (40px blur, 0.85 dark / 0.92 light). Noise grain and sheen reside on `.glass-base::after`.
  - Old `.liquid-glass`, `.glass-card` (old), `.premium-card` classes removed from `index.css`.
- **AmbientBackground** (`app/src/shared/components/AmbientBackground.tsx`) — Pure CSS animated background with 3 organic blob shapes (border-radius keyframes, no canvas/WebGL), noise grain overlay, respects `prefers-reduced-motion`. Light mode variant.
- **useDynamicFPS hook** (`app/src/shared/hooks/useDynamicFPS.ts`) — RAF loop with frame-skipping algorithm. Three FPS tiers: Active (60fps), Idle (15fps), Sleeping/Paused (0fps). Reacts to `document.visibilityState`. Integrated into AdvancedOrb (Three.js) and LiveWaveform (Canvas 2D).
- **usePerformanceMonitor hook** (`app/src/shared/hooks/usePerformanceMonitor.ts`) — Debug-only FPS tracker (guarded by `import.meta.env.DEV`).
- **Package.json cleanup** — Removed 6 unused deps: `gsap`, `@react-three/fiber`, `@react-three/drei`, `sonner`, `radix-ui`, `class-variance-authority`.

**State management is zustand v5 (`app/src/store/`):**
- `settingsStore.ts` replaces `SettingsContext.tsx` as the single source of truth for all settings state.
- `SettingsContext.tsx` is now a thin adapter wrapper — existing `useSettings()` consumers still work unchanged.
- **New components should use `useSettingsStore(selector)` directly** for selective subscriptions and zero unnecessary re-renders.
  - Good: `const theme = useSettingsStore(s => s.draftSettings?.ui.theme || 'dark')` — only re-renders on theme change
  - Bad: `const { draftSettings } = useSettings()` — re-renders on ANY setting change
- `VoxSettings` type lives in `app/src/store/settingsStore.ts` and includes `audio` and `setup` domains matching the Rust backend.

---

## Documentation (`docs/`)

Architecture and design documents that provide deep context. Read the relevant one before making major changes:

| File | Covers |
|------|--------|
| `docs/features/voice-flow.md` | **End-to-end voice pipeline flow** — audio capture → VAD → STT → LLM → TTS → playback, all algorithms, metrics, and event flow. THIS is the single canonical reference for the voice pipeline. |
| `docs/backend.md` | Rust/Tauri audio pipeline, engine lifecycle, event flow |
| `docs/frontend.md` | Dual-surface UI (main app + ephemeral overlay/tray), IPC contracts |
| `docs/models.md` | Model stack (VAD, STT, LLM, TTS), hardware constraints, defaults |
| `docs/design.md` | Visual design system, color tokens |
| `docs/packaging.md` | Native desktop distribution strategy |
| `docs/roadmap.md` | Phased versioned roadmap |
| `docs/decision-framework.md` | Rationale behind major architectural decisions |
| `docs/benchmarks/` | Performance results and TTS comparison reports |
| `docs/plans/` | Phase-based implementation plans (phase3-phase9) |
| `docs/plan.md` | Post-stable feature ideas |

**Phase Plans (in `docs/plans/`):**
| File | Phase |
|------|-------|
| `phase3-tray-ux.md` | Tray/Overlay UX improvements |
| `phase4-pipeline.md` | Pipeline architecture |
| `phase5-realtime-ux.md` | Realtime interaction UX |
| `phase6-persistence.md` | Persistence & telemetry |
| `phase7-packaging-onboarding.md` | Packaging & onboarding |
| `phase8-ci-cd.md` | CI/CD pipeline |
| **`phase9-inference-expansion.md`** | **Current — LLM Provider Architecture (v0.8.3)** |

---

## Development Commands

### Rust checks (from app/src-tauri)
```bash
cd app/src-tauri
cargo check
```
### Run a single integration test
```bash
cd app/src-tauri
cargo test --test llm_family_test -- --nocapture
cargo test --test tts_test -- --ignored --nocapture --test-threads=1
```

### TTS benchmark (Rust)
```bash
cd app/src-tauri
cargo run --bin tts-bench bench
```
Output WAVs go to `docs/benchmarks/audio_outputs/`.

### Supertonic (sole TTS engine, INT8 via sherpa-onnx native)
7 INT8 model files at `~/.vox/models/tts/supertonic-3/` (flat, no subdirectories). Uses sherpa-onnx `OfflineTtsSupertonicModelConfig` with `GenerationConfig { sid, num_steps: i32, speed, extra: { "lang" } }`. Progress callback resamples 44.1→24kHz with anti-aliasing LPF (Biquad, fc=11000Hz, 2nd-order Butterworth). Model pack: `sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2`. Expression tags (`<laugh>`, `<breath>`, `<sigh>`) injected into LLM system prompt when engine is Supertonic (see `pipeline.rs` dynamic prompt logic).

---

## Critical Build Quirks

- **Linux requires clang** for the Tauri build (llama.cpp compilation). Env vars `CC=clang CXX=clang++` are set in CI and may be needed locally.
- **Linux system deps** (must be installed): `libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev patchelf libasound2-dev`
- **`app/src-tauri/.cargo/config.toml`** forces `-lc++ -lc++abi` linker flags on Linux and `FORCE:MULTIPLE` on Windows. Required for llama.cpp symbol resolution.

---

## Testing Conventions

- Integration tests in `app/src-tauri/tests/` often need **real model files** at `~/.vox/models/`. Many are `#[ignore]`-d and run with `--ignored`.
- Tests requiring actual audio hardware or models should use `--test-threads=1` to avoid resource contention.
- `vox-bench` is the full production-parity benchmark (STT + LLM + TTS + VAD pipeline). `tts-bench` is TTS-only.
- **Benchmark audio clips** are at `data/benchmark_clips/` as `hiacc_adult_test_AD09XXX.wav`. The 5-file benchmark suite uses clips `AD09001`, `AD09004`, `AD09021`, `AD09039`, `AD09051` (approx 5-16s each, Hindi multi-lingual). Run with: `cargo run --release --bin vox-bench -- --input /home/addy/projects/apps/vox/data/benchmark_clips/hiacc_adult_test_AD09001.wav --llm minicpm/minicpm5-1b-Q4_K_M.gguf --asr nemotron` from `app/src-tauri/`. Always use absolute paths for `--input`.


## Common Pitfalls

- **Forgetting `--test-threads=1`** on tests that load models or use audio hardware → race conditions or OOM
- **Running `cargo test` from repo root** — there's no root workspace. Always `cd` into `app/src-tauri`.
- **Hardcoded absolute paths** in code — use `dirs` crate or `vox_lib::utils::paths` instead
- **Modifying streaming/latency/VAD behavior** — these are critical-path invariants. Read `docs/features/voice-flow.md` and `docs/backend.md` thoroughly first.
- **`partial_tag_len()` UTF-8 char boundary (llama_cpp.rs):** The `partial_tag_len()` function detects incomplete emotion tags at the buffer end. It MUST iterate using `text.char_indices()` not `0..text.len()` — raw byte slicing (`&text[i..]`) panics on multi-byte UTF-8 characters (Devanagari `म` is 3 bytes). This was a crash bug introduced in v0.8.2 tag stripping rewrite; fixed by changing `for i in 0..text.len()` to `for (i, _) in text.char_indices()`.
- **`ort::session::Session` API quirks (2.0.0-rc.12):** `Session` is at `ort::session::Session`, not `ort::Session`. Builder methods return `ort::Error<SessionBuilder>` which is `!Send` — always use `.map_err(|e| anyhow!("{:?}", e))?` instead of bare `?`. Access input/output info via `session.inputs()` / `session.outputs()` methods (not fields). `GraphOptimizationLevel` is at `ort::session::builder::GraphOptimizationLevel`.
- **Adding new fields to settings structs:** Always add `#[serde(default)]` to the struct to avoid deserialization failures when loading old settings files missing the new field.
- **sherpa-onnx Supertonic native API quirks:** `OfflineTtsSupertonicModelConfig` has 7 fields: `duration_predictor`, `text_encoder`, `vector_estimator`, `vocoder`, `tts_json`, `unicode_indexer`, `voice_style`. `GenerationConfig::num_steps` is `i32` (cast `quality_steps as i32`). Progress callback is `FnMut(&[f32], f32) -> bool + 'static` — must capture owned Arcs/Senders, not references.
- **CPU-aware LLM thread presets:** ModelSettings.tsx computes LLM thread presets from `navigator.hardwareConcurrency`. Max safe = totalCores − 2 (reserving cores for system + other pipeline stages). Presets are generated dynamically: [2, 4] always, plus `maxSafe` and `totalCores` when they differ. Always guard with `typeof navigator !== 'undefined'` for SSR/SSG compatibility. Do NOT hardcode thread options.
- **CPU governor detection (Linux):** `utils::check_cpu_governor()` reads `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` at startup. If not `"performance"`, emits `cpu_governor_warning` Tauri event. Frontend (`Home.tsx`) listens and shows a dismissible warning banner. On non-Linux it's a no-op.
- **`should_flush()` word-boundary safety (utils.rs):** The time-based flush and word-count fallback both require `ends_at_word_boundary()` to avoid mid-word splits. `ends_at_word_boundary()` checks the last character of the buffer: it must be whitespace or punctuation (`.!?,;:)\]—–।`) for a flush to proceed. This prevents BPE subword tokens from being split mid-word. The algorithm is now **fully dynamic** — all thresholds are continuous functions of TPS (`lerp(tps_norm, min, max)`) with no hardcoded TPS categories. Clause flushing fades out between TPS 3.0–5.0. Time gate scales from 1.0s→3.5s. Word fallback scales 5→20 words. See `docs/backend.md` Section 11 for the full algorithm.
- **Nemotron STT `transcribe()` chunking (stt/nemotron.rs):** `transcribe()` chunks audio into 8960-sample windows, feeds them sequentially through the ONNX session, and only calls `reset_state()` at the very end (not between chunks). This prevents the model from forgetting context mid-utterance and produces more coherent Devanagari Hindi transcripts for multilingual clips.
- **Emotion tags confirmed working:** `<laugh>`, `<breath>`, `<sigh>` tags in TTS input are processed correctly by sherpa-onnx Supertonic (v1.13.2). The emotion tag test (`tts_test.rs`) confirmed: tags add detectable audio differences (avg diff=0.048, max diff=0.457 for `<laugh>` which adds 18% duration vs plain baseline). The upstream issue #148 is about the upstream Rust CLI runner, not the C++ sherpa-onnx wrapper used by vox.
- **Benchmark stability:** The 5-clip `vox-bench` suite (AD09001/004/021/039/051) achieves 100% completion with Llama-3.2-1B-Instruct Q6_K. Language detection via `is_devanagari()` correctly routes Devanagari STT transcripts to Hindi LLM prompts and English STT to English prompts. Average metrics: TTFA ~15.5-25.3s (higher with multi-utterance clips), LLM TPS ~1.2-3.2, STT RTF ~0.04-0.31, Peak RSS ~2446-2503MB, LLM mem ~969-970MB. The `vox-bench` binary now mirrors the real pipeline: dynamic prompt selection based on `is_devanagari()` + emotion tag injection (`<laugh>/<breath>/<sigh>`).

---

## Post-Task Protocol

After completing any task, an agent should:

1. **Update `AGENTS.md`** if the task revealed new build quirks, conventions, or pitfalls not already documented here.
2. **Update `docs/`** if the task changed architecture, pipeline behavior, model stack, or frontend contracts. Keep the relevant doc in sync:
   - Backend/pipeline changes → `docs/backend.md`
   - Voice pipeline/algorithm changes → `docs/features/voice-flow.md`
   - Frontend/UI changes → `docs/frontend.md`
   - Model changes → `docs/models.md`
   - New architectural decisions → `docs/decision-framework.md`
   - Phase plan progress → `docs/plans/phase9-inference-expansion.md` (current phase)
