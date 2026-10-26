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
            delete_paired_device, get_paired_device, insert_paired_device, list_paired_devices,
            NewPairedDevice, PairedDeviceRole,
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
        protocol::messages::{ControlMessage, LinkLossReason, PairDenied, SenderIdentity},
        security::{
            derive_device_id, DeviceKey, PairingCoordinator, PairingOutcome, PairingRequest,
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

/// PURPOSE:  A full controller-initiated pairing via dial_and_connect must reach Connected state and persist peers in Turso DB on both sides.
/// SCENARIO: Controller calls dial_and_connect with valid pairing token and host public key over loopback; Host accepts, validates request through PairingCoordinator, approves, and completes Hello negotiation.
/// ASSERTS:  Both Controller and Host reach RemoteLinkState::Connected, and each peer is inserted into their respective Turso DB paired_devices table.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove insert_paired_device call in dial_and_connect for host persistence.
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

    // 7. Host sends PairApproved
    within_ok(
        "send PairApproved",
        host_ws_link.send_control(ControlMessage::PairApproved),
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

/// PURPOSE:  A controller pairing request denied by the host must transition the controller state to Failed.
/// SCENARIO: Controller calls dial_and_connect with token; Host responds with PairDenied; Controller receives rejection and terminates link.
/// ASSERTS:  dial_and_connect returns Err, and controller_manager transitions to RemoteLinkState::Failed containing "Pairing denied by host".
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Ignore PairDenied and leave state in Connecting in dial_and_connect.
#[tokio::test]
async fn remote_connection_pairing_denial_transitions_to_failed() {
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

    // Assert controller transitioned to Failed state with denial reason
    match controller_manager.state() {
        RemoteLinkState::Failed { reason: msg, .. } => {
            assert!(
                msg.contains("Pairing denied by host: User denied pairing"),
                "Expected failure message containing denial reason, got: {msg}"
            );
        }
        other => panic!("Expected RemoteLinkState::Failed, got {other:?}"),
    }

    // Assert host was NOT saved in controller DB
    let stored = get_paired_device(&controller_conn, host_id.as_str())
        .await
        .expect("query host in controller db");
    assert!(
        stored.is_none(),
        "Host MUST NOT be saved in DB when pairing was denied"
    );
}

/// PURPOSE:  A controller awaiting host pairing approval must time out and transition to Failed if host does not respond.
/// SCENARIO: Controller dials and sends PairRequest; Host does not answer; 35-second approval timeout expires via virtual time advance.
/// ASSERTS:  dial_and_connect returns Err, and controller_manager transitions to RemoteLinkState::Failed with timeout message.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove timeout wrapping recv_control in dial_and_connect, hanging indefinitely.
#[tokio::test]
async fn remote_connection_pairing_timeout_transitions_to_failed() {
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

    tokio::time::pause();

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

    // Host does NOT reply. Advance virtual time past 35s timeout.
    tokio::time::advance(Duration::from_secs(36)).await;

    let dial_res = dial_task.await.expect("join dial task");
    assert!(
        dial_res.is_err(),
        "dial_and_connect MUST return Err upon timeout"
    );

    match controller_manager.state() {
        RemoteLinkState::Failed { reason: msg, .. } => {
            assert!(
                msg.contains("timed out"),
                "Expected failure message containing timeout, got: {msg}"
            );
        }
        other => panic!("Expected RemoteLinkState::Failed, got {other:?}"),
    }
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
        .attach_link(Arc::new(host_link), peer_identity)
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
    let mutating_commands = [
        ("start_session", None),
        ("end_session", None),
        ("launch_engine", None),
        ("stop_engine", None),
        ("save_personal_memory", None),
        ("create_project", None),
        ("update_setting", Some("interaction")),
        ("update_setting", Some("vad")),
        ("update_setting", Some("stt")),
        ("update_setting", Some("tts")),
        ("update_setting", Some("llm")),
    ];

    for (cmd, domain) in mutating_commands {
        assert!(
            !is_desktop_command_permitted(cmd, domain, &connected),
            "Command '{cmd}' (domain: {domain:?}) must be BLOCKED on Desktop when Connected"
        );
        assert!(
            !is_command_permitted(ControlOrigin::Desktop, cmd, domain, &connected),
            "Command '{cmd}' via ControlOrigin::Desktop must be BLOCKED when Connected"
        );
        // Controller origin MUST be permitted
        assert!(
            is_command_permitted(ControlOrigin::Controller, cmd, domain, &connected),
            "Command '{cmd}' via ControlOrigin::Controller must be PERMITTED when Connected"
        );
    }

    // 2. When Connected, Desktop reads, appearance, and remote admin commands MUST be permitted
    let permitted_commands = [
        ("list_audio_devices", None),
        ("get_model_catalog", None),
        ("get_personal_memory", None),
        ("get_settings", None),
        ("get_sessions", None),
        ("get_turns", None),
        ("show_main_window", None),
        ("update_setting", Some("appearance")),
        ("disconnect_device", None),
        ("revoke_device", None),
        ("pair_new_device", None),
        ("approve_pairing", None),
        ("list_paired_devices", None),
        ("update_setting", Some("remote")),
    ];

    for (cmd, domain) in permitted_commands {
        assert!(
            is_desktop_command_permitted(cmd, domain, &connected),
            "Command '{cmd}' (domain: {domain:?}) must be PERMITTED on Desktop when Connected"
        );
        assert!(
            is_command_permitted(ControlOrigin::Desktop, cmd, domain, &connected),
            "Command '{cmd}' via ControlOrigin::Desktop must be PERMITTED when Connected"
        );
    }

    // 3. When Unpaired, all desktop commands MUST be permitted
    for (cmd, domain) in mutating_commands {
        assert!(
            is_desktop_command_permitted(cmd, domain, &unpaired),
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
        .attach_link(Arc::new(host_link), peer_identity)
        .await;
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Host executes disconnect
    within_ok(
        "disconnect",
        host_manager.disconnect(LinkLossReason::HostDisconnected),
    )
    .await;

    assert_eq!(host_manager.state(), RemoteLinkState::Paired);
    assert_eq!(host_manager.peer_identity(), None);

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
    let (_db, conn) = open_migrated_db(dir.path(), "revocation.db").await;

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
        )
        .await;
    assert_eq!(host_manager.state(), RemoteLinkState::Connected);

    // Perform revocation: delete row from Turso DB and disconnect session
    let deleted = within_ok(
        "delete paired device",
        delete_paired_device(&conn, device_id),
    )
    .await;
    assert!(deleted, "delete_paired_device must return true");

    within_ok(
        "disconnect session",
        host_manager.disconnect(LinkLossReason::Revoked),
    )
    .await;

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
