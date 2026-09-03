# Phase 11 — Recent Work & Test Suite Ledger

This document tracks test engineering, verification milestones, and performance evaluations for Phase 11.
For high-level workspace invariants and active guidelines, refer to [AGENTS.md](file:///home/addy/projects/apps/vox/AGENTS.md).

> 📖 **Phase 10 Archive:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

---

## Phase 10 Summary

> Compact summary of Phase 10 outcomes to provide context for Phase 11 work.

- **4-Domain Pipeline Refactor:** Unified and decomposed pipeline event routing across all 4 interaction domains (Modular Passive, Modular PTT, Realtime Passive, Realtime PTT) under a single-FIFO elevated router (`pipeline/router.rs`) and modular handlers (`pipeline/handlers/*.rs`).
- **Realtime WebSocket Driver Decomposition:** Extracted shared WebSocket reconnect harness (`services/realtime/transport/{connection.rs, health.rs, mod.rs}`) and decomposed monolithic `gemini_live.rs` / `deepgram_live.rs` into modular `ProviderDriver` drivers (`providers/gemini/`, `providers/deepgram/`, all files ≤ 312 LOC).
- **Frontend Home Controls Alignment:** Aligned frontend interaction state machine and Home page controls across all 4 domains; fixed `VoiceSessionContext::resume` for `Error` state resumption; extracted copy to `src/data/homeCopy.ts`.
- **Hot-Path Zero-Allocation Buffer Pools:** Implemented zero-allocation recycle pools for VAD 62.5Hz passthrough and partial STT streaming; bounded CPAL input buffers; bounded realtime streaming queues (`BRIDGE_CHANNEL_CAPACITY = 100`).
- **`services/audio/` Architecture Separation:** Decomposed monolithic `playback.rs` (532 LOC) into `services/audio/{mod, device, sink, playback}.rs` with clean single-responsibility separation.
- **Quality Baseline:** All Phase 10 work delivered with zero `cargo clippy` warnings, zero `cargo check` errors, and 100% release-mode test suite green.

---

## Phase 11 Roadmap: Comprehensive Verification Suite
1. **Critical Isolated Unit Tests (UT)**: Algorithm verification, math identities, hot-path buffers, state transitions.
2. **Integration Test Suite (IT)**: Old End-to-end seam testing spec built before phase 10  (`integration_test_spec.md`) covering modular/realtime pipelines, IPC handlers, memory waterfalls.
3. **Performance Benchmarks**: Microbenchmarks (`cargo test --benches`) for STT RTF, TTS latency, and VAD audio ingestion under `--release`.
4. **Model & Pipeline Evaluations**: Failure distribution benchmarks, precision/recall on retrieval and speech segmentation.

---

## Chronological Implementation Ledger

- **Phase 10 Concluded:** Successfully completed the 4-domain pipeline refactor, realtime WebSocket driver decomposition, frontend Home controls alignment, hot-path zero-allocation buffer pools, and `services/audio/` architecture separation with zero clippy warnings.
- **Phase 11 Initialized (2026-09-02):** Established `docs/plans/phase11/` focused on full test suite engineering: Critical Isolated Unit Tests (UT), Integration Test Seams (IT via `integration_test_spec.md`), performance benchmarks, and pipeline evaluations.
- **2026-09-03 Docs Refresh:** Full rewrite `docs/features/voice-flow.md` from domain-partitioned (5 domains) to handler event-driven (`pipeline/handlers/*` + `pipeline/dictation.rs` + `pipeline/router.rs`); surgical updates: `docs/backend.md` (§0, §1, §3 pipeline diagram, §4.4 Kokoro, §8 VoxEvent, §9 watch state), `docs/features/dictation.md` (router fast-path, DictationState tray mapping), `docs/frontend.md` (handler ref, §9 event registry), `docs/models.md` (Kokoro row, 5-col TTS matrix).
- **2026-09-03 Launch Jitter Fix (RCA → root cause: 3 layers, fixed in 3 commits):** First-launch jitter "minimised → expanded" flash had two contributing causes. **(1)** `app/src-tauri/tauri.conf.json` created the window with `visible: false` + `maximized: true` + `center: true` + redundant `width/height` — on most compositors this put the window in a withdrawn state, and the first show() triggered a full withdrawn→normal→maximized transition. **(2)** `app/src/App.tsx`'s `getCurrentWindow().show()` was gated behind `getOnboardingStatus()` (1-3s wait, dev-mode StrictMode doubles it) and was rejected by the capability layer because `core:window:allow-show` was missing from `app/src-tauri/capabilities/default.json`. **(3)** `app/src/shared/context/VoiceSessionContext.tsx` then issued a fallback `showMainWindow()` IPC 300ms later, which re-fired the show on an already-maximized surface. **Fix:** (a) `tauri.conf.json` `visible: false` → `visible: true` so the window is born visible+maximized; (b) `app/src-tauri/src/lib.rs` setup hook now calls `main_win.show() + set_focus()` **synchronously** at the very start, before any async bootstrap — Rust is the single authoritative reveal point; (c) App.tsx `show()` block and VoiceSessionContext 300ms IPC fallback both removed. Capability `core:window:allow-show` retained for any future frontend show paths. Diagnostic `[DIAG:LAUNCH-WINDOW-SEQ]` markers remain in `app/src/App.tsx` and `app/src-tauri/src/lib.rs` for user validation.
- **2026-09-03 Boot Smoothness Round 2 (2 remaining flashes):** After Round 1 the user reported the WM flash was gone but two new flashes appeared: (a) a brief white native-window background before the WebView painted, and (b) the OrbitalLoader rendered in default cyan and then swapped to the user's configured accent_seed once `applyAppearance()` ran after `getSettings()` resolved. **Fix A:** `tauri.conf.json` now sets `backgroundColor: "#050505"` so the native window paints the same color as the WebView's inline background, eliminating the white pre-paint flash. **Fix B:** `app/src/store/settingsStore.ts` `applyAppearance()` now persists `{theme, accent_seed}` to `localStorage["vox_appearance"]`; `app/index.html` has a new boot inline `<script>` that reads that key and applies `--accent` + `data-theme` to `documentElement` before React mounts, so the loader paints in the configured color from frame 1.
- **2026-09-03 Boot Smoothness Round 3 (perceived boot latency):** User reported a 1-2s black screen before the loader appeared (perceived as "app is slow to start"). Root cause: Vite dev cold start + React tree mount happens *after* the WebView first paints, so there is a 1-2s window where the WebView is showing a black background. **Fix:** `app/index.html` now contains a pure-CSS animated `#vox-boot-loader` div (no JS, no SVG, no fetched assets) that paints from the first WebView frame. The loader uses the same accent color (`var(--accent)`) as the React OrbitalLoader so the transition is visually identical. `app/src/App.tsx` mount effect calls `window.__VOX_HIDE_BOOT_LOADER()` to fade and remove the inline loader once React is ready. **Secondary:** `installOverlayStack()` in App.tsx is now deferred to `requestIdleCallback` (with `setTimeout(200ms)` fallback) since the overlay system is not needed until the first user interaction with a drawer/popover. `MemoryProfilerProvider` was audited and confirmed **not** safely deferrable because `useMemoryProfilerContext` has consumers (`useMemoryTrace` in many components) that fire on every mount; kept in the synchronous render path.
- **2026-09-03 Inline Loader Alignment:** Matched inline `#vox-boot-loader` CSS to React `OrbitalLoader` `size="lg"` dimensions (`w-28 h-28` = 112px, inset values, center orb size, font size) to eliminate visual jitter during the inline-to-React loader transition.
