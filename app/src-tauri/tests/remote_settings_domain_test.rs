//! ============================================================================
//! tests/remote_settings_domain_test.rs — Remote Settings Domain & Secret List
//! ============================================================================
//! Category     : Integration Test (Seam 15 — settings contract)
//! Component    : config/{settings,files,mutation,contract,mod}.rs + ipc/settings.rs
//! Prerequisites: None (no models, no audio hardware); isolated temp VOX_HOME
//! Execution    : cargo nextest run --test remote_settings_domain_test --release --nocapture --test-threads=1
//! Metrics      : (a) `remote` is in the supported-domains list with both of
//!                its keys, both are mutable through the production router,
//!                and both survive a save → load round trip; (b) the Azure
//!                Speech key is treated as a secret on the desktop path.
//!
//! WHY THIS EXISTS
//! ---------------
//! The `remote` domain is the first-class settings surface of the remote
//! control feature: the host toggles `remote.accepts_controllers`, the
//! controller toggles `remote.execution_mode`. A domain that is missing from
//! the supported-domains list is silently un-mutable, and a domain missing
//! from the per-domain key list is rejected with "Unknown setting" at the IPC
//! boundary. Both are runtime failures with no compile-time signal.
//!
//! The second half of this file guards a live secret leak: the Azure Speech
//! API key is a writable, readable setting that was missing from the shared
//! secret-key list, so it was not masked in diagnostic logging and would have
//! been forwarded verbatim to a controller.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! This exercises the production settings contract, the production mutation
//! router and the production load/save path, on the **desktop** path. It does
//! not exercise the controller-facing payload, because that payload does not
//! exist yet: the secret list is the source of truth the later redaction wave
//! consumes. It is therefore not VERIFY-RC-08 coverage.
//! ============================================================================

mod common;

use std::time::Duration;

use common::paths::TempPathsGuard;
use vox_lib::{
    config::{
        apply_setting_mutation, build_settings_contract, classify_known, get_setting_reload_policy,
        is_secret_key, secret_keys, supported_keys_for_domain, SettingReloadPolicy, VoxSettings,
    },
    core::events::RemoteExecutionMode,
};

/// The settings domain under test.
const DOMAIN: &str = "remote";

/// The two keys the domain carries.
const KEY_ACCEPTS: &str = "accepts_controllers";
const KEY_EXECUTION_MODE: &str = "execution_mode";

// ============================================================================
// Test 1: The domain is in the contract and is mutable
// ============================================================================

/// Verifies:
///   - `remote` is listed in the supported-domains list, with both of its
///     keys and a reload policy for each.
///   - The production mutation router recognises both keys.
///   - `execution_mode` accepts the two documented modes and rejects anything
///     else with an explicit error rather than a silent default.
///   - Both keys survive the production save → load cycle unchanged, and a
///     file that omits the `remote` section falls back to the defaults.
#[test]
fn test_the_remote_domain_is_in_the_contract_and_is_mutable() {
    let _guard = TempPathsGuard::new();

    // 1. The contract.
    let contract = build_settings_contract();
    assert!(
        contract.supported_keys.contains_key(DOMAIN),
        "The `{}` domain must appear in the settings contract sent to the frontend. \
         supported_keys domains: {:?}",
        DOMAIN,
        contract.supported_keys.keys().collect::<Vec<_>>()
    );
    let contract_keys = &contract.supported_keys[DOMAIN];
    for expected in [KEY_ACCEPTS, KEY_EXECUTION_MODE] {
        assert!(
            contract_keys.iter().any(|k| k == expected),
            "The contract must list `{}.{}`; got {:?}",
            DOMAIN,
            expected,
            contract_keys
        );
    }
    let policies = contract
        .reload_policies
        .get(DOMAIN)
        .expect("The contract must carry a reload policy for the remote domain");
    for expected in [KEY_ACCEPTS, KEY_EXECUTION_MODE] {
        assert!(
            policies.contains_key(expected),
            "The contract must classify `{}.{}`; got {:?}",
            DOMAIN,
            expected,
            policies.keys().collect::<Vec<_>>()
        );
    }

    // 2. The reload policy is explicit for both keys, so the classifier never
    //    falls through to its unknown-key default.
    for key in [KEY_ACCEPTS, KEY_EXECUTION_MODE] {
        let classified = classify_known(DOMAIN, key);
        assert!(
            classified.is_some(),
            "`{}.{}` must be classified explicitly, not fall through to the unknown-key default",
            DOMAIN,
            key
        );
        assert_eq!(
            get_setting_reload_policy(DOMAIN, key),
            classified.expect("classification must exist"),
            "`{}.{}` must resolve to the same policy from both entry points",
            DOMAIN,
            key
        );
    }
    assert_eq!(
        classify_known(DOMAIN, KEY_ACCEPTS),
        Some(SettingReloadPolicy::Hot),
        "Toggling controller acceptance is a runtime action, not an engine restart"
    );
    assert_eq!(
        classify_known(DOMAIN, KEY_EXECUTION_MODE),
        Some(SettingReloadPolicy::Restart),
        "Switching between the local pipeline and thin-client mode changes what the engine is"
    );

    // 3. The defaults are the standalone-install behaviour.
    let defaults = VoxSettings::default();
    assert!(
        !defaults.remote.accepts_controllers,
        "A fresh install must not accept remote controllers by default"
    );
    assert_eq!(
        defaults.remote.execution_mode,
        RemoteExecutionMode::Local,
        "A fresh install must run its own pipeline by default"
    );

    // 4. The mutation router recognises both keys.
    let mut settings = VoxSettings::default();
    assert!(
        apply_setting_mutation(&mut settings, DOMAIN, KEY_ACCEPTS, &serde_json::json!(true))
            .expect("remote.accepts_controllers mutation must succeed"),
        "remote.accepts_controllers must be a recognised key"
    );
    assert!(settings.remote.accepts_controllers);
    assert!(
        apply_setting_mutation(
            &mut settings,
            DOMAIN,
            KEY_EXECUTION_MODE,
            &serde_json::json!("remote")
        )
        .expect("remote.execution_mode mutation must succeed"),
        "remote.execution_mode must be a recognised key"
    );
    assert_eq!(settings.remote.execution_mode, RemoteExecutionMode::Remote);

    // An unknown mode is rejected explicitly, not coerced.
    let rejected = apply_setting_mutation(
        &mut settings,
        DOMAIN,
        KEY_EXECUTION_MODE,
        &serde_json::json!("telepathy"),
    );
    assert!(
        rejected.is_err(),
        "remote.execution_mode must reject an unknown mode"
    );
    assert_eq!(
        settings.remote.execution_mode,
        RemoteExecutionMode::Remote,
        "A rejected mutation must not change the stored value"
    );

    // 5. Both keys survive the production save → load cycle.
    settings
        .save()
        .expect("VoxSettings::save must persist the remote domain");
    let reloaded = VoxSettings::load();
    assert!(
        reloaded.remote.accepts_controllers,
        "remote.accepts_controllers must survive a save/load round trip"
    );
    assert_eq!(
        reloaded.remote.execution_mode,
        RemoteExecutionMode::Remote,
        "remote.execution_mode must survive a save/load round trip"
    );

    // 6. A settings file without the `remote` section falls back to defaults.
    let settings_path = vox_lib::utils::paths::settings_path();
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "appearance": { "theme": "nordic_frost", "accent_seed": "#00DBE9" }
        }))
        .expect("fixture must serialise"),
    )
    .expect("Failed to write the partial settings fixture");

    let recovered = VoxSettings::load();
    assert_eq!(
        recovered.appearance.theme, "nordic_frost",
        "The surviving sections must still be recovered"
    );
    assert!(
        !recovered.remote.accepts_controllers,
        "A missing remote section must fall back to the default, not to a stale value"
    );
    assert_eq!(
        recovered.remote.execution_mode,
        RemoteExecutionMode::Local,
        "A missing remote section must fall back to the default execution mode"
    );
}

// ============================================================================
// Test 2: The Azure Speech key is treated as a secret on the desktop path
// ============================================================================

/// Verifies:
///   - The Azure Speech key is in the shared secret-key list, and every
///     secret-bearing key is a supported key.
///   - The desktop's own `get_settings` response still carries the real Azure
///     key: redaction is a controller-facing concern.
///   - The real `update_setting` path masks the Azure key in the log it emits
///     and in the engine-restart reason, while the persisted value stays the
///     real key.
#[tokio::test]
async fn test_the_azure_speech_key_is_treated_as_a_secret_on_the_desktop_path() {
    tokio::time::timeout(Duration::from_secs(30), async {
        // 1. The classification.
        assert!(
            is_secret_key("tts", "azure_tts"),
            "The Azure Speech API key is secret material and must be in the secret-key list"
        );
        assert!(
            secret_keys()
                .iter()
                .any(|(d, k)| *d == "tts" && *k == "azure_tts"),
            "The secret-key list must contain (tts, azure_tts); got {:?}",
            secret_keys()
        );
        for (domain, key) in secret_keys() {
            assert!(
                supported_keys_for_domain(domain).contains(key),
                "Secret key {}.{} is not a supported key — the secret list and the mutation \
                 contract have drifted",
                domain,
                key
            );
        }
        let contract = build_settings_contract();
        assert!(
            contract
                .secret_keys
                .iter()
                .any(|s| s.domain == "tts" && s.key == "azure_tts"),
            "The settings contract must report the Azure Speech key as a secret; got {:?}",
            contract.secret_keys
        );

        // 2. The desktop path carries the real key: the desktop is the machine
        //    that holds the keys and needs them to reach its own providers.
        let _guard = TempPathsGuard::new();
        let mut settings = VoxSettings::default();
        settings.providers.tts.azure_tts.key = "azure-super-secret-key".to_string();
        settings.providers.tts.azure_tts.region_or_endpoint = "westeurope".to_string();
        settings
            .save()
            .expect("VoxSettings::save must persist the Azure configuration");

        let (app, state) = common::harness::get_test_app_and_state().await;
        let mutating_app = app.clone();
        let boot = vox_lib::ipc::settings::get_settings(app)
            .await
            .expect("get_settings must succeed");
        assert_eq!(
            boot.settings.providers.tts.azure_tts.key, "azure-super-secret-key",
            "The desktop's own settings response must carry the real Azure key"
        );

        // 3. The real update path masks the key in the log it emits. The
        //    masking happens in the command, not in the list.
        common::log_capture::install();
        state
            .restart_runner
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let real_key = "azure-super-secret-key";
        let azure = vox_lib::services::tts::TtsAzureConfig {
            region_or_endpoint: "westeurope".to_string(),
            voice: "en-US-JennyNeural".to_string(),
            key: real_key.to_string(),
            ..Default::default()
        };
        let value = serde_json::to_value(&azure).expect("the Azure config must serialise");

        let _ = common::log_capture::take();

        let result = vox_lib::ipc::settings::update_setting(
            "tts".to_string(),
            "azure_tts".to_string(),
            value,
            mutating_app,
        )
        .await
        .expect("update_setting must accept the Azure configuration");
        assert!(result.applied, "The Azure configuration must be applied");

        let captured = common::log_capture::take_text();
        assert!(
            !captured.contains(real_key),
            "The Azure key must never reach a log line. Captured:\n{}",
            captured
        );
        assert!(
            captured.contains("[redacted]"),
            "The masked Azure value must appear in the log. Captured:\n{}",
            captured
        );
        assert!(
            captured.contains("tts.azure_tts = "),
            "The settings update message must be logged. Captured:\n{}",
            captured
        );

        // 4. The applied value is still the real key: masking is for display
        //    only, never for the value the provider authenticates with.
        let stored = state
            .settings
            .read()
            .expect("the settings lock must be readable")
            .providers
            .tts
            .azure_tts
            .key
            .clone();
        assert_eq!(
            stored, real_key,
            "The persisted Azure key must be the real key, not the masked one"
        );

        state
            .restart_runner
            .store(false, std::sync::atomic::Ordering::SeqCst);
        state
            .restart_requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .await
    .expect("test_the_azure_speech_key_is_treated_as_a_secret_on_the_desktop_path timed out");
}
