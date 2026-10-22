//! ============================================================================
//! tests/paired_devices_persistence_test.rs — Remote Peer Registry Persistence
//! ============================================================================
//! Category     : Integration Test (Seam 20 — persistence)
//! Component    : persistence/{schema,paired_devices,mod}.rs
//! Prerequisites: Turso embedded engine; isolated temporary databases
//! Execution    : cargo nextest run --test paired_devices_persistence_test --release --nocapture --test-threads=1
//! Metrics      : (a) `paired_devices` exists with the documented columns and
//!                constraints (role allow-list, 32-byte public key), and the
//!                schema version increments exactly once; (b) insert / list /
//!                look-up / hint update / last-seen update / delete all work
//!                through the persistence module, and an unrecognised hint
//!                kind round-trips without loss.
//!
//! WHY THIS EXISTS
//! ---------------
//! `paired_devices` is the trust store of the remote-control feature. It is
//! also the only table with no foreign keys: a pairing must never be deleted
//! by a cascade, and a pairing must never block a session delete. Those two
//! properties are structural, so they are asserted structurally.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! The constraint checks issue raw SQL on purpose: the role allow-list and
//! the public-key length check are enforced by the storage engine, so they
//! can only be proven by attempting a violating write.
//!
//! The "one write path for `last_seen_at`" rule (RULE-DB-04) and the "no
//! SQL outside the persistence layer" rule (RULE-DB-01) are architectural
//! invariants. They are enforced by `scripts/lint-arch.sh`, not by a test:
//! a source scan written as a test is a lint wearing a test's clothes.
//! ============================================================================

mod common;

use std::{path::Path, time::Duration};

use tempfile::tempdir;
use vox_lib::persistence::{
    paired_devices::{
        delete_paired_device, get_paired_device, insert_paired_device, list_paired_devices,
        mark_paired_device_seen, update_paired_device_hints, NewPairedDevice, PairedDeviceRole,
    },
    schema::{run_migrations, SCHEMA_VERSION},
    VoxDb,
};

/// A syntactically valid device id: lowercase hex, as produced by
/// `SHA-256("vox/device-id/v1" ‖ public_key)`.
const DEVICE_ID: &str = "9f8c1d2b3a4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8";

/// The 32-byte X25519 static public key of the peer.
fn sample_public_key() -> Vec<u8> {
    (0u8..32).collect()
}

/// The schema version this wave introduces.
///
/// Pinned as a literal, NOT derived from `SCHEMA_VERSION`. A derived value
/// (`SCHEMA_VERSION - 1`) makes the migration test tautological: bumping the
/// schema to 17 would move the "previous" version with it and the test would
/// still pass, so a skipped version would go unnoticed. The literals below
/// are the contract: the registry landed in v15, on top of v14.
const CURRENT_SCHEMA_VERSION: u32 = 15;

/// The schema version that shipped before the remote peer registry.
const PREVIOUS_SCHEMA_VERSION: u32 = 14;

/// Opens a fresh database in an isolated temporary directory and migrates it.
async fn open_migrated_db(dir: &Path, file_name: &str) -> (VoxDb, turso::Connection) {
    let db = VoxDb::open(dir.join(file_name))
        .await
        .expect("Failed to open VoxDb");
    let conn = db.connect().expect("Failed to vend connection");
    run_migrations(&conn)
        .await
        .expect("Failed to run migrations");
    (db, conn)
}

/// Reads the `PRAGMA user_version` of a database.
async fn user_version(conn: &turso::Connection) -> i64 {
    let mut rows = conn
        .query("PRAGMA user_version;", ())
        .await
        .expect("Failed to query user_version");
    rows.next()
        .await
        .expect("user_version query must return a row")
        .expect("user_version row must exist")
        .get(0)
        .expect("user_version must be readable")
}

/// Counts the rows in `sqlite_master` that match a table name.
async fn table_count(conn: &turso::Connection, table: &str) -> i64 {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?;",
            (table.to_string(),),
        )
        .await
        .expect("Failed to query sqlite_master");
    rows.next()
        .await
        .expect("sqlite_master query must return a row")
        .expect("row must exist")
        .get(0)
        .expect("count must be readable")
}

// ============================================================================
// Test 1: The table exists with its documented constraints
// ============================================================================

/// Verifies:
///   - The table exists after the migration.
///   - It carries exactly the documented columns, with the documented types,
///     nullability and primary key.
///   - The role allow-list rejects an unknown role.
///   - The public-key length check rejects a wrong-length key.
///   - The table has no foreign keys, so no cascade can delete a pairing.
///   - The schema version is pinned and increments exactly once.
#[tokio::test]
async fn test_paired_devices_table_exists_with_documented_constraints() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let (_db, conn) = open_migrated_db(dir.path(), "paired_devices_schema.db").await;

        // 1. The table exists.
        assert_eq!(
            table_count(&conn, "paired_devices").await,
            1,
            "The `paired_devices` table must exist after the migration"
        );

        // 2. The columns match the documented schema.
        let mut columns = Vec::new();
        let mut col_rows = conn
            .query("PRAGMA table_info(paired_devices);", ())
            .await
            .expect("Failed to query table_info");
        while let Some(row) = col_rows.next().await.expect("table_info must iterate") {
            let name: String = row.get(1).expect("column name must be readable");
            let decl_type: String = row.get(2).expect("column type must be readable");
            let not_null: i64 = row.get(3).expect("notnull flag must be readable");
            let pk: i64 = row.get(5).expect("primary key flag must be readable");
            columns.push((name, decl_type, not_null, pk));
        }

        let expected: Vec<(&str, &str, i64, i64)> = vec![
            ("device_id", "TEXT", 0, 1),
            ("public_key", "BLOB", 1, 0),
            ("display_name", "TEXT", 1, 0),
            ("role", "TEXT", 1, 0),
            ("trust_level", "TEXT", 1, 0),
            ("hints", "TEXT", 1, 0),
            ("paired_at", "INTEGER", 1, 0),
            ("last_seen_at", "INTEGER", 0, 0),
        ];
        assert_eq!(
            columns.len(),
            expected.len(),
            "The table must carry exactly the documented columns; got {:?}",
            columns
        );
        for (actual, wanted) in columns.iter().zip(expected.iter()) {
            assert_eq!(
                actual.0, wanted.0,
                "Column names must match the database specification"
            );
            assert_eq!(
                actual.1, wanted.1,
                "Column `{}` must have type {}",
                actual.0, wanted.1
            );
            assert_eq!(
                actual.2, wanted.2,
                "Column `{}` nullability must match the database specification",
                actual.0
            );
            assert_eq!(
                actual.3, wanted.3,
                "Column `{}` primary-key flag must match the database specification",
                actual.0
            );
        }

        // 3. The role allow-list rejects an unknown role.
        let bad_role = conn
            .execute(
                "INSERT INTO paired_devices \
                 (device_id, public_key, display_name, role, trust_level, hints, paired_at) \
                 VALUES ('bad-role-device', ?, 'Spectator', 'spectator', 'full', '[]', 1);",
                (sample_public_key(),),
            )
            .await;
        assert!(
            bad_role.is_err(),
            "The role allow-list must reject a role that is not 'controller' or 'host'"
        );

        // 4. The public-key length check rejects a wrong-length key.
        let short_key = vec![0u8; 31];
        let bad_key = conn
            .execute(
                "INSERT INTO paired_devices \
                 (device_id, public_key, display_name, role, trust_level, hints, paired_at) \
                 VALUES ('bad-key-device', ?, 'Short Key', 'controller', 'full', '[]', 1);",
                (short_key,),
            )
            .await;
        assert!(
            bad_key.is_err(),
            "The public-key length check must reject a key that is not 32 bytes"
        );

        // 5. The table has no foreign keys: a pairing can never be cascaded
        //    away, and a pairing can never block a session delete.
        let mut fk_rows = conn
            .query("PRAGMA foreign_key_list(paired_devices);", ())
            .await
            .expect("Failed to query foreign_key_list");
        let mut fk_count = 0;
        while fk_rows
            .next()
            .await
            .expect("fk list must iterate")
            .is_some()
        {
            fk_count += 1;
        }
        assert_eq!(
            fk_count, 0,
            "`paired_devices` must be a standalone entity with no foreign keys"
        );

        // 6. The schema version is pinned and increments exactly once.
        assert_eq!(
            SCHEMA_VERSION, CURRENT_SCHEMA_VERSION,
            "The schema version must be exactly v{}; a skipped or unexpected \
             version must fail this test, not silently move the expectations",
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(
            PREVIOUS_SCHEMA_VERSION,
            CURRENT_SCHEMA_VERSION - 1,
            "The previous schema version must be the one immediately below the current one"
        );
        assert_eq!(
            user_version(&conn).await,
            CURRENT_SCHEMA_VERSION as i64,
            "A fresh database must migrate to the current schema version"
        );

        run_migrations(&conn)
            .await
            .expect("Re-running the migration must succeed");
        assert_eq!(
            user_version(&conn).await,
            CURRENT_SCHEMA_VERSION as i64,
            "The schema version must increment exactly once"
        );
    })
    .await
    .expect("test_paired_devices_table_exists_with_documented_constraints timed out");
}

// ============================================================================
// Test 2: The access layer round-trips
// ============================================================================

/// Verifies the full access-layer surface: insert, look-up, list, hint update,
/// last-seen update and delete — and that the persistence layer never
/// interprets a hint, so a newer transport's hints are not destroyed by an
/// older build reading them.
#[tokio::test]
async fn test_the_paired_device_access_layer_round_trips() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let dir = tempdir().expect("Failed to create tempdir");
        let (_db, conn) = open_migrated_db(dir.path(), "access_layer.db").await;

        // 1. Insert.
        let inserted = insert_paired_device(
            &conn,
            &NewPairedDevice {
                device_id: DEVICE_ID.to_string(),
                public_key: sample_public_key(),
                display_name: "Ada's Phone".to_string(),
                role: PairedDeviceRole::Controller,
                trust_level: "full".to_string(),
                hints: Vec::new(),
            },
        )
        .await
        .expect("Inserting a pairing must succeed");

        assert_eq!(inserted.device_id, DEVICE_ID);
        assert_eq!(inserted.display_name, "Ada's Phone");
        assert_eq!(inserted.role, PairedDeviceRole::Controller);
        assert_eq!(inserted.trust_level, "full");
        assert_eq!(inserted.public_key, sample_public_key());
        assert!(
            inserted.paired_at > 0,
            "The pairing time must be stamped by the access layer"
        );
        assert!(
            inserted.last_seen_at.is_none(),
            "A fresh pairing must not carry a last-seen time"
        );

        // 2. Look up.
        let fetched = get_paired_device(&conn, DEVICE_ID)
            .await
            .expect("Look-up must succeed")
            .expect("The inserted pairing must be found");
        assert_eq!(fetched, inserted);

        // 3. List.
        let listed = list_paired_devices(&conn)
            .await
            .expect("Listing must succeed");
        assert_eq!(listed.len(), 1, "Exactly one pairing must be listed");
        assert_eq!(listed[0].device_id, DEVICE_ID);

        // 4. Hint update. An unrecognised kind must survive untouched.
        let known = serde_json::json!({
            "kind": "lan",
            "addr": "192.168.1.24",
            "port": 51234,
            "source": "qr",
            "last_ok_at": null
        });
        let unknown = serde_json::json!({
            "kind": "relay",
            "relay_id": "eu-west-3",
            "path": "/vox/relay",
            "expires_at": 1893456000
        });
        let hints = vec![known.clone(), unknown.clone()];
        let updated = update_paired_device_hints(&conn, DEVICE_ID, &hints)
            .await
            .expect("The hint update must succeed");
        assert_eq!(updated, 1, "Exactly one row must be updated");
        let after_hints = get_paired_device(&conn, DEVICE_ID)
            .await
            .expect("Look-up must succeed")
            .expect("The pairing must still exist");
        assert_eq!(after_hints.hints, hints, "The hints must round-trip");
        assert_eq!(
            after_hints.hints[1], unknown,
            "An unrecognised hint kind must round-trip without data loss"
        );

        // 5. Last-seen update, through the explicit connect/disconnect path.
        let marked = mark_paired_device_seen(&conn, DEVICE_ID)
            .await
            .expect("The presence update must succeed");
        assert_eq!(marked, 1, "Exactly one row must be updated");
        let after_presence = get_paired_device(&conn, DEVICE_ID)
            .await
            .expect("Look-up must succeed")
            .expect("The pairing must still exist");
        assert!(
            after_presence.last_seen_at.is_some(),
            "A connect event must write the last-seen time"
        );

        // 6. Delete — the complete and immediate revocation of a peer.
        let deleted = delete_paired_device(&conn, DEVICE_ID)
            .await
            .expect("The delete must succeed");
        assert!(
            deleted,
            "Deleting an existing pairing must report the deletion"
        );
        assert!(
            get_paired_device(&conn, DEVICE_ID)
                .await
                .expect("Look-up must succeed")
                .is_none(),
            "A deleted pairing must be gone"
        );
        assert!(
            list_paired_devices(&conn)
                .await
                .expect("Listing must succeed")
                .is_empty(),
            "A deleted pairing must not be listed"
        );
        assert!(
            !delete_paired_device(&conn, DEVICE_ID)
                .await
                .expect("A repeated delete must succeed"),
            "Deleting an absent pairing must report that nothing was deleted"
        );
    })
    .await
    .expect("test_the_paired_device_access_layer_round_trips timed out");
}
