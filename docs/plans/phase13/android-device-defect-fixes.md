# Phase 13.2 — On-Device Defect Fixes Plan

> **Status:** authored 2026-10-11 after the first real on-device session. Nothing in
> this plan has been implemented. Every defect below was reproduced on hardware
> (POCO `22101320I`, Android 14, `arm64-v8a`, MIUI) or proven by live HTTP.
>
> **Purpose:** six user-reported bugs from the first end-to-end onboarding run.
> One of them (D1) is a crash loop that makes the app unusable, and it was
> **introduced by this project's own Phase 13 work** — see the accountability note.
>
> **Companion documents:** [`android-platform-preparation-plan.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/android-platform-preparation-plan.md)
> (platform plan, Phase 13.1) · [`recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md) (ledger).

---

## 0. Accountability note — read this first

**D1 was caused by Vox, not by Android.**

Commit `2d10246e` ("integrate mobile permission step with wizard footer…") added
`jni = "0.21"` and `ndk-context = "0.1"` to the Android target so that
`check_mic_access()` could ask the platform about `RECORD_AUDIO` instead of
asking cpal (which was itself correct — see §Defect 5).

Both new call sites carry `SAFETY` comments asserting:

> `// SAFETY: initialized by ndk-glue during Tauri startup, before any command can run.`

**That guarantee does not exist in this project.** `ndk-glue` is absent from
`Cargo.toml` *and* from `Cargo.lock` — verified:

```
$ grep -rn "ndk-glue\|ndk_glue" app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock
(no matches)
```

`ndk_context::android_context()` is `unsafe { ANDROID_CONTEXT.expect("android
context was not initialized") }`. Nothing ever calls
`ndk_context::initialize_android_context`. **Every call panics.** Twelve panics
were captured on the device during the session that produced this plan.

The comments were written without verifying the claim. That is the root failure,
and D1's fix must include correcting them — a stale `SAFETY` comment is worse
than no comment, because the next agent will trust it.

---

## 1. Device state at hand-off (verified 2026-10-11)

| Fact | Value | How verified |
|---|---|---|
| Installed build | **DEBUG** APK (HMR-capable), `2d10246e` | `dumpsys package` → `flags=[ DEBUGGABLE ... ]` |
| `RECORD_AUDIO` | `granted=true` (`USER_SET`) | `dumpsys package` |
| `POST_NOTIFICATIONS` | **`granted=false`** | `dumpsys package` |
| `setup_completed` | `false` | `settings.jsonc` |
| Models present | `vad/`, `translit/`, `stt/nemotron-3.5` (**667 MB**) | `run-as … ls` |
| `llm/qwen/` | **empty directory** (404 left nothing) | `run-as … ls -la` |
| `models/embedding/` | **does not exist** | `run-as … ls` |
| App data root | `/data/user/0/com.addy.Vox` (**not** `…/files`) | `run-as … pwd` |
| Vox icon | dark `#090D16` plate, **verified fixed** | `aapt2 dump resources` + screenshot |

> ⚠️ **Path trap.** The DEBUG APK roots at `/data/user/0/com.addy.Vox`, while
> RELEASE roots at `/data/user/0/com.addy.Vox/files`. A relative `run-as … du`
> produces plausible-looking output for the wrong tree. Always `cd` explicitly
> and print `pwd` before drawing conclusions about what is on disk.

### Already resolved upstream (do not redo)

The **Qwen 404 is fixed.** `submodules/vox-models` `e24e95b` published
`llm/qwen/qwen-3.5-0.8b-q4_k_m.gguf` (blob sourced from `~/.vox/models/`, sha256
verified as `bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517`,
matching the manifest) plus `disfluency/`, `tts/zipvoice/`, `voices/`, and
relaxed the `required` flags. Pushed to `huggingface.co/addyo07/vox-models`.

Verified live: Qwen `302`, `tts/zipvoice/encoder.int8.onnx` `302`,
`disfluency/deberta-v3-base/tokenizer.json` `302`, `embedding/minilm-l12-v2/model_int8.onnx` `302`.

Three `.gitattributes` LFS rules were added for the new trees
(`disfluency/**/*`, `tts/zipvoice/**/*`, `voices/**/*`), matching the existing
`nli/**/*` and `embedding/**/*` catch-alls. Without them, three ~8 MB binaries
(`disfluency/deberta-v3-{base,large}/tokenizer.json`,
`tts/zipvoice/espeak-ng-data/ru_dict`) would have been committed raw.

**Consequence for this plan:** D5 loses its 404 root cause and reduces to the
UI-truthfulness defects (which are still fully live). The batch-abort and
no-retry behaviour remain real and worth fixing.

---

## 2. Defect register

### D1 — Engage spins forever and cannot be cancelled · **CRITICAL**

**Severity:** blocks all runtime use. **Status:** open. **Introduced by:** `2d10246e`.

**Symptom.** Tapping Engage spins indefinitely. No error. No way out.

**Root cause (CONFIRMED).** The `ndk-context` panic, reached transitively through
cpal:

```
app/src-tauri/src/core/engine.rs:300              AudioStream::new(...)
app/src-tauri/src/services/audio/device.rs:114   device.default_input_config()?   ← unconditional
cpal-0.15.3/src/host/oboe/mod.rs:288              default_supported_configs(false)
cpal-0.15.3/src/host/oboe/mod.rs:99               get_audio_record_min_buffer_size
cpal-0.15.3/src/host/oboe/android_media.rs:18     ndk_context::android_context()  ← PANIC
```

`device.rs:114` runs regardless of whether `audio.input_device` is set, because
cpal's Oboe `default_input_device()` returns an unconditional `Some(Device(None))`
whose `supported_input_configs()` brute-forces 52 `getMinBufferSize` JNI calls.
The first panics. Hence: every engage, always.

**Why it hangs silently (CONFIRMED).** Tauri spawns async commands
(`tauri-2.11.1/src/ipc/mod.rs:371-389`); the panic unwinds past `return_result`,
so the `OwnedInvokeResponder` is dropped with no message. The JS `invoke()`
promise **never settles** — no rejection, no timeout.

**Why it cannot be cancelled (CONFIRMED, three compounding locks).**
1. `VoiceSessionContext.tsx:246-273` — `finally { setIsLaunching(false) }` never
   runs. The `8000` ms guard in `pipelineService.ts:139-144` only covers
   `waitForState` *after* `startSession` resolves; it does not bound the invoke.
2. `Home.tsx:488` — `disabled={isLaunching}`, so the only visible control locks.
3. `Home.tsx:455-474` — Disengage renders only when `isEngaged`, which never
   becomes true: the panic precedes `SessionStart` (`ipc/pipeline.rs:120-125`).

`stop_engine` exists (`ipc/pipeline.rs:34`) but `Home.tsx` never calls it, and
`stop_audio_engine` (`engine.rs:334-341`) would no-op anyway since the engine
was never stored.

**Fix.**

| # | Change | File |
|---|---|---|
| D1.1 | Initialize `ndk-context` from Tauri's `run_on_android_context`, which yields both the `JavaVM` and the `Context`. **Preferred over adding `ndk-glue`** — no new native linkage, and it is the handle Tauri already owns. Guard with a `Once`/atomic so it cannot double-init (`initialize_android_context` asserts `previous.is_none()`). | `src/lib.rs`, mobile entry (`#[cfg_attr(mobile, tauri::mobile_entry_point)]` at `:198`) |
| D1.2 | Make both JNI sites fail **soft**: `android_record_audio_granted()` → `false`; `android_external_files_dir()` → `None`. A bridge fault must never panic a Tauri command. **Correct the two false `SAFETY` comments.** | `setup/runtime_check.rs:186-250`, `ipc/notifications.rs:120-165` |
| D1.3 | Bound `engageSession`: race `invoke("start_session")` against a timeout and surface a real error. **Do this regardless of D1.1** — any backend abort reproduces this hang, and by construction it is silent. | `services/pipelineService.ts`, `shared/context/VoiceSessionContext.tsx` |
| D1.4 | Render a Stop/Cancel affordance whenever `isLaunching` exceeds the timeout, so the user is never trapped. | `pages/Home.tsx` |

**Gate.** Engage → session starts; `adb logcat` shows `[Audio::Device] Using
input device:` and no panic; force-stop mid-start recovers cleanly.

---

### D2 — Notification permission never granted, UI showed a green tick · **HIGH**

**Severity:** notifications silently dead; UI actively lies. **Status:** open.

**Device proof.** `RECORD_AUDIO: granted=true` but
`POST_NOTIFICATIONS: granted=false`.

**Two compounding causes.**

**(a) Backend (CONFIRMED from logcat).**
```
[Notification::Android] Permission request failed:
  lateinit property requestPermissionsLauncher has not been initialized
```
`tauri-plugin-notification` 2.4.0's `NotificationPlugin.kt:251-259` delegates
`requestPermissions` to `requestPermissionForAlias`, whose launcher is not
registered in this Tauri setup. **No notification dialog can ever appear.**

**(b) Frontend (`MobilePermissionStep.tsx:39`, CONFIRMED).**
```ts
const notifsGranted = notifState === 'granted' || notifState === 'not_applicable' || notifState === 'unavailable';
```
The thrown error maps to `'unavailable'` (`:53`, `:86`), which counts as
**granted** — so the failure renders as a success tick and the label reads
"Available" (`:219-220`).

This contradicts the file's own contract in `setupService.ts:140-147`:
*"Never render a bridge fault as a refusal."* Here it renders as a **grant**,
which is worse.

**Answer to the user's question — was it auto-granted?** No. Nothing was
auto-granted. The mic dialog came from wry's `RustWebChromeClient.kt:103-104`,
which requests `RECORD_AUDIO` + `MODIFY_AUDIO_SETTINGS` together and grants
app-wide. That path works. The notification path is separate and broken.

**Sequencing.** The existing code already requests mic first, then notifications
(`MobilePermissionStep.tsx:57-84`). It silently skips step 2. Once (a) is fixed,
both dialogs appear in sequence with no further ordering work.

**Fix — decision: (a) register the missing launcher** (user-selected 2026-10-11).
The plugin already owns channel creation and timed dismissal, both of which work;
only the permission launcher is absent. Keep `tauri-plugin-notification` pinned at
`=2.4.0` — 2.5.x requires `tauri ^2.12` and would drag the app off the pinned
2.11 that the hand-built Android native toolchain depends on.

| # | Change |
|---|---|
| D2.1 | Register the `requestPermissionsLauncher` for the notification plugin's `permissionState` alias, or bridge `requestPermissionForAlias`. Mirror the `jni-keep.pro` approach — a new file auto-globbed by `build.gradle.kts`, so `tauri android init` will not clobber it. |
| D2.2 | `notifsGranted` must **not** include `'unavailable'`. Render that state as "Couldn't ask" with a retry control; it is neither consent nor refusal. |
| D2.3 | Gate the mic tick on the real `RECORD_AUDIO` grant — which only becomes truthful once D1.1 lands. |

---

### D3 — Footer button flickers while navigating steps · **MEDIUM**

**Status:** open. **Cause (CONFIRMED by reading).**
`MobileOnboardingFlow.tsx:118` recomputes `footerConfig` on **every render**, and
the button in `MobileStepFooter.tsx` carries `transition-all` plus
`active:scale-[0.98]`. `label`, `disabled`, and `isLoading` all change identity
mid-swipe.

Aggravating: `MobileModelStep.tsx:206` re-renders the parent on every progress
tick via `onModelStatusChange`.

**Fix.** Memoise `footerConfig` on its real dependencies; drop `transition-all`
from the button in favour of a scoped transition on colour only.

---

### D4 — Accordion size ignores selection · **MEDIUM**

**Status:** open. **Cause (CONFIRMED).** `MobileModelStep.tsx:133-136` sums
**every** group in a category and never consults `selectedIds`:

```ts
const groupSize = groups.reduce((sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0), 0);
```

Compare `:149-154`, which does it correctly for the grand total. **Fix:** apply
the same `selectedIds.has(g.id)` filter per category.

---

### D5 — "Download completed in 2 seconds" · **HIGH (truthfulness)**

**Status:** partially resolved upstream (404); three frontend defects remain open.

The backend genuinely ran: `models/stt/nemotron-3.5` is 667 MB on disk. The UI
lied.

| # | Defect | Location |
|---|---|---|
| D5.1 | `isDone` set on the **first** `completed` event, not all selected models. Model 1 is 644 KB and finishes in <1 s → percent forced to 100 (`:157`) → button flips to "Continue". The desktop equivalent uses `.every(...)` at `ModelSetupStep.tsx:289`. | `MobileModelStep.tsx:82-84` |
| D5.2 | Only `completed` is handled. A `Failed` event is discarded, so the 404 was invisible. `MODEL_PROGRESS_STEPS.failed` exists at `welcomeCopy.ts:268` but is unreachable from this component. | `MobileModelStep.tsx:81` |
| D5.3 | On **any** IPC error, a fake timer advances 20 % per 600 ms to 100 % and calls `setIsDone(true)`. An error is rendered as a success. **Delete this block.** | `MobileModelStep.tsx:168-178` |
| D5.4 | `getRuntimeReport()` races `fetchManifest()` in the same tick; the report reads `state.manifest` before the fetch lands, returns `NotFound`, and the rejection is swallowed at `:77`. The `models_verified` short-circuit at `:73-75` is dead on first run. | `MobileModelStep.tsx:59-77` |
| D5.5 | Backend aborts the whole batch on the first failure, stranding remaining required models with no retry. Models 12 (`embedding_minilm_l12_onnx`, 118 MB) and 13 (tokenizer, 17 MB) never ran; `models/embedding/` does not exist. | `setup/manager_ops.rs:191-195` |

**Adjacent defects found while reading (not user-reported; fix or file):**

| # | Defect | Location |
|---|---|---|
| D5.6 | `is_model_file_present()` **manufactures a `.verified` marker from a size match alone**, with no hash read. Any file of the right size is blessed. | `setup/manager_ops.rs:99-118` |
| D5.7 | Skip path trusts marker-hash + file-existence; never checks size or content. | `setup/model_manager.rs:119-139` |
| D5.8 | `models_verified` is likewise size-only. | `setup/runtime_check.rs:287-299` |
| D5.9 | Manifest cache write is a detached, un-awaited `spawn_blocking`, so it can be lost. | `setup/manager_ops.rs:52-58` |

**Gate.** Download all selected models to completion; abort one mid-transfer and
confirm the batch continues past it; force a failure and confirm it is surfaced
by name with a working retry.

---

### D6 — Model step and final step need redesign · **MEDIUM**

**Status:** open. **Direction (user, 2026-10-11):** redesign freely, **do not
mirror the desktop wizard** — its UI is entirely different. Native-mobile
composition only.

**D6a — model step.** Current issues: single centred CTA, no way to leave the
screen, no background-download affordance, category rows give no per-category
progress. Must reflect D4 (honest per-category sizes) and D5 (honest progress,
visible failure, retry).

**D6b — final step.** `MobileReadyStep` copy and layout are unpolished; heavy
centre-alignment, weak hierarchy. Redesign for a phone.

**Accessibility constraint (style guide §5, non-negotiable).** Every touch
target ≥ 48×48 px; safe-area insets respected; no hover dependence.

---

### D7 — Background model download · **DEFERRED (user, 2026-10-11)**

Agreed to defer. Recorded here so it is not lost.

A true background download requires a real Android **foreground service**
(`FOREGROUND_SERVICE_DATA_SYNC` + persistent notification). There is no shortcut:
Android kills backgrounded processes, and Batch 8's resume only helps *after*
the user reopens the app.

**Interim mitigation (already shipped):** resumable downloads with retained
`.tmp` partials, so an interrupted transfer continues rather than restarting from
byte zero.

**When picked up:** the model step should offer "Continue in background", default
**on**, so a user is never trapped on the wizard waiting for a multi-GB
transfer. Estimate ≈ 200–400 LOC including manifest, service, and lifecycle.

---

## 3. Execution order

| Batch | Work | Gate | Depends on |
|---|---|---|---|
| **1** | D1 (ndk-context init, soft-fail, bounded engage, cancel affordance) | Engage works on device; no `android context was not initialized` in logcat | — |
| **2** | D2 (notification launcher, honest `unavailable`, mic tick) | Both permission dialogs appear in sequence; UI never claims a grant it lacks | 1 |
| **3** | D5 (completion/failure truthfulness, accordion size, race, batch continue) | Full download completes; failure surfaced by name with retry | 1 |
| **4** | D3 (footer flicker), D6 (two redesigns) | Visual review on device | 3 |
| **5** | D7 background download | Deferred | 4 |

Batches 1–2 must land before the device is trustworthy again. Batch 3 is
correctness and can follow immediately. Batch 4 is presentation.

**Do not run `cargo` invocations concurrently** — `verify-targets.sh` compiles
both targets against one target directory.

### Verification per AGENTS.md §3

```bash
cd app/src-tauri && ./scripts/verify-targets.sh     # both targets, -D warnings
cd app && pnpm tsc --noEmit
cd app && pnpm test
```

Plus, for every backend change, a real device run via
`scripts/android-deploy.sh cycle` and a `logcat` sweep for panics.

---

## 4. Open questions

1. **Notification launcher (D2.1).** The plugin's Kotlin launcher may be
   unreachable without forking `NotificationPlugin.kt`. If the alias cannot be
   registered cleanly, fall back to driving `POST_NOTIFICATIONS` over the same
   JNI path added for `RECORD_AUDIO` in D1 — full control, but diverges from the
   plugin. Decide during implementation; do not guess first.
2. **Required-model policy (D5.5).** The user has deliberately relaxed
   `required` — only VAD and STT are mandatory, since TTS and LLM can be cloud.
   **The batch must stop treating a failed optional model as fatal.** Confirm
   that reading before coding: it changes whether D5.5 is "continue past
   failures" or "continue only past optional failures".
3. **D5.6–D5.9.** These are pre-existing integrity defects, not regressions. Fix
   in this stream or file separately — they are not required to unblock the
   device.

---

## 5. Related documents

- [`android-platform-preparation-plan.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/android-platform-preparation-plan.md) — Phase 13.1 platform plan
- [`recent_work.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md) — Phase 13 ledger
- [`.agents/rules/android-style-guide.md`](file:///home/addy/projects/apps/vox/.agents/rules/android-style-guide.md) — mandatory for Android work; Pitfalls 8–21 migrated from the platform plan