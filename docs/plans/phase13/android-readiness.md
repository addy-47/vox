# Vox Android Readiness — Summary & Next Actions

**Date:** 2026-10-14
**Device of record:** POCO `22101320I` (codename `redwoodin`), Android 14, `arm64-v8a`, MIUI/HyperOS
**Scope:** What Android platform work has been delivered, what remains, and whether the
remote-control feature can start.

> This document **summarises and points forward**. It does not replace
> [`android-platform-preparation-plan.md`](./android-platform-preparation-plan.md)
> (the detailed plan of record) or
> [`android-device-defect-fixes.md`](./android-device-defect-fixes.md).

---

## 1. Verdict

**Remote control cannot start yet.** Six items block it. They are architectural, not
defects — no amount of on-device testing clears them.

Section 6 lists every item with a mini-plan and a classification.

**Local mode is in better shape than previously recorded.** As of 2026-10-14 the phone
runs mic → VAD → STT → cloud LLM with zero panics. Only TTS is broken, and it is broken
globally rather than on Android (§5.1).

---

## 2. What has been done

A record of delivered work only.

### 2.1 Build & toolchain

- **Dual-target compile gate.** `app/src-tauri/scripts/verify-targets.sh` compiles desktop
  (`--all-targets --release -D warnings`) and Android (`--target aarch64-linux-android --lib
  --release -D warnings`) sequentially. It is the sole Android compile gate.
- **Toolchain environment isolated.** `scripts/android-env.sh` holds the NDK variables.
  Sysroot is injected inline via `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android`; `SYSROOT`
  is never exported.
- **Release APK links and packages.** `pnpm tauri android build --apk -t aarch64` produces a
  signed, installable APK with only `lib/arm64-v8a/`, `minSdk 24`, carrying `libvox_lib.so`,
  `libc++_shared.so`, `libonnxruntime.so` and the sherpa-onnx libraries. `turso`/`libsql`
  and `llama.cpp` are verified for arm64.
- **Single-ABI, reproducible builds.** The chatterbox-rs Android toolchain fix is upstream
  (`c9d17bd`); no local `[patch]` or submodule state is required to build from a fresh clone.
- **Iterative deploy tooling.** `scripts/android-deploy.sh` (build/sign/cycle/crash/logs/pull)
  and `scripts/android-dev.sh` (debug/dev/reverse/restart/screen/logs).
- **USB reverse HMR.** `adb reverse tcp:1420 tcp:1421` + Vite gives sub-second frontend
  iteration without an APK rebuild. Requires a **debug** build.
- **IPC registration invariant.** `tests/ipc_command_registration_test.rs` catches
  unregistered IPC commands and deleted `#[cfg(not(desktop))]` stub arms.

### 2.2 Platform gating

- **Desktop-only code excluded.** `enigo`, `arboard`, `tauri-plugin-positioner` and the tray
  sit behind platform facades; `router.rs` carries zero inline `#[cfg]`.
- **Remote provisioning is desktop-only.** `setup/remote_server.rs` is `#[cfg(desktop)]` with a
  typed mobile stub; the `<RemoteServerSetup>` panel is gated on `isDesktop()`. Remote
  *runtime* inference (plain HTTP) deliberately still works on mobile.
- **Android paths correct at boot.** `paths::init_from_app_data_dir(app.path().app_data_dir())`
  on mobile (Android has no `$HOME`; `dirs::home_dir()` returns `/`).
- **Manifest permissions.** `RECORD_AUDIO` and `MODIFY_AUDIO_SETTINGS` declared, matching
  wry 0.55's permission launcher.

### 2.3 Runtime enablers

- **Hardware EP.** `ort` `nnapi` enabled as a **target-scoped** dependency
  (`[target.'cfg(target_os = "android")'.dependencies]`). Verified by feature graph: 3 `nnapi`
  refs on Android, 0 on desktop.
- **Android context initialised.** `setup/android_init.rs` calls
  `ndk_context::initialize_android_context(vm_ptr, activity_ptr)` once, over JNI from
  `MainActivity`, behind a `Once`. Both JNI consumer sites fail soft.
- **R8 keep rules.** `gen/android/app/jni-keep.pro`; `build.gradle.kts` auto-globs `**/*.pro`.
- **Bounded frontend IPC.** `promiseWithTimeout` bounds `invoke()`; a backend panic becomes a
  reported error instead of a silent hang. A cancel affordance renders on timeout.
- **Resumable model downloads.** RFC-compliant `Range` handling, SHA-256 prefix hashing,
  retained `.tmp` on every failure, absolute progress, storage preflight, per-read idle
  timeout. 14 mutation-verified tests over a hand-rolled misbehaving HTTP harness.
- **Notifications.** `TransientAlert` facade picks exactly one sink — desktop OS daemon or an
  Android channel. Channels are write-once and never recreated.

### 2.4 Mobile UI

- Single-window architecture: mobile mounts the flow directly, no desktop window proxies.
- Edge-to-edge system bars; bottom navigation with ≥48dp targets; safe-area padding.
- 5-step linear onboarding wizard (`MobileHeroStep` … `MobileReadyStep`).
- Adaptive icons across 5 densities, background `#090D16`, safe-zone padded foreground.
- Monitoring route and dock disabled on Android; `AmbientBackground` isolated to Home.
- Permission step reports five honest states and never renders a bridge fault as a grant.
- Model step reports truthful per-model progress and surfaces named failures with retry.

### 2.5 Frontend optimisation (Batches 1–5)

216+ dead copy keys pruned · unified `ContextMenuShell` · static Framer fades migrated to CSS ·
infinite decor frozen on coarse pointers · redundant blurs demoted · virtual lists calibrated
(TanStack) · Zustand selectors optimised · Lenis disabled on coarse pointers · Google Fonts
`@import` replaced with bundled `@fontsource` · dead `recharts` dependency removed.

### 2.6 Frontend transport carrier (A4 delivered 2026-10-14)

- **Decoupled carrier layer.** `app/src/services/transport.ts` provides a swappable `Transport`
  abstraction covering `invoke`, `listen`, and `convertFileSrc`.
- **Zero raw Tauri imports across services.** All 11 frontend service files migrated behind `Transport`.
- **Architectural enforcement.** `Invariant 1b` in `app/src/test/invariants.test.ts` fails the build if
  any file other than `transport.ts` imports `@tauri-apps/api/core` or `@tauri-apps/api/event`.

### 2.7 Audio backend seam (A2 delivered 2026-10-14)

- **Swappable audio traits.** `services/audio/mod.rs` defines `AudioCapture` (push mono 16 kHz f32)
  and `AudioSink` (drain stereo 48 kHz f32).
- **Decoupled hardware capture.** `CpalAudioCapture` implements `AudioCapture`; `AudioStream`
  wraps `Box<dyn AudioCapture>` and supports custom/network/mock capture backends.
- **Decoupled hardware sink.** `CpalAudioSink` implements `AudioSink`; `PlaybackEngine` owns
  `Option<Box<dyn AudioSink>>` rather than a concrete CPAL stream.
- **Headless mock implementations.** `MockAudioCapture` and `MockAudioSink` enable headless
  pipeline and benchmark execution without audio hardware dependencies.

---

## 3. Documentation

`.agents/rules/android-style-guide.md` records **22 confirmed traps**, each with the
misconception, the symptom, and the rule. It is mandatory reading for Android-touching work
(AGENTS.md §4.2).

Highest-cost traps: `target_os = "linux"` is **false** on Android (61 dead blocks across 14
files) · a backend panic becomes a **silent frontend hang** · a `SAFETY` comment asserting an
initializer must be checked against `Cargo.lock`, not intuition · a release APK can never
hot-reload · `cargo check` compiles but never links.

---

## 4. Verified on device vs. not

Last full device pass: **2026-10-14**, POCO `22101320I`, debug APK.

| Capability | Status |
|---|---|
| APK installs, launches, reaches bootstrap | ✅ proven |
| Onboarding wizard completes | ✅ proven |
| Model download (667 MB) reaches disk | ✅ proven |
| Manifest + permission prompts | ✅ proven |
| `turso` / `llama.cpp` / `ort` load without crashing | ✅ proven |
| Oboe input stream opens | ✅ **proven 2026-10-14** — `44100Hz, 2 channels`, ingestion running |
| Output stream opens | ✅ **proven 2026-10-14** — `CPAL stream started (48000Hz stereo f32)` |
| Silero VAD loads and runs | ✅ **proven 2026-10-14** (sherpa-onnx, *not* `ort`/NNAPI) |
| STT transcription on device | ✅ **proven 2026-10-14** — Nemotron-3.5, 0.832 s latency, RTF 0.570 |
| Cloud LLM turn end-to-end | ✅ **proven 2026-10-14** — NVIDIA NIM replied, session auto-titled |
| Zero panics across a full session | ✅ **proven 2026-10-14** |
| **TTS playback to speaker** | 🔴 **FAILS** — Edge TTS HTTP 403 (see §5) |
| Full turn with audible speech | ⬜ blocked on TTS |
| `ort` NNAPI inference | ⬜ **not exercised** — VAD/STT both run on sherpa-onnx |
| Debug ↔ release parity (R8) | ⬜ unproven — Pitfall 18 |
| WebGL thermal headroom under soak | ⚪ measured only in a 3 s window (see §6.5) |

### 4.1 UI audit on device (CDP, 2026-10-14)

Measured on the real device WebView via `.agents/skills/review-ui/scripts/android_webview_audit.mjs`:

- WebGL 2.0 on **Adreno 642L** — hardware GPU, despite bootstrap logging "CPU Only"
- Viewport 392×872 @ dpr 2.75; safe-area insets resolve (top 35 px, bottom 16 px)
- No horizontal overflow
- Frame timing over 3 s: mean 9.13 ms, p95 8.5 ms, 2 long frames
- 194 DOM nodes; 0 console exceptions
- ⚠️ 9 touch targets under 48 px (28 px header buttons, 39 px catalog buttons)

---

## 5. Reference configuration (validated 2026-10-14)

The phone cannot hold the full model zoo. Working local-mode chain on POCO hardware:

| Stage | Provider | Size | Status |
|---|---|---|---|
| VAD | `silero_vad` (sherpa-onnx) | 0.6 MB | ✅ proven |
| STT | `nvidia_nemotron` (embedded, sherpa-onnx) | 682 MB | ✅ proven |
| LLM | `nvidia_nim` cloud, `nvidia/nemotron-3-super-120b-a12b` | 0 | ✅ proven |
| TTS | ~~`edge_tts`~~ | — | 🔴 **unavailable — see below** |
| Embedding | *none* | — | personal memory must be disabled |

**Total on-device download: ~683 MB** (TTS excluded until resolved).

### 5.1 🔴 Edge TTS is globally unavailable

`services/tts/providers/edge_tts.rs` fails with **HTTP 403** on every synthesis attempt.
This is **not** an Android regression and **not** a Vox bug. Isolation evidence:

| Client | Result |
|---|---|
| Vox on POCO (5G) | 403 |
| Vox parameters replayed from laptop | 400 (also rejected) |
| `edge-tts` 7.2.8 Python library | **403, identical failure** |
| Vox voices-list HTTPS endpoint | **200 OK** |

The token and the service are valid — Microsoft is refusing the **synthesis WebSocket**
specifically, for all clients. Seven `Sec-MS-GEC-Version` values (`1-139` … `1-144`) were
tried; all rejected, so the DRM token version is not the cause.

**Consequence:** the WIP remote doc assumes cloud components remain available on mobile.
For TTS they no longer do. Any working phone configuration needs a **local** TTS or a
**remote-box** TTS.

| Candidate | Size | Note |
|---|---|---|
| `supertonic` | 145 MB | sherpa-onnx — the same runtime that just proved VAD and STT |
| `zipvoice` | 195 MB | sherpa-onnx |
| `kokoro` | 366 MB | heaviest |
| `chatterbox_remote` | 0 MB | HTTP to a remote box; also a remote-execution prototype |

### 5.2 Other provider facts

- NVIDIA NIM **rejects** the `stream_usage` parameter with HTTP 400. Vox sends
  `stream_options`, and `nvidia_nim` has `stream_usage: false`, so this is already handled —
  but the preset flag is load-bearing.
- `nvidia/nemotron-nano-3-30b-a3b` is listed by `/v1/models` but returns **404**. List
  membership is not availability. Verified working: `nemotron-3-super-120b-a12b` and
  `nemotron-3.5-lightning-30b-a3b` (tool calls + streaming, ~0.5 s TTFB).
- The cloud **tool-support probe times out at 4 s** on the phone
  (`pipeline/assistant/session.rs:791`, `Duration::from_secs(4)`), while the same probe
  succeeds in ~1 s from the laptop. It fails open — tools stay enabled — so it is a
  warning, not a break. Distinct from the 60 s cold-model probe.

---

## 6. Next actions

Classification:

- 🔴 **PREREQUISITE FOR REMOTE** — remote cannot start without it
- 🟡 **ABSTRACTION SWAP ENABLER** — makes a component replaceable; no desktop logic changes
- 🟢 **NO DESKTOP IMPACT** — Android-local only
- ⚪ **DOES NOT BLOCK REMOTE** — quality/robustness; ships independently

---

### 6.1 🔴 A1 — Execution-environment boundary

**Why it blocks:** remote mode runs the Vox pipeline on another machine with no Tauri window.
That boundary does not exist. This *is* the feature, not a prerequisite to it.

**Confirmed couplings:**
- `services/harness/mod.rs:81` — `TurnExecutionRequest.app: AppHandle<R>`
- `core/state.rs:134` — `CheckMenuItem<Wry>`; `:136` — tauri `JoinHandle`

These are **compile-time type bounds**, not runtime configuration. Changing them is a
type-level refactor across the pipeline.

**Mini-plan**
1. Define a handle/event-source abstraction covering what the pipeline actually uses from
   `AppHandle` (emit event, resolve asset, read config).
2. Introduce a window-less implementation; keep the Tauri one as the existing behaviour.
3. Replace `AppHandle<R>` in `TurnExecutionRequest` and swap `CheckMenuItem<Wry>` /
   `JoinHandle` in `AppState` for the neutral types.
4. Prove with a headless entry point that boots the pipeline and completes a turn with no
   window — **no desktop behaviour change**.

---

### 6.2 🟢 A2 — Audio backend seam · ✅ DELIVERED 2026-10-14

**Delivered:**
- `AudioCapture` (start/pause) and `AudioSink` traits defined in `services/audio/mod.rs`.
- `CpalAudioCapture` hardware stream implementation; `AudioStream` refactored to encapsulate `Box<dyn AudioCapture>`.
- `CpalAudioSink` hardware stream implementation; `PlaybackEngine` refactored to store `Option<Box<dyn AudioSink>>`.
- Zero-dependency `MockAudioCapture` and `MockAudioSink` implementations for headless testing and benchmarks.
- Clean integration with dual-target verify-targets gate and nextest test suites.

---

### 6.3 🔴 A3 — Prove one full turn on hardware

**Why it blocks:** the only device run ever completed exposed a fatal uninitialised context.
Every downstream claim — Oboe, NNAPI, STT quality, TTS playback — rests on this.

**Status 2026-10-14: mic → VAD → STT → cloud LLM all proven. TTS is the single
remaining break.** See §6.8.

**Mini-plan**
1. Build a debug APK; install; push the Section 5 config.
2. Download `silero_vad` + `nvidia_nemotron` (~683 MB).
3. Engage; speak ~3 s; release.
4. Assert, in order: no `android context was not initialized` · Oboe input opened · VAD fired ·
   STT transcript · LLM tokens streamed · TTS audio played.
5. Record the first failure precisely rather than summarising the chain as failed.

---

### 6.4 🟢 A4 — Frontend transport carrier · ✅ DELIVERED 2026-10-14

**Delivered:**
- Single `Transport` interface in `app/src/services/transport.ts` implementing `invoke`, `listen`,
  and `convertFileSrc`.
- Default `TauriTransport` with runtime swapping (`setTransport()`).
- All 11 service files in `app/src/services/` moved behind `Transport`.
- `Invariant 1b` in `app/src/test/invariants.test.ts` guarantees zero product code outside `transport.ts`
  imports `@tauri-apps/api/core` or `@tauri-apps/api/event`.
- 61 vitest unit and invariant tests passing.

---

### 6.5 🟡 A5 — WebGL thermal headroom

**Why:** remote control does not remove render cost — the phone still draws the orb. Remote's
success criterion is "use Vox conversationally from the phone", which a throttling phone fails.

**Mini-plan**
1. Run `.agents/skills/review-ui/scripts/android_webview_audit.mjs --soak 300` on device.
2. Record frame p95 and DOM drift; capture battery temperature before/after.
3. Batch 9 (WebGL thermal tuning) against the measurements.

---

### 6.6 🟡 A6 — Config migration for existing installs

**Why:** `tts.voice_index` → `tts.voice_id` shipped with **zero backwards-compatibility shim**
(`storage-spec.md:125`). Any device configured before 2026-10-13 has a config that no longer
deserialises cleanly. Not a blocker for remote, but it is a real installed base.

**Mini-plan:** decide between a one-shot forward migration on load, or accept that pre-2026-10-13
installs must be reconfigured. Record the decision in `storage-spec.md` first.

---

### 6.7 ⚪ A7 — Release parity smoke test

**Why:** debug and release are **not** at parity — release sets `isMinifyEnabled = true`.
A debug-only pass does not clear the shipped artifact.

**Mini-plan:** `./scripts/android-deploy.sh rebuild`, then repeat A3. Budget ~10 min.

---

### 6.8 🔴 A8 — Restore working TTS on Android

**Why it blocks:** a phone that cannot speak is not a Vox client. This blocks the remote
feature's own success criterion ("use Vox conversationally from the phone") and it blocks
the local-mode proof in A3.

Edge TTS is 403 globally (§5.1). A decision is required.

**Mini-plan**
1. Pick a replacement: local sherpa-onnx TTS (`supertonic`, 145 MB) or `chatterbox_remote`
   against the remote GPU box.
2. Configure it and re-run A3. Assert **audible** speech, not just a non-empty buffer.
3. Record the choice in `providers-spec.md` before coding.
4. Decide separately what `edge_tts` does now — keep the provider and surface a clear
   error, or retire it. Do not leave it silently failing.

**Swappable by design:** TTS is already behind a provider factory
(`[TTS Factory] Initializing EdgeTTS provider`). Switching providers is a config change
with no pipeline edit — which is why A8 is tractable.

---

### 6.9 🟡 A9 — Fix `android-dev.sh` HMR traps

**Why:** Android-only tooling, but it costs a full build cycle and a blank-screen
diagnosis every time. No product code is touched.

Delivered 2026-10-14: the script header now documents both traps, `debug` states plainly
that it is not HMR-capable, and `dev` passes `--host 127.0.0.1`.

**Remaining:** decide whether to keep the frontend-less `debug` APK at all, or remove it
in favour of `dev` alone.

---

## 7. Remote feature entry gate

Remote control may begin when **all** of the following are true:

- [ ] A1 — a headless pipeline entry point completes a turn with no Tauri window
- [x] A2 — audio capture and playback are behind swappable traits
- [ ] A3 — one full turn proven on hardware
- [x] A4 — the frontend reaches the backend through a swappable carrier
- [ ] A user decision recorded on pairing, discovery, authentication and reconnect
      (all four remain open questions in `wip/remote-control-android.md`)

A5, A6 and A7 do **not** gate remote work.

---

## 8. Related documents

- [`android-platform-preparation-plan.md`](./android-platform-preparation-plan.md) — detailed plan of record
- [`android-device-defect-fixes.md`](./android-device-defect-fixes.md) — on-device defect register
- [`../wip/remote-control-android.md`](../wip/remote-control-android.md) — the feature
- [`../../.agents/rules/android-style-guide.md`](../../.agents/rules/android-style-guide.md) — 22 confirmed traps