//! ============================================================================
//! tests/remote_connection_test.rs — Remote Control Connection Seam (Seam 03)
//! ============================================================================
//! Category  : Integration (Seam 03)
//! Component : src/services/remote/{session, security, control, link}
//! Binary    : remote_connection_test
//! Prereqs   : none
//! Run       : cargo nextest run --test remote_connection_test --release --nocapture --test-threads=1
//! ============================================================================

mod common;

use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use tempfile::tempdir;
use vox_lib::{
    persistence::{
        paired_devices::{
            get_paired_device, insert_paired_device, list_paired_devices, NewPairedDevice,
            PairedDeviceRole,
        },
        schema::run_migrations,
        VoxDb,
    },
    services::remote::{
        control::{
            guard::{is_command_permitted, is_desktop_command_permitted, ControlOrigin},
            routing::{get_routing_table, CommandClass},
        },
        link::{bind, LinkCloseReason, LoopbackLink, RemoteLink},
        protocol::{
            framing::StreamFormat,
            messages::{
                ControlMessage, Hello, LinkLossReason, PairApproved, PairDenied, SenderIdentity,
            },
        },
        security::{
            default_noise_prologue, derive_device_id, DeviceKey, NoiseHandshake,
            PairingCoordinator, PairingOutcome, PairingRequest,
        },
        session::{RemoteLinkState, RemoteSessionManager, SessionRole},
    },
};

/// Bounded timeout for asynchronous operations in this seam.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Bounds an async future with TEST_TIMEOUT.
async fn within<F: Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(TEST_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!("[{label}] operation did not complete within {TEST_TIMEOUT:?}"),
    }
}

/// Bounds a fallible async operation with TEST_TIMEOUT and unwraps.
async fn within_ok<F, T, E>(label: &str, future: F) -> T
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match within(label, future).await {
        Ok(value) => value,
        Err(err) => panic!("[{label}] operation failed: {err}"),
    }
}

/// Opens an isolated migrated Turso test database.
async fn open_migrated_db(dir: &std::path::Path, file_name: &str) -> (VoxDb, turso::Connection) {
    let db = VoxDb::open(dir.join(file_name))
        .await
        .expect("database open failed");
    let conn = db.connect().expect("connection vend failed");
    run_migrations(&conn).await.expect("migrations failed");
    (db, conn)
}

// ============================================================================
// Subtests
// ============================================================================

/// PURPOSE:  A full controller-initiated pairing via pair_device must reach Connected state and persist peers in Turso DB on both sides.
/// SCENARIO: Controller calls pair_device with valid pairing token and host public key over loopback; Host accepts, validates request through PairingCoordinator, approves, and completes Hello negotiation.
/// ASSERTS:  Both Controller and Host reach RemoteLinkState::Connected, and each peer is inserted into their respective Turso DB paired_devices table.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove insert_paired_device call in pair_device for host persistence.
#[tokio::test]
async fn remote_connection_pairing_handshake_approved_reaches_connected() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (_host_db, host_conn) = open_migrated_db(dir.path(), "host_pair.db").await;
    let (controller_db, controller_conn) = open_migrated_db(dir.path(), "controller_pair.db").await;

    // 1. Host key and loopback listener
    let host_key = DeviceKey::generate().expect("generate host key");
    let host_id = derive_device_id(host_key.public_key());
    let host_pk_hex = hex::encode(host_key.public_key());

    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let addr = listener.local_addr().expect("listener address");

    // 2. Host issues pairing token
    let coordinator = PairingCoordinator::new();
    let now = Instant::now();
    let token = coordinator.issue_token(now).expect("issue token");

    // 3. Controller initiates dial_and_connect in background task
    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    let cm = Arc::clone(&controller_manager);
    let c_db = Arc::new(controller_db);
    let target = format!("127.0.0.1:{}", addr.port());
    let h_id = host_id.as_str().to_string();
    let tok_str = token.to_hex();
    let hpk_str = host_pk_hex.clone();

    let dial_task = tokio::spawn(async move {
        cm.pair_device(c_db, target, tok_str, hpk_str, h_id, None)
            .await
    });

    // 4. Host accepts connection
    let host_ws_link = within_ok("accept connection", listener.accept()).await;

    // 5. Host receives PairRequest
    let initial_msg = within_ok("recv pair request", host_ws_link.recv_control()).await;
    let claimed_req = match initial_msg {
        ControlMessage::PairRequest(req) => req,
        other => panic!("Expected PairRequest, received {other:?}"),
    };

    let pubkey_bytes = hex::decode(&claimed_req.public_key).expect("decode controller pubkey");
    let pairing_req = PairingRequest::from_wire(
        &claimed_req.token,
        &claimed_req.device_id,
        &claimed_req.public_key,
        &claimed_req.display_name,
    )
    .expect("valid wire pairing request");

    let pending = coordinator
        .begin(
            pairing_req,
            &pubkey_bytes,
            "127.0.0.1".parse().expect("valid ip"),
            Instant::now(),
        )
        .expect("begin pairing");

    let prompt = pending.prompt();
    assert!(prompt.approve());
    let outcome = pending.settle().await;
    assert!(matches!(outcome, PairingOutcome::Approved { .. }));

    // 6. Host inserts controller into its DB
    insert_paired_device(
        &host_conn,
        &NewPairedDevice {
            device_id: claimed_req.device_id.clone(),
            public_key: pubkey_bytes,
            display_name: claimed_req.display_name.clone(),
            role: PairedDeviceRole::Controller,
            trust_level: "full".to_string(),
            hints: vec![],
        },
    )
    .await
    .expect("insert controller in host db");

    // 6b. Host drives Noise responder handshake
    let handshake =
        NoiseHandshake::responder(&host_key, None, &default_noise_prologue()).expect("handshake");
    let transport = host_ws_link
        .drive_handshake(handshake)
        .await
        .expect("drive handshake");
    host_ws_link.set_cipher(transport).await;

    // 7. Host sends PairApproved
    within_ok(
        "send PairApproved",
        host_ws_link.send_control(ControlMessage::PairApproved(PairApproved {
            host_identity: SenderIdentity {
                device_id: host_id.as_str().to_string(),
                display_name: "Vox Host".into(),
            },
        })),
    )
    .await;

    // 8. Controller dial_task finishes successfully
    within_ok("dial task result", dial_task)
        .await
        .expect("pair_device succeeded");

    // 9. Assertions
    assert_eq!(controller_manager.state(), RemoteLinkState::Paired);

    // Assert controller persisted host in Turso DB
    let stored_host = get_paired_device(&controller_conn, host_id.as_str())
        .await
        .expect("query host in controller db");
    assert!(
        stored_host.is_some(),
        "Host MUST be saved in controller's paired_devices"
    );
    assert_eq!(stored_host.unwrap().display_name, "Vox Host");

    // Assert host persisted controller in Turso DB
    let stored_controller = get_paired_device(&host_conn, &claimed_req.device_id)
        .await
        .expect("query controller in host db");
    assert!(
        stored_controller.is_some(),
        "Controller MUST be saved in host's paired_devices"
    );
}

/// PURPOSE:  A denied pairing request must NOT corrupt RemoteLinkState (spec §5.4/§5.5).
/// SCENARIO: Controller calls pair_device with a token; Host responds with PairDenied; Controller receives the rejection and closes the link.
/// ASSERTS:  pair_device returns Err, controller_manager stays in its previous state (Unpaired), and the host is not persisted.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Set RemoteLinkState::Failed on PairDenied, conflating PairingState with RemoteLinkState.
#[tokio::test]
async fn remote_connection_pairing_denial_keeps_link_state() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, controller_conn) =
        open_migrated_db(dir.path(), "controller_denial.db").await;

    let host_key = DeviceKey::generate().expect("generate host key");
    let host_id = derive_device_id(host_key.public_key());
    let host_pk_hex = hex::encode(host_key.public_key());

    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let addr = listener.local_addr().expect("listener address");

    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    let cm = Arc::clone(&controller_manager);
    let c_db = Arc::new(controller_db);
    let target = format!("127.0.0.1:{}", addr.port());
    let h_id = host_id.as_str().to_string();
    let hpk_str = host_pk_hex.clone();

    let dial_task = tokio::spawn(async move {
        cm.pair_device(
            c_db,
            target,
            "dummy_token_abc".to_string(),
            hpk_str,
            h_id,
            None,
        )
        .await
    });

    let host_ws_link = within_ok("accept connection", listener.accept()).await;
    let initial_msg = within_ok("recv pair request", host_ws_link.recv_control()).await;
    assert!(matches!(initial_msg, ControlMessage::PairRequest(_)));

    let handshake =
        NoiseHandshake::responder(&host_key, None, &default_noise_prologue()).expect("handshake");
    let transport = host_ws_link
        .drive_handshake(handshake)
        .await
        .expect("drive handshake");
    host_ws_link.set_cipher(transport).await;

    // Host explicitly denies the pairing request
    within_ok(
        "send PairDenied",
        host_ws_link.send_control(ControlMessage::PairDenied(PairDenied {
            reason: "User denied pairing".to_string(),
        })),
    )
    .await;

    // Controller dial task completes with an Err
    let dial_res = dial_task.await.expect("join dial task");
    assert!(
        dial_res.is_err(),
        "dial_and_connect MUST return Err when pairing is denied"
    );

    // Spec §5.5: denial leaves RemoteLinkState in its previous state.
    assert_eq!(
        controller_manager.state(),
        RemoteLinkState::Unpaired,
        "Pairing denial MUST NOT corrupt RemoteLinkState"
    );

    // Assert host was NOT saved in controller DB
    let stored = get_paired_device(&controller_conn, host_id.as_str())
        .await
        .expect("query host in controller db");
    assert!(
        stored.is_none(),
        "Host MUST NOT be saved in DB when pairing was denied"
    );
}

/// PURPOSE:  A pairing approval timeout must NOT corrupt RemoteLinkState (spec §5.4/§5.5).
/// SCENARIO: Controller dials and sends PairRequest; Host does not answer; the 35-second approval timeout expires via virtual time advance.
/// ASSERTS:  pair_device returns Err and controller_manager stays in its previous state (Unpaired).
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Set RemoteLinkState::Failed on pairing timeout.
#[tokio::test]
async fn remote_connection_pairing_timeout_keeps_link_state() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, _controller_conn) =
        open_migrated_db(dir.path(), "controller_timeout.db").await;

    let host_key = DeviceKey::generate().expect("generate host key");
    let host_id = derive_device_id(host_key.public_key());
    let host_pk_hex = hex::encode(host_key.public_key());

    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let addr = listener.local_addr().expect("listener address");

    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    let cm = Arc::clone(&controller_manager);
    let c_db = Arc::new(controller_db);
    let target = format!("127.0.0.1:{}", addr.port());
    let h_id = host_id.as_str().to_string();
    let hpk_str = host_pk_hex.clone();

    let dial_task = tokio::spawn(async move {
        cm.pair_device(
            c_db,
            target,
            "timeout_token".to_string(),
            hpk_str,
            h_id,
            None,
        )
        .await
    });

    let host_ws_link = within_ok("accept connection", listener.accept()).await;
    let initial_msg = within_ok("recv pair request", host_ws_link.recv_control()).await;
    assert!(matches!(initial_msg, ControlMessage::PairRequest(_)));

    let handshake =
        NoiseHandshake::responder(&host_key, None, &default_noise_prologue()).expect("handshake");
    let transport = host_ws_link
        .drive_handshake(handshake)
        .await
        .expect("drive handshake");
    host_ws_link.set_cipher(transport).await;

    // Pause time and advance virtual time past 35s timeout
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(36)).await;

    let dial_res = dial_task.await.expect("join dial task");
    assert!(
        dial_res.is_err(),
        "dial_and_connect MUST return Err upon timeout"
    );

    // Spec §5.5: timeout leaves RemoteLinkState in its previous state.
    assert_eq!(
        controller_manager.state(),
        RemoteLinkState::Unpaired,
        "Pairing timeout MUST NOT corrupt RemoteLinkState"
    );
}

/// PURPOSE:  Host must detect missed pongs during link inactivity and transition state to Paired.
/// SCENARIO: Host attaches active link with heartbeat; peer link is closed to drop all pongs; time advances past 3 heartbeat intervals.
/// ASSERTS:  Host heartbeat supervisor marks missed pongs >= 3 and transitions canonical state to RemoteLinkState::Paired.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove missed_pongs check in run_heartbeat_loop, preventing transition to Paired.
#[tokio::test]
async fn remote_connection_heartbeat_timeout_transitions_to_paired() {
    tokio::time::pause();

    let (host_link, client_link) = LoopbackLink::pair();
    let host_manager = RemoteSessionManager::new(SessionRole::Host);

    let peer_identity = SenderIdentity {
        device_id: "controller_device_007".to_string(),
        display_name: "Pixel 9".to_string(),
    };

    host_manager
        .attach_link(Arc::new(host_link), peer_identity, None)
        .await;
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Drop/close client link so that host's outgoing Pings cannot be sent/answered
    within_ok("close client", client_link.close(LinkCloseReason::Local)).await;

    // Advance virtual time step-by-step past 3 missed heartbeat intervals
    for _ in 0..6 {
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        if host_manager.state() == RemoteLinkState::Paired {
            break;
        }
    }

    assert_eq!(host_manager.state(), RemoteLinkState::Paired);
}

/// PURPOSE:  Desktop mutating commands must be denied while Connected under RULE-RC-GUARD, while reads and appearance remain permitted.
/// SCENARIO: Query is_desktop_command_permitted and is_command_permitted with mutating writes, reads, appearance, and remote admin commands in Connected vs Disconnected states.
/// ASSERTS:  When Connected, mutating writes return false; reads, appearance setting, and remote admin return true. When Disconnected, all desktop writes return true. Controller origin always returns true.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Return true unconditionally for Desktop origin in is_desktop_command_permitted.
#[test]
fn remote_connection_rule_rc_guard_blocks_desktop_mutating_writes() {
    let connected = RemoteLinkState::Connected;
    let unpaired = RemoteLinkState::Unpaired;

    // 1. When Connected, Desktop mutating commands MUST be blocked
    let mutating_commands: &[(&str, Option<&str>, Option<&str>)] = &[
        ("start_session", None, None),
        ("end_session", None, None),
        ("launch_engine", None, None),
        ("stop_engine", None, None),
        ("save_personal_memory", None, None),
        ("create_project", None, None),
        ("update_setting", Some("interaction"), None),
        ("update_setting", Some("vad"), None),
        ("update_setting", Some("stt"), None),
        ("update_setting", Some("tts"), None),
        ("update_setting", Some("llm"), None),
        ("update_setting", Some("remote"), Some("execution_mode")),
    ];

    for (cmd, domain, key) in mutating_commands {
        assert!(
            !is_desktop_command_permitted(cmd, *domain, *key, &connected),
            "Command '{cmd}' (domain: {domain:?}) must be BLOCKED on Desktop when Connected"
        );
        assert!(
            !is_command_permitted(ControlOrigin::Desktop, cmd, *domain, *key, &connected),
            "Command '{cmd}' via ControlOrigin::Desktop must be BLOCKED when Connected"
        );
        // Controller origin MUST be permitted
        assert!(
            is_command_permitted(ControlOrigin::Controller, cmd, *domain, *key, &connected),
            "Command '{cmd}' via ControlOrigin::Controller must be PERMITTED when Connected"
        );
    }

    // 2. When Connected, Desktop reads, appearance, and remote admin commands MUST be permitted
    let permitted_commands: &[(&str, Option<&str>, Option<&str>)] = &[
        ("list_audio_devices", None, None),
        ("get_model_catalog", None, None),
        ("get_personal_memory", None, None),
        ("get_settings", None, None),
        ("get_sessions", None, None),
        ("get_turns", None, None),
        ("show_main_window", None, None),
        ("update_setting", Some("appearance"), None),
        ("disconnect_device", None, None),
        ("revoke_device", None, None),
        ("pair_new_device", None, None),
        ("approve_pairing", None, None),
        ("list_paired_devices", None, None),
        (
            "update_setting",
            Some("remote"),
            Some("accepts_controllers"),
        ),
    ];

    for (cmd, domain, key) in permitted_commands {
        assert!(
            is_desktop_command_permitted(cmd, *domain, *key, &connected),
            "Command '{cmd}' (domain: {domain:?}) must be PERMITTED on Desktop when Connected"
        );
        assert!(
            is_command_permitted(ControlOrigin::Desktop, cmd, *domain, *key, &connected),
            "Command '{cmd}' via ControlOrigin::Desktop must be PERMITTED when Connected"
        );
    }

    // 3. When Unpaired, all desktop commands MUST be permitted
    for (cmd, domain, key) in mutating_commands {
        assert!(
            is_desktop_command_permitted(cmd, *domain, *key, &unpaired),
            "Command '{cmd}' must be PERMITTED on Desktop when Unpaired"
        );
    }

    // 4. Verify static routing table contains all essential commands
    let routing = get_routing_table();
    assert_eq!(routing.get("start_session"), Some(&CommandClass::Remote));
    assert_eq!(routing.get("appearance.theme"), Some(&CommandClass::Local));
    assert_eq!(
        routing.get("interaction.pipeline_mode"),
        Some(&CommandClass::Remote)
    );
    assert_eq!(routing.get("get_settings"), Some(&CommandClass::Split));
}

/// PURPOSE:  Host-initiated disconnect must terminate the active connection while keeping device pairing in Turso DB.
/// SCENARIO: Register a paired controller in Turso DB, establish active connection, issue host disconnect.
/// ASSERTS:  Host link transitions to Disconnected and active link closes, but get_paired_device still returns the stored record.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Call delete_paired_device during host disconnect in manager.rs.
#[tokio::test]
async fn remote_connection_host_disconnect_keeps_pairing() {
    let dir = tempdir().expect("tempdir");
    let (_db, conn) = open_migrated_db(dir.path(), "host_disconnect.db").await;

    let device_id = "test_controller_keep_pair";
    let new_device = NewPairedDevice {
        device_id: device_id.to_string(),
        public_key: vec![7u8; 32],
        display_name: "Pixel 9".to_string(),
        role: PairedDeviceRole::Controller,
        trust_level: "full".to_string(),
        hints: vec![],
    };

    let inserted = within_ok(
        "insert paired device",
        insert_paired_device(&conn, &new_device),
    )
    .await;
    assert_eq!(inserted.device_id, device_id);

    // Host establishes active connection
    let (host_link, _client_link) = LoopbackLink::pair();
    let host_manager = RemoteSessionManager::new(SessionRole::Host);
    let peer_identity = SenderIdentity {
        device_id: device_id.to_string(),
        display_name: "Pixel 9".to_string(),
    };

    host_manager
        .attach_link(Arc::new(host_link), peer_identity, None)
        .await;
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Host executes disconnect
    within_ok("disconnect", host_manager.disconnect_host_initiated()).await;

    assert_eq!(host_manager.state(), RemoteLinkState::Paired);
    assert!(host_manager.peer_identity().is_some());

    // Verify pairing survives in Turso DB
    let stored = within_ok("query paired device", get_paired_device(&conn, device_id)).await;
    assert!(
        stored.is_some(),
        "Paired device row MUST persist after host disconnect"
    );
    assert_eq!(stored.unwrap().display_name, "Pixel 9");
}

/// PURPOSE:  Revoking a paired device must permanently remove its row from Turso DB and terminate active link.
/// SCENARIO: Register a paired device in Turso DB, establish active connection, execute delete_paired_device and disconnect.
/// ASSERTS:  Row is deleted from paired_devices table, get_paired_device returns None, list_paired_devices is empty, and host link state is Disconnected.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Omit delete_paired_device execution when revoking device.
#[tokio::test]
async fn remote_connection_revocation_permanently_removes_device() {
    let dir = tempdir().expect("tempdir");
    let (db, conn) = open_migrated_db(dir.path(), "revocation.db").await;

    let device_id = "test_controller_revoked";
    let new_device = NewPairedDevice {
        device_id: device_id.to_string(),
        public_key: vec![9u8; 32],
        display_name: "Stolen Phone".to_string(),
        role: PairedDeviceRole::Controller,
        trust_level: "full".to_string(),
        hints: vec![],
    };

    within_ok("insert device", insert_paired_device(&conn, &new_device)).await;

    // Connect device
    let (host_link, _client_link) = LoopbackLink::pair();
    let host_manager = RemoteSessionManager::new(SessionRole::Host);
    host_manager
        .attach_link(
            Arc::new(host_link),
            SenderIdentity {
                device_id: device_id.to_string(),
                display_name: "Stolen Phone".to_string(),
            },
            None,
        )
        .await;
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Perform revocation: delete row from Turso DB and disconnect session
    let deleted = within_ok("revoke device", host_manager.revoke_device(&db, device_id)).await;
    assert!(deleted, "revoke_device must return true");

    assert_eq!(host_manager.state(), RemoteLinkState::Unpaired);

    // Assert device row is permanently gone
    let check = within_ok("query revoked device", get_paired_device(&conn, device_id)).await;
    assert!(
        check.is_none(),
        "Revoked device MUST NOT exist in paired_devices table"
    );

    let all_devices = within_ok("list paired devices", list_paired_devices(&conn)).await;
    assert!(
        all_devices.is_empty(),
        "paired_devices table must be empty after revocation"
    );
}

/// PURPOSE:  A controller revoking an idle paired host must dial the host, deliver LinkLoss(Revoked), and delete the DB row.
/// SCENARIO: Controller is paired with Host; no active link is present; Controller calls revoke_device; background task dials Host hints and delivers LinkLoss(Revoked).
/// ASSERTS:  Host listener receives LinkLoss(Revoked), Controller transitions to Unpaired, and Host row is deleted from Controller DB.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Revert !has_active_link to !is_current in manager.rs::revoke_device.
#[tokio::test]
async fn remote_connection_controller_idle_revocation_syncs_to_host() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, controller_conn) =
        open_migrated_db(dir.path(), "controller_idle_revoke.db").await;

    // 1. Host setup with loopback listener
    let host_key = DeviceKey::generate().expect("generate host key");
    let host_id = derive_device_id(host_key.public_key());
    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let addr = listener.local_addr().expect("listener address");

    // 2. Controller setup: insert host into controller DB with hints pointing to listener
    let host_device = NewPairedDevice {
        device_id: host_id.as_str().to_string(),
        public_key: host_key.public_key().to_vec(),
        display_name: "Vox Desktop Host".to_string(),
        role: PairedDeviceRole::Host,
        trust_level: "full".to_string(),
        hints: vec![serde_json::json!({ "addr": format!("127.0.0.1:{}", addr.port()) })],
    };
    within_ok(
        "insert host in controller db",
        insert_paired_device(&controller_conn, &host_device),
    )
    .await;

    let controller_manager = RemoteSessionManager::new(SessionRole::Controller);
    controller_manager.set_peer_identity(Some(SenderIdentity {
        device_id: host_id.as_str().to_string(),
        display_name: "Vox Desktop Host".to_string(),
    }));
    controller_manager.set_state(RemoteLinkState::Paired);
    assert_eq!(controller_manager.state(), RemoteLinkState::Paired);
    assert!(controller_manager.peer_identity().is_some());

    // 3. Controller revokes the idle host
    let deleted = within_ok(
        "revoke host from controller",
        controller_manager.revoke_device(&controller_db, host_id.as_str()),
    )
    .await;
    assert!(deleted, "revoke_device must return true");
    assert_eq!(controller_manager.state(), RemoteLinkState::Unpaired);
    assert_eq!(controller_manager.peer_identity(), None);

    // 4. Host listener accepts the notification connection spawned by controller
    let host_ws_link = within_ok("accept connection", listener.accept()).await;

    let handshake = NoiseHandshake::responder(&host_key, None, &default_noise_prologue())
        .expect("responder handshake");
    let transport = host_ws_link
        .drive_handshake(handshake)
        .await
        .expect("drive handshake");
    host_ws_link.set_cipher(transport).await;

    let msg = within_ok("recv LinkLoss", host_ws_link.recv_control()).await;
    match msg {
        ControlMessage::LinkLoss(loss) => {
            assert_eq!(loss.reason, LinkLossReason::Revoked);
        }
        other => panic!("Expected LinkLoss(Revoked), got {other:?}"),
    }

    // Assert controller DB row is permanently deleted
    let check = within_ok(
        "query revoked host",
        get_paired_device(&controller_conn, host_id.as_str()),
    )
    .await;
    assert!(
        check.is_none(),
        "Revoked host row must be deleted from controller DB"
    );
}

/// PURPOSE:  Toggling accepts_controllers off and on must round-trip from Paired -> Disabled -> Paired.
/// SCENARIO: Host has a paired device in DB; transitions to Disabled; hydrate_from_db restores Paired state.
/// ASSERTS:  State is Disabled after toggle off, and transitions back to Paired with peer_identity restored after toggle on.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Require state == Unpaired in hydrate_from_db, leaving host stuck in Disabled.
#[tokio::test]
async fn remote_connection_settings_toggle_round_trip() {
    let dir = tempdir().expect("tempdir");
    let (db, conn) = open_migrated_db(dir.path(), "toggle_round_trip.db").await;

    let device_id = "test_paired_controller";
    let new_device = NewPairedDevice {
        device_id: device_id.to_string(),
        public_key: vec![3u8; 32],
        display_name: "Pixel 9".to_string(),
        role: PairedDeviceRole::Controller,
        trust_level: "full".to_string(),
        hints: vec![],
    };
    within_ok("insert device", insert_paired_device(&conn, &new_device)).await;

    let host_manager = RemoteSessionManager::new(SessionRole::Host);
    host_manager.hydrate_from_db(&db).await;
    assert_eq!(host_manager.state(), RemoteLinkState::Paired);
    assert_eq!(
        host_manager.peer_identity().map(|p| p.device_id),
        Some(device_id.to_string())
    );

    // Toggle off: host enters Disabled and clears peer_identity
    host_manager.set_state(RemoteLinkState::Disabled);
    assert_eq!(host_manager.state(), RemoteLinkState::Disabled);
    assert_eq!(host_manager.peer_identity(), None);

    // Toggle on: hydrate_from_db restores Paired state and peer_identity
    host_manager.hydrate_from_db(&db).await;
    assert_eq!(host_manager.state(), RemoteLinkState::Paired);
    assert_eq!(
        host_manager.peer_identity().map(|p| p.device_id),
        Some(device_id.to_string())
    );
}

/// PURPOSE:  Controller must dial an existing paired host via dial_and_connect, negotiate Noise XX and Hello, and transition both sides to Connected.
/// SCENARIO: Host listener accepts connection and runs Noise responder + Hello exchange; Controller dials paired host device ID.
/// ASSERTS:  Both Controller and Host reach RemoteLinkState::Connected; Controller persists the observed working address to hints.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove manager.attach_link call in dial_and_connect.
#[tokio::test]
async fn remote_connection_dial_and_connect_reaches_connected() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, controller_conn) = open_migrated_db(dir.path(), "ctrl_dial.db").await;

    // 1. Host key, ID, and loopback listener
    let host_key = DeviceKey::generate().expect("host key");
    let host_id = derive_device_id(host_key.public_key()).to_string();
    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let host_port = listener.local_addr().expect("listener address").port();

    // 2. Pre-pair host into controller's DB with host port hint
    insert_paired_device(
        &controller_conn,
        &NewPairedDevice {
            device_id: host_id.clone(),
            public_key: host_key.public_key().to_vec(),
            display_name: "Desktop Host".to_string(),
            role: PairedDeviceRole::Host,
            trust_level: "full".to_string(),
            hints: vec![serde_json::json!({ "addr": format!("127.0.0.1:{host_port}") })],
        },
    )
    .await
    .expect("insert paired host");

    // 3. Host listener accepts and negotiates in background task
    let host_manager = Arc::new(RemoteSessionManager::new(SessionRole::Host));
    let hm = Arc::clone(&host_manager);
    let h_id = host_id.clone();

    tokio::spawn(async move {
        let ws_link = listener.accept().await.expect("host accept");
        let first_msg = ws_link.recv_handshake_message().await.expect("recv msg1");
        let prologue = default_noise_prologue();
        let mut handshake =
            NoiseHandshake::responder(&host_key, None, &prologue).expect("responder");
        handshake.read_message(&first_msg).expect("read msg1");
        let transport = ws_link
            .drive_handshake(handshake)
            .await
            .expect("drive handshake");
        ws_link.set_cipher(transport).await;

        let client_hello = match ws_link.recv_control().await.expect("recv client hello") {
            ControlMessage::Hello(h) => h,
            other => panic!("expected Hello, got {other:?}"),
        };

        let host_hello = Hello {
            version: vox_lib::services::remote::PROTOCOL_VERSION,
            sender_identity: SenderIdentity {
                device_id: h_id,
                display_name: "Desktop Host".to_string(),
            },
            capabilities: vox_lib::services::remote::protocol::capabilities::CAP_CONTROL_MESSAGES,
            uplink_format: StreamFormat::DOWNLINK,
            downlink_format: StreamFormat::UPLINK,
        };
        ws_link
            .send_control(ControlMessage::Hello(host_hello))
            .await
            .expect("send host hello");

        hm.attach_link(Arc::new(ws_link), client_hello.sender_identity, None)
            .await;
    });

    // 4. Controller dials host
    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    within_ok(
        "dial_and_connect",
        controller_manager
            .clone()
            .dial_and_connect(Arc::new(controller_db), host_id.clone(), None),
    )
    .await;

    // 5. Verify states
    assert_eq!(controller_manager.state(), RemoteLinkState::Connected);
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // 6. Verify observed hint persistence in controller DB (spec §5.7)
    let updated = get_paired_device(&controller_conn, &host_id)
        .await
        .expect("query")
        .expect("device exists");
    assert!(
        !updated.hints.is_empty(),
        "Hints must be updated with observed IP"
    );
    assert_eq!(updated.hints[0]["addr"], format!("127.0.0.1:{host_port}"));
    assert_eq!(updated.hints[0]["source"], "Observed");
}

/// PURPOSE:  When the primary stored hint is unreachable, dial_and_connect must cycle to the next candidate and reach Connected.
/// SCENARIO: Controller has dead IP (127.0.0.1:1) at index 0 and working host at index 1.
/// ASSERTS:  dial_and_connect succeeds; Controller connects to candidate 1 and updates observed hint.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Break candidate loop in perform_dial_handshake on first error instead of trying remaining candidates.
#[tokio::test]
async fn remote_connection_dead_primary_hint_falls_back_to_valid_candidate() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, controller_conn) = open_migrated_db(dir.path(), "ctrl_fallback.db").await;

    // 1. Host listener on valid port
    let host_key = DeviceKey::generate().expect("host key");
    let host_id = derive_device_id(host_key.public_key()).to_string();
    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let valid_port = listener.local_addr().expect("listener address").port();

    // 2. Pre-pair host with dead port at index 0, valid port at index 1
    insert_paired_device(
        &controller_conn,
        &NewPairedDevice {
            device_id: host_id.clone(),
            public_key: host_key.public_key().to_vec(),
            display_name: "Desktop Host".to_string(),
            role: PairedDeviceRole::Host,
            trust_level: "full".to_string(),
            hints: vec![
                serde_json::json!({ "addr": "127.0.0.1:1" }),
                serde_json::json!({ "addr": format!("127.0.0.1:{valid_port}") }),
            ],
        },
    )
    .await
    .expect("insert paired host");

    // 3. Host listener accepts and negotiates
    let host_manager = Arc::new(RemoteSessionManager::new(SessionRole::Host));
    let hm = Arc::clone(&host_manager);
    let h_id = host_id.clone();

    tokio::spawn(async move {
        let ws_link = listener.accept().await.expect("host accept");
        let first_msg = ws_link.recv_handshake_message().await.expect("recv msg1");
        let prologue = default_noise_prologue();
        let mut handshake =
            NoiseHandshake::responder(&host_key, None, &prologue).expect("responder");
        handshake.read_message(&first_msg).expect("read msg1");
        let transport = ws_link
            .drive_handshake(handshake)
            .await
            .expect("drive handshake");
        ws_link.set_cipher(transport).await;

        let client_hello = match ws_link.recv_control().await.expect("recv client hello") {
            ControlMessage::Hello(h) => h,
            other => panic!("expected Hello, got {other:?}"),
        };

        let host_hello = Hello {
            version: vox_lib::services::remote::PROTOCOL_VERSION,
            sender_identity: SenderIdentity {
                device_id: h_id,
                display_name: "Desktop Host".to_string(),
            },
            capabilities: vox_lib::services::remote::protocol::capabilities::CAP_CONTROL_MESSAGES,
            uplink_format: StreamFormat::DOWNLINK,
            downlink_format: StreamFormat::UPLINK,
        };
        ws_link
            .send_control(ControlMessage::Hello(host_hello))
            .await
            .expect("send host hello");

        hm.attach_link(Arc::new(ws_link), client_hello.sender_identity, None)
            .await;
    });

    // 4. Controller dials host (should gracefully bypass dead 127.0.0.1:1 and connect to candidate 1)
    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    within_ok(
        "dial_and_connect with dead candidate 0",
        controller_manager
            .clone()
            .dial_and_connect(Arc::new(controller_db), host_id.clone(), None),
    )
    .await;

    assert_eq!(controller_manager.state(), RemoteLinkState::Connected);
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Observed working candidate promoted to index 0
    let updated = get_paired_device(&controller_conn, &host_id)
        .await
        .expect("query")
        .expect("device exists");
    assert_eq!(updated.hints[0]["addr"], format!("127.0.0.1:{valid_port}"));
    assert_eq!(updated.hints[0]["source"], "Observed");
}

/// PURPOSE:  When dial_and_connect fails (all candidates unreachable), the Controller must enter Failed state while preserving peer_identity and state DTO device ID.
/// SCENARIO: Controller dials a paired host with only unreachable candidates; dial fails.
/// ASSERTS:  dial_and_connect returns Err; controller_manager.state() is Failed; peer_identity is Some; state_dto().device_id equals host_id.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Clear self.peer_identity when entering Failed state in dial_and_connect.
#[tokio::test]
async fn remote_connection_dial_failure_retains_peer_identity() {
    let dir = tempdir().expect("tempdir");
    vox_lib::utils::paths::init_with_root(dir.path().to_path_buf());

    let (controller_db, controller_conn) =
        open_migrated_db(dir.path(), "ctrl_fail_identity.db").await;

    let host_key = DeviceKey::generate().expect("host key");
    let host_id = derive_device_id(host_key.public_key()).to_string();

    // Pre-pair host with unreachable candidate
    insert_paired_device(
        &controller_conn,
        &NewPairedDevice {
            device_id: host_id.clone(),
            public_key: host_key.public_key().to_vec(),
            display_name: "MacBook Pro Host".to_string(),
            role: PairedDeviceRole::Host,
            trust_level: "full".to_string(),
            hints: vec![serde_json::json!({ "addr": "127.0.0.1:1" })],
        },
    )
    .await
    .expect("insert paired host");

    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));
    let dial_res = controller_manager
        .clone()
        .dial_and_connect(Arc::new(controller_db), host_id.clone(), None)
        .await;

    assert!(
        dial_res.is_err(),
        "dial_and_connect must fail for unreachable host"
    );
    assert!(
        matches!(controller_manager.state(), RemoteLinkState::Failed { .. }),
        "State must be Failed, got: {:?}",
        controller_manager.state()
    );

    // CRITICAL: Peer identity must be preserved so UI does not show "No device paired"
    let peer = controller_manager.peer_identity();
    assert!(
        peer.is_some(),
        "peer_identity must not be cleared on Failed state"
    );
    assert_eq!(peer.as_ref().unwrap().device_id, host_id);
    assert_eq!(peer.as_ref().unwrap().display_name, "MacBook Pro Host");

    let dto = controller_manager.state_dto();
    assert_eq!(dto.link, "Failed");
    assert_eq!(dto.device_id, Some(host_id));
    assert_eq!(dto.display_name, Some("MacBook Pro Host".to_string()));
}
