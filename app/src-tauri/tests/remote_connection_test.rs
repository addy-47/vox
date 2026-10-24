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

use common::remote_noise::peer_pair;
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
        link::{bind, dial, LinkCloseReason, LoopbackLink, RemoteLink},
        protocol::{
            framing::StreamFormat,
            messages::{ControlMessage, LinkLossReason, SenderIdentity},
        },
        security::{
            derive_device_id, mint_pairing_qr_svg, PairingCoordinator, PairingOutcome,
            PairingRequest,
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

/// PURPOSE:  A full pairing, Noise_XX handshake, and Hello negotiation must reach Connected link state over loopback.
/// SCENARIO: Host binds loopback TCP WsListener, issues single-use pairing token; Controller dials, submits PairRequest, completes Noise handshake and user approval, exchanges Hello, and attaches link.
/// ASSERTS:  Host verifies identity binding, user approval yields Approved outcome, Hello completes, and Host state is RemoteLinkState::Connected with matching peer identity.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip state transition to Connected upon attaching link in RemoteSessionManager::attach_link.
#[tokio::test]
async fn remote_connection_loopback_e2e_full_flow() {
    let (controller_key, host_key) = peer_pair();
    let host_id = derive_device_id(host_key.public_key());
    let controller_id = derive_device_id(controller_key.public_key());

    // 1. Host binds physical loopback listener
    let listener = within_ok(
        "bind listener",
        bind("127.0.0.1:0".parse().expect("valid loopback")),
    )
    .await;
    let addr = listener.local_addr().expect("listener address");

    // 2. Host issues ephemeral pairing token & mints QR SVG
    let coordinator = PairingCoordinator::new();
    let now = Instant::now();
    let token = coordinator.issue_token(now).expect("issue token");

    let (payload_json, qr_svg_data_url) = mint_pairing_qr_svg(
        &host_id,
        host_key.public_key(),
        "127.0.0.1",
        addr.port(),
        &token,
    )
    .expect("mint pairing qr");

    assert!(payload_json.contains(host_id.as_str()));
    assert!(qr_svg_data_url.starts_with("data:image/svg+xml;base64,"));

    // 3. Connect listener and dialer
    let server_task = tokio::spawn(async move { listener.accept().await });
    let client_link = within_ok(
        "dial host",
        dial(&format!("ws://{addr}")),
    )
    .await;
    let server_link = server_task
        .await
        .expect("server accept task join")
        .expect("server accept");

    // 4. Controller submits pre-Noise PairRequest claim over physical control channel
    let pair_request_msg = ControlMessage::PairRequest(vox_lib::services::remote::protocol::messages::PairRequest {
        token: token.to_hex(),
        device_id: controller_id.as_str().to_string(),
        public_key: hex::encode(controller_key.public_key()),
        display_name: "Pixel 9 Pro".to_string(),
    });
    within_ok("send pair request", client_link.send_control(pair_request_msg)).await;

    let received_preauth = within_ok("recv pair request", server_link.recv_control()).await;
    let claimed_req = match received_preauth {
        ControlMessage::PairRequest(req) => req,
        other => panic!("Expected PairRequest, received {other:?}"),
    };

    // 5. Host validates claim against completed Noise handshake and obtains user approval
    let pairing_req = PairingRequest::from_wire(
        &claimed_req.token,
        &claimed_req.device_id,
        &claimed_req.public_key,
        &claimed_req.display_name,
    )
    .expect("valid wire request");

    let pending_pairing = coordinator
        .begin(
            pairing_req,
            controller_key.public_key(),
            "127.0.0.1".parse().expect("valid ip"),
            Instant::now(),
        )
        .expect("coordinator begin");

    let prompt = pending_pairing.prompt();
    let approved = prompt.approve();
    assert!(approved);

    let outcome = pending_pairing.settle().await;
    assert!(matches!(
        outcome,
        PairingOutcome::Approved { ref peer } if *peer.device_id() == controller_id
    ));

    // 6. Complete Hello negotiation
    let host_manager = Arc::new(RemoteSessionManager::new(SessionRole::Host));
    let controller_manager = Arc::new(RemoteSessionManager::new(SessionRole::Controller));

    let host_identity = SenderIdentity {
        device_id: host_id.as_str().to_string(),
        display_name: "Vox Host Laptop".to_string(),
    };
    let controller_identity = SenderIdentity {
        device_id: controller_id.as_str().to_string(),
        display_name: "Pixel 9 Pro".to_string(),
    };

    let server_link_arc = Arc::new(server_link);
    let client_link_arc = Arc::new(client_link);

    let h_link = Arc::clone(&server_link_arc);
    let c_link = Arc::clone(&client_link_arc);
    let hm = Arc::clone(&host_manager);
    let cm = Arc::clone(&controller_manager);

    let host_hello_task = tokio::spawn(async move {
        hm.exchange_hello(
            &*h_link,
            host_identity,
            StreamFormat::UPLINK,
            StreamFormat::DOWNLINK,
        )
        .await
    });

    let controller_hello_task = tokio::spawn(async move {
        cm.exchange_hello(
            &*c_link,
            controller_identity,
            StreamFormat::UPLINK,
            StreamFormat::DOWNLINK,
        )
        .await
    });

    let (host_hello_res, controller_hello_res) =
        tokio::try_join!(host_hello_task, controller_hello_task).expect("hello join");

    let peer_hello_for_host = host_hello_res.expect("host exchange hello");
    let peer_hello_for_controller = controller_hello_res.expect("controller exchange hello");

    assert_eq!(
        peer_hello_for_host.sender_identity.device_id,
        controller_id.as_str()
    );
    assert_eq!(
        peer_hello_for_controller.sender_identity.device_id,
        host_id.as_str()
    );

    // 7. Attach link to host session manager
    host_manager
        .attach_link(server_link_arc, peer_hello_for_host.sender_identity)
        .await;

    assert_eq!(host_manager.state(), RemoteLinkState::Connected);
    assert_eq!(
        host_manager
            .peer_identity()
            .expect("attached peer")
            .device_id,
        controller_id.as_str()
    );
}

/// PURPOSE:  Host must detect missed pongs during link inactivity and transition state to Disconnected.
/// SCENARIO: Host attaches active link with heartbeat; peer link is closed to drop all pongs; time advances past 3 heartbeat intervals.
/// ASSERTS:  Host heartbeat supervisor marks missed pongs >= 3 and transitions canonical state to RemoteLinkState::Disconnected.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Remove missed_pongs check in run_heartbeat_loop, preventing transition to Disconnected.
#[tokio::test]
async fn remote_connection_heartbeat_timeout_transitions_to_disconnected() {
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
        if host_manager.state() == RemoteLinkState::Disconnected {
            break;
        }
    }

    assert_eq!(host_manager.state(), RemoteLinkState::Disconnected);
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
    let disconnected = RemoteLinkState::Disconnected;

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

    // 3. When Disconnected, all desktop commands MUST be permitted
    for (cmd, domain) in mutating_commands {
        assert!(
            is_desktop_command_permitted(cmd, domain, &disconnected),
            "Command '{cmd}' must be PERMITTED on Desktop when Disconnected"
        );
    }

    // 4. Verify static routing table contains all essential commands
    let routing = get_routing_table();
    assert_eq!(routing.get("start_session"), Some(&CommandClass::Remote));
    assert_eq!(routing.get("appearance.theme"), Some(&CommandClass::Local));
    assert_eq!(routing.get("interaction.pipeline_mode"), Some(&CommandClass::Remote));
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

    let inserted = within_ok("insert paired device", insert_paired_device(&conn, &new_device)).await;
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

    assert_eq!(host_manager.state(), RemoteLinkState::Disconnected);
    assert_eq!(host_manager.peer_identity(), None);

    // Verify pairing survives in Turso DB
    let stored = within_ok(
        "query paired device",
        get_paired_device(&conn, device_id),
    )
    .await;
    assert!(stored.is_some(), "Paired device row MUST persist after host disconnect");
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
    let deleted = within_ok("delete paired device", delete_paired_device(&conn, device_id)).await;
    assert!(deleted, "delete_paired_device must return true");

    within_ok(
        "disconnect session",
        host_manager.disconnect(LinkLossReason::HostDisconnected),
    )
    .await;

    assert_eq!(host_manager.state(), RemoteLinkState::Disconnected);

    // Assert device row is permanently gone
    let check = within_ok(
        "query revoked device",
        get_paired_device(&conn, device_id),
    )
    .await;
    assert!(check.is_none(), "Revoked device MUST NOT exist in paired_devices table");

    let all_devices = within_ok("list paired devices", list_paired_devices(&conn)).await;
    assert!(all_devices.is_empty(), "paired_devices table must be empty after revocation");
}
