---
trigger: manual
description: Read before touching ANY Rust code that must compile on Android, or anything touching cpal, tray, dictation, paths, model loading, or platform cfgs.
---

---
description: Read before touching ANY Rust code that must compile on Android, or anything touching cpal, tray, dictation, paths, model loading, or platform cfgs.
---

# Android Platform Pitfalls

Vox is being prepared so a single push builds an installable APK. That work is **incomplete** — `docs/plans/phase13/android-platform-preparation-plan.md` is the source of truth for what is done and what is not.

This file exists because the same traps have already been rediscovered the hard way. Each entry below is a **confirmed** finding with evidence, not a hypothesis.

---

## 0. The gate

**Always verify with the script. Never hand-roll the cargo command.**

```bash
cd app/src-tauri && ./scripts/verify-targets.sh          # desktop + Android
cd app/src-tauri && ./scripts/verify-targets.sh desktop  # desktop only, no NDK needed
```

It runs both targets sequentially, `--release`, with `-D warnings`. A change is not done until both are clean.

---

## 1. `target_os = "linux"` is **false** on Android

This is the single biggest trap, and no compiler will ever flag it.

Android reports `target_os = "android"`, **not** `"linux"`. So every `#[cfg(target_os = "linux")]` block is *silently dead* on device — no error, no warning, just code that never runs.

Meanwhile:

| cfg | On Android |
|---|---|
| `#[cfg(target_os = "linux")]` | **DEAD** |
| `#[cfg(unix)]` | **LIVE** |
| `#[cfg(desktop)]` (Tauri: linux\|macos\|windows) | **DEAD** |
| `#[cfg(not(desktop))]` (android\|ios) | **LIVE** |

**Current exposure:** 61 `#[cfg(target_os = "linux")]` blocks across 14 files are dead on Android, versus 24 `#[cfg(desktop)]` blocks.

Get the 14 files with `rg -l 'cfg\(target_os = "linux"\)' app/src-tauri/src/`.

**Rule:** for "not on a phone", use `#[cfg(desktop)]` / `#[cfg(not(desktop))]`. Never `#[cfg(target_os = "linux")]` unless you specifically mean desktop-Linux. Use `#[cfg(unix)]` deliberately, remembering it includes Android.

---

## 2. `clippy-driver` honors `SYSROOT`; `rustc` does not

Exporting `SYSROOT=<NDK>/.../sysroot` breaks **every** Android clippy run:

```
error[E0463]: can't find crate for `std`
  = note: the `x86_64-unknown-linux-gnu` target may not be installed
error: could not compile `Vox` (build script) due to 1 previous error
```

`cargo clippy` compiles this crate's **build script for the host** with `clippy-driver`, which treats `SYSROOT` as an authoritative sysroot override and looks for host `x86_64` std inside the *Android* sysroot. `rustc` ignores the variable entirely, which is why `cargo check --target` succeeds and hides this.

**Rule:** never export `SYSROOT`. Inline it into `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android`. The script does this and unsets `SYSROOT` defensively.

---

## 3. `cargo check --target X` is not a gate

It passes while target-specific dead code sits in the tree, and it structurally cannot surface it. Batch 4 was reported complete on the strength of it while 6 of 10 planned exclusions had not been made.

Use clippy on the Android target. Both, always.

---

## 4. Desktop-only dependencies must be keyed off `not(android/ios)`

`Cargo.toml:103` uses the correct shape:

```toml
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
arboard = ...
enigo = { version = "0.2", default-features = false }
```

Do **not** key these on `cfg(target_os = "linux")`. `enigo` has no Android backend at all (X11/Wayland/XDO/libei are all desktop stacks), and keying on `linux` silently drops the feature and then hard-fails at `enigo/src/linux/mod.rs:15`.

`Cargo.toml:109-114` *does* still key `gtk` / `webkit2gtk` / `cairo-rs` / `libc` / the `x11rb` enigo variant on `target_os = "linux"` — that is intentional and correct, since those deps have no mobile equivalent.

---

## 5. Dual-arm stub pattern — do not delete the mobile arm

IPC commands are defined **twice**, not excluded:

```rust
#[cfg(desktop)]
pub async fn hide_tray_window<R: tauri::Runtime>(app: AppHandle<R>) { /* real */ }

#[cfg(not(desktop))]
pub async fn hide_tray_window<R: tauri::Runtime>(_app: AppHandle<R>) {}   // stub
```

(`ipc/tray.rs:97,167` and `:122,170`.)

The mobile arm is what keeps the command **registered** in `generate_handler![]` on Android. Deleting it breaks the frontend contract on device. `services/dictation/` and `pipeline/dictation/` follow the same shape.

**This is why the raw `#[tauri::command]` count (72) exceeds the registered command count (70).** Do not "deduplicate" those.

---

## 6. Mobile is assistant-only

Dictation is excluded on Android **by necessity**, not by preference — `enigo` has no Android input backend. Unless dictation is reimplemented against Android's `InputManager`, Android gets assistant only.

Do not re-add `services/dictation/**` to the mobile build, and do not add a dictation entry point to the mobile UI.

---

## 7. Runtime remote inference already works; do not reach for `ssh`

`services/llm/transport/config.rs:8-13` exposes `TransportType::{ChatCompletions, OllamaNative, Responses}` over **plain HTTP**. An Android build can already talk to a remote GPU box with no Rust changes.

`setup/remote_server.rs` is a **one-time provisioning helper** — it spawns `ssh` to run `setup_server.sh` and stream setup progress (`:55-75,88`). It is *not* the inference path. Provisioning is a desktop-admin task; it is correctly gated off mobile. Making in-app provisioning work would need a pure-Rust SSH client (`russh`), which is a feature request, not a portability fix.

---

## 8. Model loading is already portable — do not add a platform layer

`paths.rs:347-349`:

```rust
pub fn model_dir(name: &str) -> PathBuf { get().models.join(name) }
```

Every provider builds from this. There are **zero** `home_dir`, absolute-path, or `CARGO_MANIFEST_DIR` assumptions under `services/{stt,llm,tts}/`. `dirs::data_local_dir()` resolves to app-private storage on Android and the relative layout is identical.

Download once, then models load exactly as on desktop. **No asset-pack abstraction, no per-platform path mapping.** The plan's original claim that "flat model layout is the real blocker" was backwards — flat layout is precisely why it ports.

The real defect is that `setup/model_manager.rs` is not resumable (see Batch 8), which is a robustness bug, not a platform one.

---

## 9. What is actually still broken

Do not assume the platform work is finished. As of the last verification:

- `setup/remote_server.rs` had **zero `cfg` tags** and would spawn a nonexistent `ssh`. Gated as of Phase 1 — confirm it is actually `#[cfg(desktop)]` before relying on it.
- `tauri android init` has **never been run** — `gen/android` does not exist, and `tauri.conf.json` has no `bundle.android` block.
- **Nothing has been proven to *link* for `arm64-v8a`.** `cargo check` compiles; it does not link. `turso`/`libsql` (§7.2) and `ort` NNAPI (§7.3) are both still unverified against a real link.
- No APK has ever been installed on a device. NNAPI and the `cpal` `oboe` audio backend are **hardware-dependent** — an emulator validates neither.

---

## 10. Before you finish

- [ ] `./scripts/verify-targets.sh` exits 0 (both targets)
- [ ] Did you add a `#[cfg(target_os = "linux")]`? Is that intentional given trap 1?
- [ ] Did you delete a `#[cfg(not(desktop))]` stub arm? Re-read trap 5.
- [ ] Did you add a desktop-only dependency? Re-read trap 4.
- [ ] Did you touch `model_dir`, `paths.rs`, or model loading? Re-read trap 8 — you probably didn't need to.
- [ ] Update `AGENTS.md` §5 per the sync hook.
---

## 11. Cargo finds `.cargo/config.toml` from CWD, not from the manifest

**The subtlest trap here, and the one that cost the most time.**

Cargo resolves config files by walking up from the **current working directory**. Build scripts are executed with cwd = *the crate's own source directory*:

```
~/.cargo/registry/src/.../llama-cpp-sys-4-0.2.61/     <- registry dep
<repo>/submodules/chatterbox-rs/                      <- path dep
```

Neither path is under `app/src-tauri/`. So a `[env]` block placed in
`app/src-tauri/.cargo/config.toml` is **never read by those crates** — the upward
search from `~/.cargo/registry/...` stops at `$CARGO_HOME`.

The symptom is that a bare `cargo build --target aarch64-linux-android` succeeds,
while `pnpm tauri android build` fails inside Gradle with:

```
/usr/include/features-time64.h:20:10: fatal error: 'bits/wordsize.h' file not found
```

**Fix:** put `[env]` in `$CARGO_HOME/config.toml` (`~/.cargo/config.toml`). That is
the only directory every crate's upward search reaches. `scripts/android-env.sh`
writes the block there idempotently, so an NDK version bump self-heals and CI
needs no manual step.

**Do not duplicate `[env]` into multiple config files.** Cargo merges them and
refuses mismatched shapes:

```
failed to merge key `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android` between
  app/src-tauri/.cargo/config.toml and ~/.cargo/config.toml
expected table, but found string
```

---

## 12. `tauri android build` must be scoped with `-t aarch64`

Without it, Gradle builds **all four ABIs**:

```
-PabiList=arm64-v8a,armeabi-v7a,x86,x86_64
-PtargetList=aarch64,armv7,i686,x86_64
```

`ort-sys` has no prebuilt `armv7-linux-androideabi` binary, so the build dies:

```
error: ort-sys@2.0.0-rc.13: no prebuilt binaries available for target armv7-linux-androideabi
```

It also costs ~4x the build time for slices you never ship.

**Always use:** `pnpm tauri android build --apk -t aarch64`

The target-specific bindgen var is named per-triple
(`BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android`), so scoping to one ABI also
means only one such variable needs to exist.

---

## 13. Gradle cannot be driven standalone — it dials back to the CLI

Do not try to bypass `tauri android build` by invoking `gradlew` directly, even
with a perfect environment. The Gradle task is a **client**, not a builder:

```
$ tauri android android-studio-script --release --target aarch64
panicked at tauri-cli/src/mobile/mod.rs:403:
failed to read CLI options: Connection refused
```

Gradle shells out to the Tauri CLI and expects to connect back over a **WebSocket**
to the still-running parent process. There is no `--ci` or env-passthrough flag,
and the env Tauri hands to Gradle is a hardcoded allowlist that omits
`BINDGEN_EXTRA_CLANG_ARGS*`, `CC_`/`CXX_`/`AR_*`, and `ANDROID_NDK`. That is why
trap 11's `$CARGO_HOME` approach is the only injection point that works.

---

## 14. `chatterbox-rs` needs its own Android toolchain branch

Its vendored ggml calls `ggml_get_system_arch()`
(`chatterbox-cpp/ggml/cmake/common.cmake`), which keys off `CMAKE_SYSTEM_PROCESSOR`.
Passing `-DCMAKE_SYSTEM_PROCESSOR=aarch64` is **not enough** — CMake recomputes
that variable inside `project()` and shadows the cache entry, so ggml sees the
host and selects its x86 backend:

```
-- CMAKE_SYSTEM_PROCESSOR: x86_64
-- x86 detected
-- Adding CPU backend variant ggml-cpu: -march=native
clang: error: unsupported argument 'native' to option '-march='
```

Only a `CMAKE_TOOLCHAIN_FILE` sets it early enough. `llama-cpp-sys-4/build.rs:1885`
already does this correctly; chatterbox did not. Fixed locally via a `[patch]`
pending an upstream bump — see §15.

---

## 15. The duplicate-ggml and ABI toolchain fixes live in a submodule

`llama-cpp-sys-4` and `chatterbox-rs` each vendor their own llama.cpp. Two
verifiers apply:

- `app/src-tauri/build.rs` — emits `cargo:rustc-link-arg=-Wl,--allow-multiple-definition`,
  scoped to `target_os == "android"`. **Not** `.cargo/config.toml`: Tauri exports
  `CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS`, and an env var beats config.toml,
  so a config-file rustflag is silently ignored in the Gradle path.
- `submodules/chatterbox-rs/build.rs` — the Android toolchain branch from trap 14.
- `app/src-tauri/Cargo.toml` — a `[patch]` redirecting chatterbox-rs to the local
  submodule. **It must stay at the end of the file**: a `[patch]` table header
  terminates `[dependencies]`, so placing it mid-file orphans every later dep.
