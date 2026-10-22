//! ============================================================================
//! ipc_command_registration_test.rs — every `#[tauri::command]` is reachable from `generate_handler![]`, and every desktop-only command keeps a mobile stub arm
//! ============================================================================
//! Category  : Integration (Seam 00 — source invariant)
//! Component : src/lib.rs, src/ipc/**/*.rs
//! Binary    : ipc_command_registration_test
//! Prereqs   : none
//! Run       : cargo nextest run --test ipc_command_registration_test --release --test-threads=1
//! ============================================================================
//!
//! WHY THIS EXISTS
//! ---------------
//! Several IPC commands are defined TWICE — a real body under `#[cfg(desktop)]`
//! and a no-op stub under `#[cfg(not(desktop))]`. That pairing is deliberate: it
//! is what keeps the command REGISTERED on Android, so the frontend contract
//! does not silently break on device.
//!
//! The failure mode is dangerous. An agent refactoring `ipc/tray.rs` can delete
//! what looks like a redundant mobile stub. The crate still compiles on both
//! targets and `generate_handler![]` still lists the name, so no compiler
//! catches it — but on Android the command resolves to the desktop-only symbol
//! and the frontend call fails at runtime, discovered only on a device.
//!
//! These tests make that class of regression impossible to merge silently.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! This parses source text; it does not evaluate `cfg` expressions. So it can
//! only reason about the *presence and text* of guards, never their truth on a
//! given target. The stub test checks that a command guarded for desktop also
//! has an arm whose predicate textually negates that — which is exactly the
//! stub-loss regression. It cannot prove the predicate compiles to the right
//! thing.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Root of the `Vox` crate under test.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Recursively collect every `.rs` file under `dir`.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_rs_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Extract the identifier list from the `tauri::generate_handler![ ... ]` block
/// in `lib.rs`.
///
/// Written line-wise rather than assuming a fixed column, so a `rustfmt` pass
/// cannot break it. Comment lines (used for section banners) are skipped.
fn extract_registered_commands(lib_rs: &str) -> BTreeSet<String> {
    let mut registered = BTreeSet::new();

    let Some(start) = lib_rs.find("generate_handler![") else {
        panic!(
            "lib.rs: no `tauri::generate_handler![` block found — the IPC\n\
                registration block was renamed or removed. This test must be\n\
                updated to match."
        );
    };

    let body = &lib_rs[start..];
    // The opening `[` sits on the first line, mid-expression (after
    // `.invoke_handler(tauri::generate_handler!`), so it cannot be found by
    // looking for a line that *starts* with `[`. Consume it here instead.
    let mut depth: usize = 1;
    let mut first_line = true;

    for line in body.lines() {
        if first_line {
            first_line = false;
            continue;
        }

        let code = line.split("//").next().unwrap_or("").trim();

        depth += code.matches('[').count();
        depth = depth.saturating_sub(code.matches(']').count());

        if depth == 0 {
            break;
        }

        if let Some(name) = code.strip_suffix(',') {
            let name = name.trim();
            if !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                registered.insert(name.to_string());
            }
        }
    }

    registered
}

/// Extract every `#[tauri::command]` under `src/ipc/` as `name -> guards`.
///
/// `guards` holds the effective guard text for each definition: the enclosing
/// `#[cfg(..)] mod { .. }` scopes (from brace-depth tracking) combined with any
/// attribute directly on the item. An unconditional command maps to `[""]`.
///
/// Tracking the enclosing module matters. In `ipc/tray.rs` the guard sits on the
/// *module*, not the function:
///
/// ```ignore
/// #[cfg(desktop)]
/// mod desktop {
///     #[cfg(target_os = "linux")]      // <-- a `use` guard, NOT the fn's guard
///     use gtk::prelude::*;
///     #[tauri::command]                // effective guard: `desktop`
///     pub async fn hide_tray_window(..) {}
/// }
/// ```
///
/// A naive "most recent `#[cfg]` wins" scan reports that `use` guard instead and
/// silently misses the desktop/non-desktop pairing this test exists to enforce.
fn extract_defined_commands(ipc_dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut defined: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for file in collect_rs_files(ipc_dir) {
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };

        // Enclosing cfg'd module scopes, innermost last. Each entry records the
        // brace depth at which the module body started, so it pops when that
        // depth is left again.
        let mut scopes: Vec<(i32, String)> = Vec::new();
        let mut depth: i32 = 0;

        let mut pending_cfg: Option<String> = None;
        let mut pending_cmd: bool = false;
        let mut cmd_local_cfg: Option<String> = None;

        for line in source.lines() {
            let code = line.split("//").next().unwrap_or("").trim();

            if code.starts_with("#[cfg(") {
                // `#[cfg(not(desktop))]` -> `not(desktop))]` -> drop the `)]`
                // PAIR. Trimming chars individually would also eat the `)` that
                // belongs to `not(...)`, yielding `not(desktop`.
                let inner = code.trim_start_matches("#[cfg(");
                let predicate = inner
                    .strip_suffix(")]")
                    .or_else(|| inner.strip_suffix(']'))
                    .unwrap_or(inner);
                pending_cfg = Some(predicate.trim().to_string());
                continue;
            }

            if code.starts_with("#[") {
                if code.contains("#[tauri::command]") {
                    pending_cmd = true;
                    cmd_local_cfg = pending_cfg.take();
                }
                continue;
            }

            let opens = code.matches('{').count() as i32;
            let closes = code.matches('}').count() as i32;

            // A `#[cfg(..)]` directly governing a `mod { .. }` scopes its body.
            if let Some(guard) = pending_cfg.take() {
                if code.contains("mod") && opens > 0 {
                    scopes.push((depth, guard));
                }
            }

            if pending_cmd {
                if let Some(idx) = code.find("fn ") {
                    let name: String = code[idx + 3..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        let mut parts: Vec<String> =
                            scopes.iter().map(|(_, g)| g.clone()).collect();
                        if let Some(local) = cmd_local_cfg.take() {
                            parts.push(local);
                        }
                        defined.entry(name).or_default().push(parts.join(" && "));
                    }
                    pending_cmd = false;
                } else if !code.is_empty() {
                    pending_cmd = false;
                }
            }

            depth += opens - closes;
            while scopes.last().is_some_and(|(d, _)| depth <= *d) {
                scopes.pop();
            }
        }
    }

    defined
}

/// True when `guard` is a desktop predicate (`desktop`, or one that mentions it
/// as the satisfied side, e.g. `all(unix, desktop)`).
fn is_desktop_guard(guard: &str) -> bool {
    let g = guard.replace(char::is_whitespace, "");
    g.contains("desktop") && !g.contains("not(desktop)")
}

/// True when `guard` is a non-desktop predicate.
///
/// Only the `not(desktop)` form is recognised, which is the sole form used in
/// `src/ipc/` today. A more exotic negation (e.g. `not(any(desktop, ...))`)
/// would not match and would surface as a reported offender — a false
/// positive that points an agent at a real guard, rather than a silent pass.
fn is_non_desktop_guard(guard: &str) -> bool {
    let g: String = guard.chars().filter(|c| !c.is_whitespace()).collect();
    g.contains("not(desktop)")
}

/// PURPOSE:  The registration parser must find the `generate_handler![]` block
///           in `lib.rs`; if the block is renamed or removed, every IPC
///           registration check silently passes with an empty set.
/// SCENARIO: Parse the real `src/lib.rs` and inspect the extracted names.
/// ASSERTS:  At least one command name is extracted from the registration block.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Rename `tauri::generate_handler![` in `src/lib.rs` — the parser
///           would then find nothing and every registration assertion would
///           become vacuous.
#[test]
fn handler_registration_block_yields_command_names() {
    let root = crate_root();
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");

    let registered = extract_registered_commands(&lib_rs);

    assert!(
        !registered.is_empty(),
        "no commands parsed from the `generate_handler![]` block in lib.rs"
    );
    assert!(
        registered.contains("get_settings"),
        "the parsed registration set must contain the known `get_settings` command"
    );
}

/// PURPOSE:  The definition scanner must find the `#[tauri::command]` functions
///           under `src/ipc/`; if the IPC layout moves, the registration check
///           compares against an empty set and always passes.
/// SCENARIO: Scan the real `src/ipc/` directory tree.
/// ASSERTS:  The extracted map is non-empty and every entry has at least one
///           guard record.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Move `src/ipc/*.rs` to another directory — the scanner would find
///           no commands and the "defined but not registered" check would pass
///           vacuously.
#[test]
fn ipc_command_definitions_are_scanned_with_guards() {
    let root = crate_root();
    let ipc_dir = root.join("src/ipc");

    let defined = extract_defined_commands(&ipc_dir);

    assert!(
        !defined.is_empty(),
        "no `#[tauri::command]` functions found under {}. Either the IPC layout\n\
         moved or this test is pointed at the wrong directory.",
        ipc_dir.display()
    );
    for (name, guards) in &defined {
        assert!(
            !guards.is_empty(),
            "command `{name}` must carry at least one guard record"
        );
    }
}

/// PURPOSE:  A command that is defined but not listed in `generate_handler![]`
///           is unreachable from the frontend on every target, silently.
/// SCENARIO: Compare the set of `#[tauri::command]` names under `src/ipc/`
///           against the names registered in `src/lib.rs`.
/// ASSERTS:  Every defined command name appears in the registered set.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Delete `get_settings,` from `tauri::generate_handler![]` in
///           `src/lib.rs` — the frontend call would then fail at runtime.
#[test]
fn defined_command_is_registered_in_generate_handler() {
    let root = crate_root();
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let ipc_dir = root.join("src/ipc");

    let defined = extract_defined_commands(&ipc_dir);
    let registered = extract_registered_commands(&lib_rs);

    assert!(
        !defined.is_empty(),
        "no `#[tauri::command]` functions found under {}. Either the IPC layout\n\
         moved or this test is pointed at the wrong directory.",
        ipc_dir.display()
    );
    assert!(
        !registered.is_empty(),
        "no commands parsed from the `generate_handler![]` block in lib.rs"
    );

    let unregistered: Vec<&String> = defined
        .keys()
        .filter(|name| !registered.contains(*name))
        .collect();

    assert!(
        unregistered.is_empty(),
        "\n{} IPC command(s) are defined with `#[tauri::command]` but are NOT\n\
         listed in `tauri::generate_handler![]` in src/lib.rs:\n  {}\n\n\
         A command that is defined but not registered is unreachable from the\n\
         frontend on every target. Add the name to generate_handler![].\n\n\
         defined but not registered: {:?}",
        unregistered.len(),
        unregistered
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        unregistered
    );
}

/// PURPOSE:  Deleting the mobile stub arm of a dual-arm command compiles
///           cleanly on both targets and breaks at runtime on Android only.
/// SCENARIO: For every command guarded by a desktop predicate, look for a
///           second arm guarded by the negated predicate.
/// ASSERTS:  Every desktop-guarded command also has a non-desktop arm.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Delete the `#[cfg(not(desktop))]` arm of a command in
///           `src/ipc/tray.rs` — Android would then resolve to a desktop-only
///           symbol and fail at runtime.
#[test]
fn desktop_guarded_command_retains_non_desktop_stub() {
    let root = crate_root();
    let defined = extract_defined_commands(&root.join("src/ipc"));

    // Commands that exist under a desktop guard must ALSO exist under a
    // non-desktop guard. Otherwise deleting the mobile stub compiles cleanly on
    // both targets and only breaks at runtime, on device.
    let offenders: Vec<(&String, &Vec<String>)> = defined
        .iter()
        .filter(|(_, guards)| guards.iter().any(|g| is_desktop_guard(g)))
        .filter(|(_, guards)| !guards.iter().any(|g| is_non_desktop_guard(g)))
        .collect();

    assert!(
        offenders.is_empty(),
        "\n{} command(s) exist under a desktop `#[cfg]` guard but have NO arm\n\
         guarded for mobile. On Android these commands would resolve to a\n\
         desktop-only symbol and fail at runtime, with no compile-time warning.\n\n\
         Fix: add a no-op stub arm, mirroring ipc/tray.rs:\n\
             #[cfg(not(desktop))]\n\
             pub async fn NAME<R: tauri::Runtime>(_app: AppHandle<R>) {{}}\n\n\
         offenders (name -> guards):\n  {}\n\n\
         See .agents/rules/android-pitfalls.md trap 5.",
        offenders.len(),
        offenders
            .iter()
            .map(|(n, g)| format!("{} -> {:?}", n, g))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// PURPOSE:  The desktop-guard classifier decides which commands need a mobile
///           stub; a wrong classifier would report the wrong offender set.
/// SCENARIO: Feed representative `cfg` predicate texts to the classifier.
/// ASSERTS:  `desktop` and `all(unix, desktop)` are desktop; `not(desktop)` is
///           not; the two classifiers are mutually exclusive on those inputs.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Drop the `!g.contains("not(desktop)")` clause from
///           `is_desktop_guard` — `not(desktop)` arms would then be counted as
///           desktop arms and every stub would look self-satisfied.
#[test]
fn desktop_guard_predicate_is_classified_correctly() {
    assert!(is_desktop_guard("desktop"));
    assert!(is_desktop_guard("all(unix, desktop)"));
    assert!(!is_desktop_guard("not(desktop)"));
    assert!(!is_desktop_guard(""));

    assert!(is_non_desktop_guard("not(desktop)"));
    assert!(!is_non_desktop_guard("desktop"));
    assert!(!is_non_desktop_guard("all(unix, desktop)"));

    assert!(
        !(is_desktop_guard("not(desktop)") && is_non_desktop_guard("not(desktop)")),
        "the two guard classifiers must not both accept the same predicate"
    );
}

/// PURPOSE:  The registration parser must ignore banner comments and inline
///           comments, otherwise a commented-out command would be reported as
///           registered (a false green).
/// SCENARIO: Parse a synthetic `generate_handler![]` block containing comment
///           lines and an inline trailing comment.
/// ASSERTS:  Only the three real command names are extracted.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the `line.split("//")` strip in `extract_registered_commands`
///           — the commented-out name would then be treated as registered.
#[test]
fn registration_parser_ignores_comment_lines() {
    let synthetic = r#"
        .invoke_handler(tauri::generate_handler![
            alpha,
            // beta is retired and must not be parsed
            gamma, // inline comment
        ])
    "#;

    let registered = extract_registered_commands(synthetic);

    assert_eq!(
        registered,
        BTreeSet::from([
            "alpha".to_string(),
            "gamma".to_string(),
        ]),
        "comment lines and inline comments must not contribute command names"
    );
}

/// PURPOSE:  The registration parser must stop at the end of the
///           `generate_handler![]` block; reading past it would import
///           unrelated identifiers as registered commands.
/// SCENARIO: Parse a synthetic block followed by an unrelated identifier.
/// ASSERTS:  The identifier after the closing bracket is not in the set.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the `if depth == 0 { break; }` early exit in
///           `extract_registered_commands` — identifiers past the block would
///           be counted as registered.
#[test]
fn registration_parser_stops_at_block_end() {
    let synthetic = r#"
        .invoke_handler(tauri::generate_handler![
            alpha,
        ])
        .run(|app_handle, event| {
            let not_a_command = 1;
        })
    "#;

    let registered = extract_registered_commands(synthetic);

    assert_eq!(
        registered,
        BTreeSet::from(["alpha".to_string()]),
        "the parser must stop at the closing bracket of the registration block"
    );
}
