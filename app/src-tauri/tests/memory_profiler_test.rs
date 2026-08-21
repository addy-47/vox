//! ============================================================================
//! memory_profiler_test.rs — Integration Tests for Memory Profiler & Persistence
//! ============================================================================
//! Category     : Integration Test
//! Component    : Memory Profiler (`vox_lib::ipc::memory_profiler`)
//! Execution    : cargo test --test memory_profiler_test --release
//! ============================================================================

use vox_lib::ipc::memory_profiler::{
    collect_profiler_snapshot, record_memory_profile_event, resolve_temp_dir, sanitize_page_name,
    MemoryProfileLogEvent, ProcessMemoryEntry,
};

#[test]
fn test_sanitize_page_name_normalization() {
    assert_eq!(sanitize_page_name("/history"), "history");
    assert_eq!(sanitize_page_name("/"), "home");
    assert_eq!(sanitize_page_name(""), "home");
    assert_eq!(sanitize_page_name("/memory"), "memory");
    assert_eq!(sanitize_page_name("/settings"), "settings");
    assert_eq!(sanitize_page_name("/dictation/config"), "dictation_config");
}

#[test]
fn test_get_profiler_snapshot_contract() {
    let snapshot = collect_profiler_snapshot(true, false, false);
    assert!(snapshot.total_vox_ram_mb > 0.0, "Total Vox RAM must be > 0 MB");
    assert!(snapshot.main_process_ram_mb > 0.0, "Main process RAM must be > 0 MB");
    assert!(!snapshot.process_tree.is_empty(), "Process tree must contain at least the main process");

    let main_entry = snapshot.process_tree.iter().find(|p| p.is_main_process);
    assert!(main_entry.is_some(), "Process tree must identify the main process");
}

#[tokio::test]
async fn test_record_memory_profile_snapshot_jsonl_persistence() {
    let temp_dir = resolve_temp_dir();
    assert!(temp_dir.exists(), "temp directory must exist");

    let test_timestamp_ms: u64 = 1771599888000;
    let expected_filename = format!("{}-history.jsonl", test_timestamp_ms / 1000);
    let expected_file_path = temp_dir.join(&expected_filename);

    // Clean up any stale test file if present
    if expected_file_path.exists() {
        let _ = std::fs::remove_file(&expected_file_path);
    }

    let event = MemoryProfileLogEvent {
        route: "/history".to_string(),
        event_type: "snapshot".to_string(),
        baseline_ram_mb: Some(120.5),
        current_ram_mb: 145.2,
        peak_ram_mb: Some(150.0),
        peak_delta_mb: Some(24.7),
        retained_ram_mb: None,
        retained_delta_mb: None,
        main_webview_ram_mb: Some(85.0),
        tray_webview_ram_mb: None,
        active_components: vec!["HistoryStage".to_string(), "DetailPanel".to_string()],
        dom_node_count: 450,
        font_face_count: 8,
        timestamp_ms: test_timestamp_ms,
        process_tree: Some(vec![ProcessMemoryEntry {
            pid: 12345,
            parent_pid: None,
            name: "vox-core".to_string(),
            memory_mb: 145.2,
            cpu_usage: 1.5,
            start_time: 1000,
            is_main_process: true,
            role: "Main Process".to_string(),
        }]),
    };

    let result = record_memory_profile_event(event.clone()).await;
    assert!(result.is_ok(), "record_memory_profile_event must return Ok");

    assert!(expected_file_path.exists(), "Expected snapshot file {:?} was not created", expected_file_path);

    let content = std::fs::read_to_string(&expected_file_path).expect("Failed to read snapshot file");
    assert!(!content.trim().is_empty(), "Snapshot file must not be empty");

    let read_event: MemoryProfileLogEvent = serde_json::from_str(content.lines().last().unwrap())
        .expect("Failed to deserialize persisted JSONL snapshot line");

    assert_eq!(read_event.route, "/history");
    assert_eq!(read_event.event_type, "snapshot");
    assert_eq!(read_event.current_ram_mb, 145.2);
    assert_eq!(read_event.dom_node_count, 450);
    assert_eq!(read_event.active_components.len(), 2);
    assert!(read_event.process_tree.is_some());

    // Clean up test artifact
    let _ = std::fs::remove_file(&expected_file_path);
}
