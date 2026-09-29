# Settings Architecture Refactor — Senior Backend Review

**Scope:** uncommitted working tree (`git diff HEAD` + untracked `src/config/`, `ipc/settings.rs`, `ipc/catalog.rs`, 5 domain `config.rs` files) against `a5290b6c`.
**Method:** read-only. No files modified except this report and the mandatory `AGENTS.md` log entry. `cargo clippy` / `nextest` were not re-run (per the brief, they are green) — every claim below is grounded in a direct source read with a `file:line` citation, and I have stated the read path for each so you can spot-check.

---

## Review Summary

**Calibration:** this is a **single-user, local-first desktop app**. One process, one user, one settings writer, human-rate mutations (a few per second at most from slider drags), no network fan-out on the settings path, and the engine holds multi-GB models in RAM.

That calibration kills most of what a generic reviewer would reach for. **Do not bother** with: lock-free settings reads, `ArcSwap` on `AppState.settings`, per-field atomic persistence, write batching beyond the existing debounce, sharding. All of that is over-engineering at this scale and I am not flagging it.

What *does* matter at this scale is exactly two things:

1. **Does what the app tells the user actually happen?** The `reload_policy` string returned by `update_setting` is a user-facing promise. Every place it lies is a bug report.
2. **Does a user's config survive a restart, a crash, or a typo?** Settings are the only place a user is asked to hand-enter API keys and prompts. Losing them is the worst class of bug this subsystem has.

The refactor itself is good work — the module boundaries are correct and the layering is real, not cosmetic (see *What's Actually Fine*). The findings below are almost entirely about the **contract between the reload-policy table and the dispatch table**, and about **default-value and recovery-path divergence** that only shows up when something has already gone wrong.

**Headline:** 4 findings will produce a wrong outcome for a real user, 12 more cost real pain. None of them require a redesign. The single most important fix is one line in `config/mod.rs`.

---

## 🔴 Will Break

### 1. `llm.cloud_keys` reports "hot-applied" and then does nothing until restart

**What it is.** `get_setting_reload_policy` classifies `("llm", "cloud_keys")` as `Hot` (`app/src-tauri/src/config/mod.rs:53-58`). But the LLM provider — the only consumer of the key — is constructed exactly once per engine lifetime and cached.

The read path, end to end:

- `LlmSettings::to_provider_config()` is the **only** place `cloud_keys` is read, in the `Cloud` arm (`app/src-tauri/src/services/llm/config.rs:167-179`).
- It is called from `create_llm_provider_from_llm_settings` (`app/src-tauri/src/services/llm/factory.rs:16`) and nowhere else in the production path.
- `factory.rs:53` → `actor.rs:369`, inside `warm_up_llm`, which is guarded by `if handles.llm_tx.is_some() { return Ok(()); }` (`app/src-tauri/src/services/llm/actor.rs:363`).
- `handle_setting_side_effects` has **no `llm` arm** (`config/dispatch.rs:238-301`), so nothing triggers a rebuild.

**Why it matters.** A user pastes a new NVIDIA key, the UI reports `llm.cloud_keys = {...} — hot-applied`, `providers.jsonc` is updated, and the next request still 401s. The key that the whole cloud path depends on is the one field whose contract is inverted — note that the *container* `llm.cloud` is correctly classified `Restart` while the *key inside it* is `Hot`.

**Replacement.** Correct the policy; the rebuild path is a much larger change and is not warranted at this scale.

```rust
// config/mod.rs — replace lines 53-61
"llm" if key == "temperature"
    || key == "compaction_temperature"
    || key == "reasoning_enabled"
    || key == "max_output_tokens" => SettingReloadPolicy::Hot,
// Provider construction reads cloud_keys once (llm/factory.rs:16) and the
// actor caches it for the engine lifetime (llm/actor.rs:363).
"llm" if key == "cloud_keys"
    || key == "cloud"
    || key == "server"
    || key == "provider"
    || key == "active"
    || key == "model" => SettingReloadPolicy::Restart,
```

I verified `temperature` genuinely *is* hot — `services/harness/chassis.rs:79` reads `settings.llm.temperature` per request — so that one stays. Only the provider-identity fields move.

---

### 2. `vad.max_speech_duration_s` claims a worker dispatch that does not exist

**What it is.** `("vad", "max_speech_duration_s")` → `SettingReloadPolicy::WorkerCommand` (`config/mod.rs:62-70`). `dispatch_worker_command` handles `threshold`, `ptt_noise_gate`, `silence_duration_ms`, `speech_onset_ms` — and **not** `max_speech_duration_s` (`config/dispatch.rs:315-405`).

**Why it matters.** User raises max speech duration from 30s to 60s for a long dictation. `update_setting` returns `reload_policy: "worker_command"`, the UI shows it as applied live, `settings.jsonc` records 60, and the running segmenter still cuts at 30. The setting silently reverts on next launch. This is the mirror image of finding 1: there, the policy is wrong; here, the dispatch table is incomplete.

**Replacement.** Either add the arm or drop it from the policy. Adding it is the better product, but it needs a new `VadCommand` variant, so until that exists the honest answer is `Restart`:

```rust
"vad" if key == "threshold"
    || key == "ptt_noise_gate"
    || key == "silence_duration_ms"
    || key == "speech_onset_ms" => SettingReloadPolicy::WorkerCommand,
// max_speech_duration_s has no VadCommand variant — see config/dispatch.rs:315-405
"vad" if key == "max_speech_duration_s" => SettingReloadPolicy::Restart,
```

**Which suggests the real fix:** the policy table and the dispatch table are two hand-maintained lists that must agree, and they have already drifted in both directions. That is a test, not a code change — see *Test Debt* below.

---

### 3. `context_window` clamp is dead code; the raw field reaches the GGUF loader

**What it is.** `LlmSettings::effective_ctx_size()` clamps to `MIN_LLM_CONTEXT_WINDOW` (`services/llm/config.rs:153-155`) and has a unit test for it (`config.rs:265-275`). **It has no production caller.** The real path uses the raw field:

```rust
// services/llm/factory.rs:18
let ctx_size = llm_settings.context_window;   // not effective_ctx_size()
```

**Why it matters.** The only guard on this value is in `apply_llm_mutation` (`config/mutation.rs:225-231`). `load()` performs **no validation at all** — `merge_agent_config` assigns `cfg.cognitive.context_window` straight through (`config/files.rs:294`). So a hand-edited or valid-but-wrong `agent.jsonc` with `"context_window": 512` loads and reaches `EmbeddedProvider::new(llm_path, 512, n_threads)`. Given `agent.jsonc` is documented as a user-editable file, this is reachable by design.

**Replacement.** One word:

```rust
// services/llm/factory.rs:18
let ctx_size = llm_settings.effective_ctx_size();
```

---

### 4. Settings are not flushed on exit — the last 1.5s of changes are lost

**What it is.** `schedule_debounced_save` defers the disk write by `SETTINGS_SAVE_DEBOUNCE_MS = 1500` (`config/dispatch.rs:31,412-440`). `RunEvent::Exit` sends engine shutdown, signals the persistence worker, then sleeps 150ms — and never touches the settings debounce (`app/src-tauri/src/lib.rs:714-746`).

**Why it matters.** Toggle the theme, hit Quit from the tray 500ms later. `RunEvent::Exit` fires, the process is gone before the 1500ms timer. The change is gone, and it is gone **silently** — no error, no log line. The user's mental model is "I saved it"; the app's behaviour is "I didn't".

**This is pre-existing** — the identical debounce and identical exit handler are in `a5290b6c`. Flagging it because the refactor is the natural moment to fix it and because it is a guaranteed user report.

**Replacement.** Abort the pending handle and write synchronously before the shutdown sequence:

```rust
// lib.rs, inside `tauri::RunEvent::Exit`, before the engine teardown
let debounce = state.save_debounce.blocking_lock().take();
if let Some(handle) = debounce {
    handle.abort();
    // Blocking write: we are already on the exit path, latency is irrelevant.
    let snapshot = state.settings.read().unwrap_or_else(|p| p.into_inner()).clone();
    if let Err(e) = snapshot.save() {
        log::error!("[Vox] Final settings flush failed: {}", e);
    }
}
```

---

## 🟠 Real Cost at This Scale

### 5. Three different defaults for `tts.active`

`TtsActiveProvider` derives `Default` as **`Kokoro`** (`services/tts/config.rs:37-38`). `TtsSettings::default()` sets **`EdgeTts`** (`services/tts/config.rs:158`). `TtsWiringSettings::default()` sets **`Kokoro`** (`config/files.rs:79`).

Concretely:

| On-disk state | Result |
|---|---|
| No config files at all | `Self::default()` → **EdgeTts** (`config/persistence.rs:276`) |
| `settings.jsonc` present, `"tts"` object absent | `SettingsConfigFile` is `#[serde(default)]` → `TtsWiringSettings::default()` → **Kokoro**, then `merge_settings_config` assigns it unconditionally (`config/files.rs:267`) |
| `settings.jsonc` present, `"tts"` present | whatever is on disk |

**Why it matters.** The second row is exactly the *recovery* path — partial corruption, a hand-edit, a truncated write. The safety net hands the user a different TTS engine than a clean install would, with no log line explaining why. The `#[default]` attribute on the enum is also a live trap: any future `TtsSettings { ..Default::default() }` or `TtsActiveProvider::default()` written by an agent will disagree with `TtsSettings::default()`.

**Replacement.** Delete the third source and make the wiring type derive from the domain default:

```rust
// config/files.rs:76-87
impl Default for TtsWiringSettings {
    fn default() -> Self {
        let d = TtsSettings::default();
        Self {
            active: d.active,
            voice_index: d.voice_index,
            speed: d.speed,
            threads: d.threads,
            supertonic: d.supertonic,
            kokoro: d.kokoro,
        }
    }
}
```

Same treatment for `SttWiringSettings::default` (`config/files.rs:36-44`) and `LlmWiringSettings::default` (`config/files.rs:54-62`) — they currently duplicate default literals that live in `core/defaults.rs` via a second path.

---

### 6. `SttSettings::cloud_keys` is a phantom field

`services/stt/config.rs:114` declares `cloud_keys: HashMap<String, String>`. It is:

- **never read** — `rtk rg "cloud_keys"` across the crate returns only the declaration, the `Default` init, its own unit test, and the `LlmSettings` twin;
- **never written** — no `apply_stt_mutation` arm (contrast `llm`'s at `config/mutation.rs:252`);
- **never persisted** — `SttProvidersConfig` holds only `cloud: SttCloudConfig` (`config/files.rs:104-108`), and `to_providers_config` / `merge_providers_config` (`config/files.rs:226-228,279`) never mention it.

It survived the split only in the *legacy* branch, where `recover_all_sections` parses `stt` as the full `SttSettings` (`config/persistence.rs:173-175`) — so a legacy monolith's STT cloud keys load into memory and are then dropped on the first `save()`.

Related: `SttActiveProvider::Cloud` has **no provider implementation**. `services/stt/providers/` contains only `embedded.rs`, `nemotron.rs`, `qwen.rs`; `rtk rg -i "api_key|token|auth" services/stt/providers/` returns nothing but tokenizer paths. So `stt.cloud` is a persisted config surface for a code path that does not exist.

**Recommendation.** Delete `cloud_keys` from `SttSettings` (ZBC — the project is explicit about this). If STT cloud is genuinely planned, land it with the provider and give it a persisted home in `SttProvidersConfig` at the same time. Do not leave a credential-shaped field that is neither stored nor read.

---

### 7. Two dead knobs in `llm.embedded` that persist to `settings.jsonc`

`LlmEmbeddedConfig.threads` (`services/llm/config.rs:27`) and `LlmEmbeddedConfig.context_size` (`:26`) have **no production reader**. The engine uses `llm_settings.threads` (`services/llm/factory.rs:20`) and `llm_settings.context_window` (`:18`).

**Why it matters.** `LlmWiringSettings` clones the whole `embedded` struct into `settings.jsonc` (`config/files.rs:206,266`), so `settings.jsonc` ships two pairs of plausible-looking knobs that do nothing. A user tuning "LLM threads" by editing the nested `embedded.threads` gets a silent no-op. `apply_llm_mutation("threads")` writes the *outer* field, so the file ends up containing both values disagreeing with each other.

**Replacement.** Remove both fields. `LlmEmbeddedConfig` then reduces to `model` + `n_gpu_layers`, which is honest.

---

### 8. `save()` writes all three files unconditionally, in an order that can half-commit

`VoxSettings::save` (`config/persistence.rs:279-296`) unconditionally writes `settings.jsonc` → `providers.jsonc` → `agent.jsonc`, each a full create + `fsync` + `rename` (+ `chmod` for providers). Toggling `dictation.enabled` rewrites the file containing every API key, three times the I/O, and re-applies `0600`.

More importantly, the `?` operators mean **a failure on `providers.jsonc` skips `agent.jsonc` entirely** while `settings.jsonc` has already been replaced. The user is left with a mixed-vintage trio, and `load()` cannot detect it.

**Replacement.** Reverse the order so the secret file commits last, and don't early-return:

```rust
pub fn save(&self) -> Result<()> {
    let mut first_err = None;
    // agent + settings first; providers (the 0600 secret file) commits last so a
    // failure can never leave it newer than the settings that reference it.
    for (path, cfg) in [
        (paths::agent_path(), self.to_agent_config()),
        (paths::settings_path(), self.to_settings_config()),
    ] {
        if let Err(e) = atomic_write_json(&path, &cfg, None) {
            log::error!("[Settings] Failed to persist {:?}: {}", path, e);
            first_err.get_or_insert(e);
        }
    }
    #[cfg(unix)]
    let p = atomic_write_json(&paths::providers_path(), &self.to_providers_config(), Some(0o600));
    #[cfg(not(unix))]
    let p = atomic_write_json(&paths::providers_path(), &self.to_providers_config(), None);
    if let Err(e) = p {
        log::error!("[Settings] Failed to persist providers.jsonc: {}", e);
        first_err.get_or_insert(e);
    }
    first_err.map_or(Ok(()), Err)
}
```

A dirty-bit per file (`AtomicU8` bitmask in `AppState`) so a `dictation.enabled` toggle only touches `settings.jsonc` is a reasonable follow-on, but the ordering fix is the part that matters.

---

### 9. The two likeliest hand-edit corruptions skip recovery entirely and nuke the file

`strip_comments` (`utils/jsonc.rs`) handles `//` and `/* */` correctly, including escapes and URLs — that part is solid. It does not handle:

- **UTF-8 BOM.** A file saved by any Windows editor, or by VS Code with `"files.encoding": "utf8bom"`, starts with `\u{feff}`. `serde_json` rejects it.
- **Trailing commas.** `{"a": 1,}` is the single most common JSONC typo and is accepted by the reference `jsonc-parser` implementations users are used to.

In both cases `from_jsonc_str::<SettingsConfigFile>` fails, `from_jsonc_str::<Value>` fails, and `load()` falls to `backup_corrupt` (`config/persistence.rs:224`) — the whole file is renamed away and the user is on defaults. A one-character mistake costs every API key in `providers.jsonc`.

**Replacement.** Two small additions to `strip_comments`:

```rust
pub fn from_jsonc_str<T: DeserializeOwned>(input: &str) -> Result<T, serde_json::Error> {
    // Tolerate a UTF-8 BOM (any Windows/VS Code editor can emit one) and
    // trailing commas (the most common JSONC hand-edit slip). Both are
    // recoverable, so neither should reach the corrupt-file backup path.
    let mut s = strip_comments(input);
    if s.starts_with('\u{feff}') {
        s.remove(0);
    }
    serde_json::from_str(&strip_trailing_commas(&s))
}

/// Drops `,` that is followed only by whitespace and then `}` or `]`.
fn strip_trailing_commas(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escaped = false;
    for (i, &c) in b.iter().enumerate() {
        if in_string {
            out.push(c as char);
            if escaped { escaped = false; }
            else if c == b'\\' { escaped = true; }
            else if c == b'"' { in_string = false; }
            continue;
        }
        if c == b'"' { in_string = true; out.push('"'); continue; }
        if c == b',' {
            let next = b[i + 1..].iter().find(|x| !x.is_ascii_whitespace());
            if matches!(next, Some(b'}') | Some(b']')) { continue; }
        }
        out.push(c as char);
    }
    out
}
```

(Bit-indexing a `String` with a `char` push is only safe because every branch above pushes a byte that was verified to be ASCII in that position; for a production version, iterate `char_indices` instead. The logic is what matters here.)

---

### 10. `recover_*_sections` is 127 lines of near-duplicate code that is almost unreachable, and its log message is wrong

`config/persistence.rs:81-207` contains three recovery functions. `recover_settings_sections` and `recover_providers_sections` are the live 3-way ones; `recover_all_sections` is the legacy-monolith one and is a near-line-for-line superset.

The log line at `config/persistence.rs:264` says *"Partial corruption or schema drift detected … attempting section recovery."* That is misleading. Every type involved carries `#[serde(default)]` and does **not** use `deny_unknown_fields`, so:

- missing keys → filled from defaults, parse **succeeds**;
- unknown/stale keys → ignored, parse **succeeds**;
- `unknown_junk_section` → ignored, parse **succeeds**.

Recovery only engages on a genuine **type error** (`"threshold": "high"`, `"threads": "four"`). So `test_settings_partial_section_recovery` — which writes a partial JSON object with a stale `quality_steps` key and an `unknown_junk_section` — actually exercises the *happy path*, and its name is a false green.

**Replacement.** Two moves:

1. Delete `recover_all_sections` and the entire legacy branch (`:255-273`) under ZBC. One boot with a legacy monolith is not a compatibility surface worth 20 lines and a divergent failure mode. That removes ~50 lines and one of the two untested-but-divergent corrupt paths.
2. Fix the message to say what actually happened, so an operator reading logs at 3am is not misled:

```rust
log::warn!(
    "[Settings] Type error in {} — recovering parseable sections, \
     missing sections fall back to defaults.",
    settings_path.display()
);
```

**Test debt that follows from this:** tests 2 and 3 in `settings_persistence_test.rs` both write only `settings.jsonc` and leave `providers.jsonc`/`agent.jsonc` absent — which routes both through the *legacy* branch. The 3-way corrupt path is untested, and its behaviour genuinely differs: a totally corrupt `settings.jsonc` in the 3-way branch still merges `providers.jsonc` and `agent.jsonc`, so the result is **not** defaults, which is exactly what test 2's assertions would claim. If you keep the 3-way branch, add a test with all three files present and one corrupt.

---

### 11. `reset_settings` reports nothing, applies nothing, and silently destroys API keys

```rust
// ipc/settings.rs:118-141
let defaults = VoxSettings::default();
{ *settings = defaults.clone(); }
emit_ipc(&app, IpcEvent::SettingsUpdated)?;
schedule_debounced_save(...).await;
Ok(defaults)
```

Three problems:

1. **No `reload_policy` is returned** — unlike `update_setting`, which returns `SettingUpdateResult { applied, reload_policy, message }`. The frontend has no way to know a restart is required, so it renders a settings panel showing `tts.active = edge_tts` while the running engine is still Kokoro.
2. **No side effects run.** `handle_setting_side_effects` is not called, so `working_memory.private_mode` is not pushed to `telemetry.is_private_mode`, memory retrieval is not disabled, the consolidation scheduler is not re-evaluated. The in-memory settings and the running subsystems disagree until restart.
3. **`providers.jsonc` is wiped** — every API key, every `cloud_keys` entry, all four `realtime` provider keys. `VoxSettings::default()` sets `cloud.api_key: None` (`services/llm/config.rs:67-71`). Whether the UI confirms this, I could not verify from the frontend call site in scope.

**Replacement.** Make it symmetric with `update_setting` and be explicit about the blast radius:

```rust
#[tauri::command]
pub async fn reset_settings<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<SettingUpdateResult, VoxIpcError> {
    let state: State<'_, Arc<AppState>> = app.state();
    {
        let mut settings = state.settings.write().map_err(|e| VoxIpcError::Internal(e.to_string()))?;
        *settings = VoxSettings::default();
    }
    // Wipes providers.jsonc (all API keys) and reverts every engine selector.
    // Push the subsystems that read settings live back into sync.
    handle_setting_side_effects(&app, &state, "working_memory", "private_mode", &serde_json::json!(false)).await;
    handle_setting_side_effects(&app, &state, "personal_memory", "context_retrieval_enabled", &serde_json::json!(false)).await;
    if let Err(e) = emit_ipc(&app, IpcEvent::SettingsUpdated) { log::warn!("[Settings::Reset] emit failed: {}", e); }
    schedule_debounced_save(state.inner().clone()).await;
    Ok(SettingUpdateResult {
        applied: true,
        reload_policy: SettingReloadPolicy::Restart.as_str().to_string(),
        message: "settings reset to defaults — restart required (all API keys cleared)".to_string(),
    })
}
```

---

### 12. `("audio", "output_mode")` dispatch arm is unreachable, and contradicts the spec

`config/dispatch.rs:366-375` sends `VadCommand::UpdateAudioMode`. `get_setting_reload_policy` has no `audio` arm, so it falls through to `_ => SettingReloadPolicy::Restart` (`config/mod.rs:82`) — and `dispatch_worker_command` is only called when the policy is `WorkerCommand` (`ipc/settings.rs:87-89`). The arm can never execute.

Meanwhile `docs/specs/storage-spec.md:244` states: *"`settings.jsonc` (Audio/VAD) — IPC mutate — ≤ 100 ms"*. So this is both dead code and a spec violation: switching `speaker` ↔ `headset` reports "restart required" when the mechanism to do it live already exists.

**Replacement.** Add the arm to the policy:

```rust
"audio" if key == "output_mode" => SettingReloadPolicy::WorkerCommand,
```

and delete `_ => SettingReloadPolicy::Restart` in favour of an explicit `log::warn!` on unknown keys, so future drift is loud rather than silent.

---

### 13. `interaction.*` is labelled `Restart` but is genuinely hot

`get_setting_reload_policy` sends `interaction` to the default `Restart`. In reality:

- `interaction.mode` runs `handle_interaction_side_effects` → launches/stops the engine and sends `VadCommand::UpdateMode` (`config/dispatch.rs:193-236`);
- `interaction.pipeline_mode` is read live on every prompt build — `AppState::active_system_prompt` branches on it (`core/state.rs:209-212`).

**Why it matters.** The user is told to restart for a change that already took effect. False "restart required" prompts train users to ignore the whole policy string, which then hides the real lies from findings 1 and 2.

**Replacement.**

```rust
"interaction" => SettingReloadPolicy::Hot,
```

---

### 14. `0600` is best-effort; the failure is silently discarded on the secret file

`storage-spec.md:37` is categorical: *"Provider API keys … must reside strictly in `providers.jsonc` … protected with `0600`."*

```rust
// config/persistence.rs:42 and :59
let _ = file.set_permissions(fs::Permissions::from_mode(m));
let _ = fs::set_permissions(path, fs::Permissions::from_mode(m));
```

Both results are discarded. If either fails — non-POSIX mount, restrictive-but-odd filesystem, NFS, a `umask`/ACL interaction — the API keys sit in a world-readable file and the app reports success. Additionally the tmp file is created via `fs::File::create` (mode `0666 & ~umask`) *before* the chmod, so there is a brief window where the file exists with default permissions; it is empty at that point, so nothing leaks, but the ordering is better expressed as create-with-mode.

**Replacement.** Use `OpenOptions::mode` and propagate the error:

```rust
use std::os::unix::fs::OpenOptionsExt;
let mut opts = fs::OpenOptions::new();
opts.write(true).create(true).truncate(true);
#[cfg(unix)]
if let Some(m) = mode { opts.mode(m); }   // mode is applied at create(2), not after
let mut file = opts.open(&tmp_path)?;
```

Then drop the post-rename `set_permissions` entirely — the mode is already correct and `rename` preserves it.

---

### 15. The 3-way decomposition does not match `storage-spec.md` §4.2–4.4

The spec is the SSOT per `AGENTS.md` §4.3, and the on-disk shape in it does not describe what the code writes.

| Spec (`storage-spec.md:106-200`) | Implementation |
|---|---|
| `"pipeline": { "mode", "stt_engine", "tts_engine", "llm_engine" }` | `interaction.mode` + `stt.active` / `tts.active` / `llm.active` (`config/files.rs:92-102`) |
| `"vad": { "backend": … }` | `"vad": { "vad_backend": … }` (`services/vad/config.rs:25`) |
| `providers.jsonc` = flat `"providers"` map keyed by provider name | `llm` / `stt` / `tts` / `realtime` sub-objects (`config/files.rs:139-146`) |
| `agent.jsonc` has `persona.name`, `persona.system_prompt` | `persona.modular_prompt` / `realtime_prompt`, no `name` (`config/settings.rs:115-120`) |
| `"$schema": "./schemas/*.schema.json"` in every file | no `$schema` written; no `schemas/` directory exists |
| `tts_engine` default `"kokoro"` | `TtsSettings::default()` = `edge_tts` (finding 5) |
| Migration: "Archive legacy `settings.json` -> `cache/settings.json.bak`" (`:269`) | `rename` to `config/settings.jsonc`, no archive (`utils/paths.rs:185`) |
| Migration: "Create symlink `~/.vox/vox.db` -> `data/db/vox.db`" (`:262`) | plain `rename`, no symlink — correct per ZBC, **the spec is stale** |
| Reload: `providers.jsonc` = "Instant Hot-Reload, 0 ms" (`:246`) | `llm.cloud` → `Restart` (finding 1) |

**Recommendation.** Per `AGENTS.md` §4.3, the spec is updated **first** — this is a spec-drift resolution, not a code change. I would rewrite §4.2–4.4 to the shape the code actually produces, fix the symlink line in §6 (ZBC was applied, the spec wasn't updated), and either add the `schemas/` + `$schema` files or drop that from the examples. Leaving both documents claiming different file formats is the failure mode the hook exists to prevent.

---

### 16. `docs/backend.md` documents the code that was just deleted

`docs/backend.md` is the doc other agents will read first, and it is wrong in ways that matter:

- `:379` — "Reload Policies (`core/settings.rs:171-190`)" → file deleted, now `config/mod.rs:45-83`.
- `:387` — "Dispatch is via `ipc/settings/mutation.rs:dispatch_worker_command`" → now `config/dispatch.rs`.
- `:239` — "Quality steps, speed, and `threads` (default `DEFAULT_TTS_THREADS=2`) are `WorkerCommand` hot-reloadable." `quality_steps` was removed (the test at `settings_persistence_test.rs:170-173` asserts it stays removed) and `threads` is `Restart` (`config/mod.rs:79`). An agent trusting this line will build the wrong thing.
- `:193`, `:213` — `core/settings.rs:VadSettings`, `core/settings.rs:398-405`, all relocated.

Also stale: `app/src/services/settingsService.ts:31,36,41,66` cite `ipc/settings/catalog.rs` and `ipc/settings/mutation.rs`, both deleted. And `docs/specs/ownership-spec.md:33` names `ipc/settings/core.rs` as the **authoritative** owner of dictation transitions — that file no longer exists, which matters because that spec is an ownership contract, not a comment.

---

### 17. Durability and debris

Three smaller items, all real:

- **No directory `fsync` after `rename`** (`config/persistence.rs:52-60`). On ext4 with `data=ordered` the rename is usually durable, but a hard power loss can leave you with the old file. For a config file this is an acceptable trade — worth a comment saying so, so the next reader doesn't assume it's already handled.
- **Trio is not atomic.** Three `rename` calls with no journal. A crash between write 1 and 3 leaves a mixed-vintage trio (see finding 8).
- **Debris accumulates unboundedly.** A crash mid-write leaves `settings.jsonc.<nanos>.tmp` in `config/` forever; repeated corruption leaves `settings.corrupt.<ts>.jsonc` files forever. `migrate_legacy_layout` (`utils/paths.rs:154-283`) sweeps legacy paths but not these. Worse, `backup_corrupt` uses `as_secs()` (`config/persistence.rs:65-69`) — two corruptions inside the same second produce the same filename, the second `rename` fails, and the corrupt file is then overwritten by the next `save()`. Use `as_nanos()`, and sweep `config/*.{tmp,corrupt.*}` older than a day at boot.

---

## 🟡 Stylistic / Optional

- **Four hand-maintained field lists must agree.** `to_settings_config` / `to_providers_config` / `to_agent_config` / `merge_*` (`config/files.rs:190-296`) each enumerate the same fields by hand, in both directions. Adding a field to `TtsSettings` and forgetting both lists produces a setting that saves and loads as a silent default. Consider `#[serde(flatten)]` on a `WiringProjection` or a compile-time exhaustiveness check.
- **Dead string-parse branch.** `config/dispatch.rs:379-382` accepts a string for `tts.voice_index`, but `apply_tts_mutation` (`config/mutation.rs:313`) requires `as_i64()`, so the string branch can never be reached from the IPC path.
- **Full `VoxSettings` crosses the IPC boundary including plaintext API keys** (`ipc/settings.rs:41`). `storage-spec.md:37` says the UI must never *serialize* provider contents; returning them over IPC puts every key in the webview's JS heap, in devtools, and in any accidental `console.log`. A masked projection (`api_key_set: bool`) plus a dedicated `set_api_key` command would be the tighter design. Not urgent for a local app, but it is the spec's intent.
- **`interaction_mode` clone removed** (`lib.rs:546`) — good cleanup, the `DictationInteractionMode` `Copy` derive is the right call.

---

## What's Actually Fine

This section is not padding. The following are genuinely well done and should survive the follow-up work:

- **The decomposition is real, not cosmetic.** `TtsSettings` lives with TTS, `DictationSettings` with dictation, `LlmSettings` with LLM. No domain module reaches into another's config; the coupling goes through `VoxSettings` as an aggregate and `config/files.rs` as the single projection boundary. That is the correct shape and it is what made the rest of this review tractable.
- **`atomic_write_json` gets the important parts right**: unique tmp name (nanosecond), `sync_all` before rename, tmp removed on every error path, `0600` applied pre-rename so the secret is never briefly world-readable with content. Only the error-discarding and the create-order need the fix from finding 14.
- **The two-tier load ladder** (typed parse → `Value` parse + section recovery → backup-and-defaults) is the right idea and is much better than the common `unwrap_or_default()`. It just needs its dead branch pruned and its log message corrected (finding 10).
- **`load()` never panics on malformed input.** Every failure path degrades to defaults with a log line and a backup file. That is the property that matters most in a settings loader, and it holds.
- **The debounce design is right for this scale** — 1500ms, snapshot-then-`spawn_blocking`, no lock held across the blocking write, `snapshot.save()` on a clone. The only flaw is the missing flush on exit (finding 4).
- **`AppState.settings` as a single `RwLock<VoxSettings>` with `.read().map(|s| …).unwrap_or_else(|p| p.into_inner())` poison recovery throughout** — consistent, and poison-tolerant in the right places (boot, debounce, tray).
- **No settings are `Debug`-formatted into logs anywhere.** I grepped for it specifically given `storage-spec.md:37`; the only `{:?}` in the vicinity are unrelated LLM token/option diagnostics.
- **TTS speed is clamped provider-side** (`kokoro.rs:119,138`, `zipvoice.rs:160`, `chatterbox.rs:165`, …), so the absence of range validation in `apply_tts_mutation` for `speed` is not exploitable — the engine self-defends. (`ptt_noise_gate`, `temperature`, and `silence_auto_stop_ms` are *not* defended anywhere; lower risk, but worth a pass.)
- **Domain-level range validation exists and is decent** where it's applied: `threshold` 0..=1, `silence_duration_ms` 100..=5000, `speech_onset_ms` 16..=1000, `partial_throttle_ms` 50..=2000, `threads` 1..=64, `max_output_tokens` 1..=32768, `top_k_facts` 1..=100, `consolidation_time` parsed as HH:MM. The gaps are the exception, not the rule.
- **The IPC split is clean.** `ipc/settings.rs` is 141 lines of pure transport with zero policy; `ipc/catalog.rs` is pure reads. That matches `ipc-spec.md` §1.1 and is the right altitude for a Tauri command layer.
- **`test_settings_jsonc_with_comments_support`** genuinely exercises the 3-way JSONC path with all three files present, including a `// URL with // must not be stripped` case in `jsonc.rs` and a 0600 permission assertion. That test has real value.

---

---

# Part 2 — Remediation Verification (2026-09-29)

Verified by direct source read against the post-fix tree. **14 of 17 fully fixed, 1 partially fixed, 1 deliberately declined, and 1 new regression introduced.**

| # | Finding | Status | Evidence |
|---|---|---|---|
| 🔴1 | `llm.cloud_keys` falsely `Hot` | ✅ Fixed | `config/mod.rs:67-77` → `Restart`, with the reason in a comment |
| 🔴2 | `vad.max_speech_duration_s` no dispatch arm | ✅ Fixed | `config/mod.rs:83` → `Restart`; regression-guarded by the new test |
| 🔴3 | `effective_ctx_size()` dead | ✅ Fixed | `llm/factory.rs:19` now calls it |
| 🔴4 | No settings flush on exit | ✅ Fixed | `lib.rs:720-734` — abort + synchronous `save()` before engine teardown |
| 🟠5 | Three `tts.active` defaults | ✅ Fixed | `tts/config.rs:36` `#[default] EdgeTts`; all 3 wiring defaults now derive from `*Settings::default()` |
| 🟠6 | Phantom `stt.cloud_keys` | ✅ Fixed | Field gone from `stt/config.rs:105-111`; zero refs remain |
| 🟠7 | Dead `embedded.threads`/`context_size` | ✅ Fixed | Both pruned; `LlmEmbeddedConfig` is now `{ model, n_gpu_layers }` |
| 🟠8 | `save()` ordering / early-return | ✅ Fixed | `persistence.rs:231-264` — agent→settings→providers, errors collected via `first_err` |
| 🟠9 | BOM + trailing commas | ✅ Fixed | `jsonc.rs` — `strip_trailing_commas` is `char`-based (correct, unlike my byte-index sketch) and BOM stripped before parse |
| 🟠10 | Dead recovery code | ✅ Fixed | `recover_all_sections` + legacy branch removed; log message now accurate |
| 🟠11 | `reset_settings` | ⚠️ Partial | Side effects added (`ipc/settings.rs:131-147`); **return type still `VoxSettings`, no `reload_policy`** — see below |
| 🟠12 | `audio.output_mode` unreachable | ✅ Fixed | `config/mod.rs:53` → `WorkerCommand`; arm is live again |
| 🟠13 | `interaction` mislabelled `Restart` | ✅ Fixed | Folded into the `Hot` arm at `config/mod.rs:48-53` |
| 🟠14 | `0600` errors discarded | ✅ Fixed | `OpenOptionsExt::mode()` at create + `?` propagation; no longer best-effort |
| 🟠15 | Spec drift §4.2–4.4, §6 | ✅ Fixed | All three schemas rewritten to real shapes; symlink line now says "no symlinks" |
| 🟠16 | Stale doc paths | ✅ Fixed | `backend.md`, `ownership-spec.md`, `settingsService.ts` all repointed; `backend.md:385` now lists the corrected `Restart` set |
| 🟠17 | Durability & debris | ⚠️ Partial | Nanosecond collision fixed; **tmp/corrupt sweep and dir-fsync note not added** — see below |
| 🟡 | Dead string-parse branch | ⬜ Not done | Lowest-value item, correctly deprioritised |

The cross-check test landed and is better than the one I sketched — it asserts *both* directions (`settings_persistence_test.rs:582-624`), including negative assertions for the two reclassified keys, and `dispatch_worker_command_has_arm` (`dispatch.rs:411-422`) matches the live `match` arms exactly.

---

## One new regression: the ZBC prune orphaned your own migration path

**This is the one thing to look at before shipping.**

I recommended deleting the legacy monolith branch. That was correct *as a code-deletion* but I did not check what else called into it, and the answer is: your boot migration.

- `utils/paths.rs:176-188` still moves `~/.vox/settings.json` → `config/settings.jsonc`. That is a **monolith** — it still has `realtime`, `persona`, `working_memory`, `personal_memory`, `llm.cloud_keys`, and every provider block at the top level.
- `VoxSettings::load()` no longer has a branch that can read a monolith (`persistence.rs:177-227`). It only ever tries `SettingsConfigFile`.
- `SettingsConfigFile` is `#[serde(default)]` with **no** `deny_unknown_fields` (I grepped — there is none in the crate). So a monolith does **not** fail to parse. It parses *successfully*, every unknown key is silently ignored, and you get a `VoxSettings` with defaults for `persona`, `realtime`, `working_memory`, `personal_memory`, and every API key.
- The first `save()` — which `lib.rs:539` triggers on the setup auto-complete path — then **overwrites the user's monolith** with the 3-way files built from those defaults.

Net effect: a user upgrading from a pre-3-way install loses their API keys, persona prompt, and memory policy on first launch, with no error and no backup. `backup_corrupt` never fires because nothing failed.

**Why the previous state was better:** the legacy branch caught exactly this. It was dead code for a *current* install and live code for an *upgrading* one — and I classified it by the first, not the second. My error.

**Fix — pick one:**

Option A (recommended): make the file-move migration in `paths.rs` call the decomposition, so the monolith is split at the point where it is known to be a monolith:

```rust
// utils/paths.rs — after the rename at line 183
if legacy_root_jsonc.exists() {
    let _ = rename(&legacy_root_jsonc, &target_settings);
} else {
    let _ = rename(&legacy_root_json, &target_settings);
}
// Decompose the migrated monolith into the 3-way layout immediately, before
// VoxSettings::load() can parse it as a SettingsConfigFile and silently drop
// the provider/persona/memory sections it doesn't model.
let _ = crate::config::VoxSettings::load().and_then(|s| s.save());
```

Option B (cheaper, more honest): drop the `settings.json` move from `paths.rs` entirely and let ZBC mean ZBC — the user's old file stays where it is, untouched, and they re-enter two API keys. If you take this route, delete the now-dead branch in the spec flowchart too (`storage-spec.md:321-326` still documents an "Extract provider keys/urls" step that no longer exists).

Either way, **add a test** — a monolith-shaped `settings.jsonc` with no `providers.jsonc`/`agent.jsonc` present, asserting the persona prompt and `llm.cloud.api_key` survive. That is the exact case nothing currently covers; `rtk rg "monolith\|legacy" tests/` returns only unrelated dataset-loader hits.

---

## Two partial fixes worth finishing

### `reset_settings` still can't tell the frontend a restart is required (finding 11)

The side-effect call is in and correct (`ipc/settings.rs:131-147`). The return type is not:

```rust
// ipc/settings.rs:118-120 — unchanged
pub async fn reset_settings<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<VoxSettings, VoxIpcError> {
```

So the frontend still receives a bare `VoxSettings` (`app/src/services/settingsService.ts:68`), applies it to the store (`settingsStore.ts:785-792`), and has no `reload_policy` to key a restart banner off. Every engine the user has running keeps its old provider while the UI shows `tts.active = edge_tts`. Changing the return type to `SettingUpdateResult` is a ~10-line change plus one frontend type update.

### Debris sweep and durability note (finding 17)

`backup_corrupt` now uses `as_nanos()` — the same-second collision is fixed. The other two items were skipped:

- **No sweep of `config/*.{tmp,corrupt.*}`.** `rtk rg "tmp\|corrupt" utils/paths.rs` returns nothing. A crash mid-write leaves a `.tmp` forever; repeated corruption accumulates `.corrupt.*` files forever. `migrate_legacy_layout` sweeps legacy paths but not these. A few lines at the top of that function would close it.
- **No directory `fsync` after `rename`, and no comment saying that's deliberate.** `persistence.rs:63-68` does the rename and stops. A one-line comment ("rename durability on ext4 is best-effort without a parent-dir fsync; accepted trade-off for a user-editable config file") prevents the next reader from assuming it's handled.

---

## Bottom Line

**The refactor is architecturally sound and ready to land** — the module boundaries are correct, the atomic-write and load-ladder primitives are right, and the IPC layer is genuinely decoupled. Ship it. But do not ship it before fixing the `cloud_keys` policy (finding 1) and the settings flush on exit (finding 4): both produce silent, user-visible wrong behaviour, and both are small.

**If you only fix one thing:** `config/mod.rs:53-58` — move `llm.cloud_keys` from `Hot` to `Restart`. It is one line, it fixes the highest-consequence field in the system, and it is the same class of bug as finding 2, so fix them together and then add the cross-check test that would have caught both:

```rust
#[test]
fn reload_policy_worker_command_keys_all_have_dispatch_arms() {
    // Every (domain, key) the policy routes to WorkerCommand must appear in
    // dispatch_worker_command's match, and vice versa. Enumerate both by hand
    // in the test so an addition on either side fails to compile-adjacent review.
    for (domain, key) in [
        ("tts", "speed"), ("tts", "voice_index"), ("tts", "voice"),
        ("vad", "threshold"), ("vad", "ptt_noise_gate"),
        ("vad", "silence_duration_ms"), ("vad", "speech_onset_ms"),
        ("vad", "max_speech_duration_s"), ("audio", "output_mode"),
    ] {
        assert_eq!(
            get_setting_reload_policy(domain, key),
            vox_lib::config::SettingReloadPolicy::WorkerCommand,
            "{}.{} is expected to be worker-dispatched",
            domain, key
        );
        assert!(
            vox_lib::config::dispatch_worker_command_has_arm(domain, key),
            "{}.{} is classified WorkerCommand but dispatch_worker_command has no arm",
            domain, key
        );
    }
}
```

This needs a small `pub(crate)` introspection helper in `config/dispatch.rs`; the alternative — deriving both tables from one `const` slice of `(domain, key, Policy)` — is better still and removes the duplication permanently.

**Then, before the next commit:** update `storage-spec.md` §4.2–4.4 and §6 to match reality, and `docs/backend.md` / `ownership-spec.md` / `settingsService.ts` to match the new file layout. Right now the SSOT and the code disagree about the on-disk format, which is precisely the state `AGENTS.md` §4.3 exists to prevent — and the next agent to read the spec will build against a format that does not exist.
