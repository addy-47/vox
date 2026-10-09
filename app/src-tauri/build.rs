fn main() {
    // ── Android duplicate-ggml link fix ──────────────────────────────────────
    //
    // `llama-cpp-sys-4` and `chatterbox-rs` each vendor and statically build
    // their OWN copy of llama.cpp/ggml (chatterbox-rs has no Rust deps beyond
    // cc/cmake/libc). Both export the same `ggml_*` symbols, so the final link
    // fails with `ld.lld: error: duplicate symbol: ggml_*`.
    //
    // The obvious fix is `-C link-arg=-Wl,--allow-multiple-definition` in
    // `.cargo/config.toml` under `[target.aarch64-linux-android]`. That WORKS for
    // a bare `cargo build`, but is silently ignored by `pnpm tauri android build`:
    // Tauri exports `CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS`, and per cargo's
    // precedence rules the env var beats the config file. The flag must therefore
    // be injected from here, which no RUSTFLAGS value can override.
    //
    // Desktop targets keep using the config.toml form (no env override there).
    //
    // Consequence, accepted: the linker collapses both archives onto ONE ggml.
    // That is the same outcome desktop has shipped with, so it is not a new risk
    // class -- but it does require both crates to stay ABI-compatible with
    // whichever ggml wins. Verified only at link time; device testing must
    // confirm chatterbox TTS still runs.
    //
    // Scoped to Android deliberately: `-Wl,--allow-multiple-definition` is a GNU
    // ld / lld flag. MSVC's link.exe and Apple's ld64 both reject it, and neither
    // needs it (desktop Linux already gets it from .cargo/config.toml).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
    }

    tauri_build::build();
}
